#![feature(duration_consts_2)]
#![feature(iter_zip)]

mod io_mgr;

use std::cmp::min;
use std::collections::BTreeSet;
use std::error::Error;
use std::ffi::OsString;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::fs::{create_dir_all, File};
use std::io::{Error as IoError, Seek, SeekFrom};
use std::iter::zip;
use std::mem::drop;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use clap::Clap;
use generic_array::{typenum::U20, GenericArray};
use ini::Ini;
use reqwest::header::{CONTENT_LENGTH, RANGE};
use reqwest::{Client, Error as RequestError};
use tokio::sync::Semaphore;
use tokio::task::{JoinError, JoinHandle};
use trust_dns_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use trust_dns_resolver::AsyncResolver;

use la_tools::git_index;
use la_tools::git_index::Hash;

use io_mgr::create_mmap;

struct FinalFile {
    hash: Hash,
    name: String,
    size: u64,
}

#[derive(Clap)]
struct Opts {
    #[clap(long, default_value = "4")]
    disk_threads: usize,
    #[clap(long, default_value = "")]
    output_dir: String,
    #[clap(long, default_value = "64")]
    network_threads: usize,
    #[clap(long)]
    system_dns: bool,
}

#[derive(Debug)]
enum ChunkError {
    Io(IoError),
    Join(JoinError),
    Request(RequestError),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MainError {
    DohFail,
    InvalidVersionIni,
    InvalidGitIndex,
}

impl Display for ChunkError {
    fn fmt(self: &Self, f: &mut Formatter) -> FmtResult {
        write!(f, "{:?}", self)
    }
}

impl Error for ChunkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use ChunkError::*;
        match self {
            Io(e) => e.source(),
            Join(e) => e.source(),
            Request(e) => e.source(),
        }
    }
}

impl Display for MainError {
    fn fmt(self: &Self, f: &mut Formatter) -> FmtResult {
        write!(f, "{:?}", self)
    }
}

impl Error for MainError {}

const CHUNK_SIZE: u64 = 16 * 1024 * 1024;
const RETRY_WAIT_BASE: Duration = Duration::new(1, 0); // 1 second

#[tokio::main]
pub async fn try_main<I, T>(itr: I) -> Result<i32, Box<dyn Error>>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let opts = Opts::parse_from(itr);

    let mut client_builder =
        Client::builder().user_agent("PmangDownloader_27cf2b254140ab9a07a7b8615e18d902c0a26edc");

    if !opts.system_dns {
        eprintln!("Finding real IP of la.cdn.gameon.jp");
        let mut group = NameServerConfigGroup::cloudflare_https();
        group.merge(NameServerConfigGroup::google_https());
        let resolver = AsyncResolver::tokio(
            ResolverConfig::from_parts(None, vec![], group),
            ResolverOpts::default(),
        )
        .map_err(|_| MainError::DohFail)?;
        let responses = resolver
            .ipv4_lookup("la.cdn.gameon.jp")
            .await
            .map_err(|_| MainError::DohFail)?;
        let mut found = false;
        for response in responses.iter() {
            let cdn_ip = response;
            eprintln!("la.cdn.gameon.jp is at {}", cdn_ip);
            let cdn_addr = std::net::SocketAddrV4::new(*cdn_ip, 80);
            client_builder = client_builder.resolve("la.cdn.gameon.jp", cdn_addr.into());
            found = true;
            break;
        }
        if !found {
            eprintln!("DNS resolution failed");
            return Ok(1);
        }
    }

    let client = client_builder.build()?;

    eprintln!("Downloading version.ini");
    let version_ini_str = client
        .get("http://games.cdn.gameon.jp/lostark/version.ini")
        .send()
        .await?
        .text()
        .await?;

    let version_ini = Ini::load_from_str(&version_ini_str)?;
    let index_name_str = version_ini
        .get_from(Some("VERSION"), "INDEX")
        .ok_or(MainError::InvalidVersionIni)?;

    eprintln!("Current version is {}", index_name_str);

    if index_name_str.is_empty() {
        eprintln!("Invalid VERSION.INDEX in version.ini");
        return Ok(2);
    }

    eprintln!("Downloading index");
    let index_bytes = client
        .get(format!(
            "http://la.cdn.gameon.jp/la/patch/{}",
            &index_name_str
        ))
        .send()
        .await?
        .bytes()
        .await?;
    let index = git_index::parse(&index_bytes).ok_or(MainError::InvalidGitIndex)?;

    eprintln!("Index defines {} files", index.entries.len());

    let entries: Vec<FinalFile> = index
        .entries
        .iter()
        .filter_map(|e| {
            let s = std::str::from_utf8(e.name).ok()?;
            Some(FinalFile {
                hash: e.header.sha1,
                name: s.to_string(),
                size: e.header.size.into(),
            })
        })
        .collect();

    if entries.len() != index.entries.len() {
        eprintln!(
            "{} files had invalid filenames",
            index.entries.len() - entries.len()
        );
        return Ok(3);
    }

    eprintln!("Calculating directories");

    let dirs: BTreeSet<String> = entries
        .iter()
        .map(|e| &e.name)
        .filter_map(|n| Some(n[..n.rfind('/')?].into()))
        .collect();

    eprintln!("Found {} directories:", dirs.len());

    for d in dirs.iter() {
        eprintln!("    {}", d);
    }

    let mut out_dir = opts.output_dir;
    if out_dir.is_empty() {
        out_dir = get_fallback_output_dir();
    }
    if out_dir.is_empty() {
        eprintln!("Run the official installer at least once, or specify --output-dir");
        return Ok(4);
    }

    eprintln!("Will download to {}", &out_dir);

    let out_path = Path::new(&out_dir);

    eprintln!("Creating directories");
    for d in dirs.iter() {
        let p = out_path.join(d);
        eprintln!("    {}", p.to_string_lossy());
        create_dir_all(&p)?;
    }

    eprintln!("Checking for already completed files:");
    let todo_entries: Vec<FinalFile> = entries
        .into_iter()
        .filter_map(|e| {
            if let Ok(mut f) = File::open(out_path.join(&e.name)) {
                if let Ok(size) = f.seek(SeekFrom::End(0)) {
                    if size == e.size {
                        eprintln!("    {}", e.name);
                        return None;
                    }
                }
            }
            Some(e)
        })
        .collect();

    eprintln!("{} files left to download", todo_entries.len());

    let net_sem = Arc::new(Semaphore::new(opts.network_threads));

    eprintln!("Downloading object information");
    let content_length_tasks = todo_entries
        .iter()
        .map(|e| {
            let sem = net_sem.clone();
            let url = url_for_hash(&e.hash);
            let req = client.head(url);
            tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();
                let res_result = req.send().await;
                drop(_permit);
                res_result
            })
        })
        .collect::<Vec<_>>();

    let mut total_content_length = 0u64;
    let mut content_lengths = Vec::<u64>::new();
    content_lengths.reserve_exact(todo_entries.len());
    for (e, t) in zip(todo_entries.iter(), content_length_tasks.into_iter()) {
        let res = t.await??;
        match res.headers().get(CONTENT_LENGTH) {
            Some(s) => {
                let content_length = s.to_str()?.parse()?;
                total_content_length += content_length;
                content_lengths.push(content_length);
            }
            None => {
                eprintln!(
                    "Could not get content length of {} ({:x}) {:?}",
                    e.name,
                    GenericArray::from(e.hash),
                    res
                );
                return Ok(5);
            }
        }
    }

    eprintln!(
        "Total of {:.3} GiB to download",
        (total_content_length as f64) / 1024. / 1024. / 1024.
    );

    let mut total_chunks = 0u64;
    let file_tasks = zip(todo_entries.iter(), content_lengths.iter())
        .map(|(e, l)| {
            let len = *l;
            let name = e.name.clone();
            let tmp_path = out_path.join(format!("{}.tmp", &name));
            let url = url_for_hash(&e.hash);

            let total_file_chunks = (len + CHUNK_SIZE - 1) / CHUNK_SIZE;
            total_chunks += total_file_chunks;
            let mut chunk_tasks = Vec::<JoinHandle<Result<(), ChunkError>>>::new();
            for chunk_i in 0u64..total_file_chunks {
                let client_ref = client.clone();
                let name_clone = name.clone();
                let sem = net_sem.clone();
                let url_clone = url.clone();

                let range_begin = chunk_i * CHUNK_SIZE;
                let range_end = min(len, (chunk_i + 1u64) * CHUNK_SIZE);
                let range_size = range_end - range_begin;
                let range_str = format!("bytes={}-{}", range_begin, range_end - 1);
                let req = client_ref.get(url_clone.clone()).header(RANGE, range_str.clone()).build().unwrap(); // TODO: eliminate unwrap
                let tmp_path_clone = tmp_path.clone();
                let task = tokio::spawn(async move {
                    // first take the semaphore so that we don't open files before we're ready
                    let _permit = sem.acquire_owned().await.unwrap();
                    // now acquire mmap
                    // TODO: make the conversion from u64 to usize nicer
                    let mut mapping = create_mmap(tmp_path_clone, len, range_begin, range_size as usize).map_err(ChunkError::Io)?;
                    let mut retry = 0;
                    loop {
                        // send request and wait for response
                        let res_result = client_ref.execute(req.try_clone().unwrap()).await; // TODO: eliminate unwrap
                        // verify result
                        match res_result {
                            Ok(res) => {
                                if res.status() != 206 {
                                    let delay = RETRY_WAIT_BASE * 2u32.pow(retry);
                                    eprintln!(
                                        "Error downloading {} ({}) chunk {} ({}) (retry {}) wait {:?}: {}",
                                        &name_clone, &url_clone, chunk_i, &range_str, retry, &delay, res.status()
                                    );
                                    tokio::time::sleep(delay).await;
                                    retry += 1;
                                    continue;
                                }
                                let bytes = res.bytes().await.map_err(ChunkError::Request)?;
                                mapping.copy_from_slice(bytes.as_ref());
                                mapping.flush_async().map_err(ChunkError::Io)?;
                                break;
                            }
                            Err(e) => {
                                let delay = RETRY_WAIT_BASE * 2u32.pow(retry);
                                eprintln!(
                                    "Error downloading {} ({}) chunk {} ({}) (retry {}) wait {:?}: {:?}",
                                    &name_clone, &url_clone, chunk_i, &range_str, retry, &delay, e
                                );
                                tokio::time::sleep(delay).await;
                                retry += 1;
                            }
                        }
                    }
                    // allow another task to request
                    drop(_permit);
                    Ok(())
                });
                chunk_tasks.push(task);
            }

            // TODO: task to decode the git object
            {
                let task = tokio::spawn(async move {
                    for t in chunk_tasks.into_iter() {
                        t.await.map_err(ChunkError::Join)??;
                    }
                    // TODO: actually decode
                    eprintln!("Chunk tasks complete for {}", &name);
                    Ok::<(), ChunkError>(())
                });
                task
            }
        })
        .collect::<Vec<_>>();

    for t in file_tasks.into_iter() {
        t.await??;
    }

    eprintln!("All done!");

    Ok(0)
}

#[cfg(not(target_os = "windows"))]
fn get_fallback_output_dir() -> String {
    "".into()
}

#[cfg(target_os = "windows")]
fn get_fallback_output_dir() -> String {
    {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let la_key = hkcu.open_subkey("SOFTWARE\\GameOn\\Pmang\\lostark")?;
        let location_val = la_key.get_value("location")?;
        Some(location_val)
    }
    .unwrap_or("")
}

fn url_for_hash<'a>(hash: &Hash) -> String {
    let hash_str = format!("{:x}", GenericArray::<u8, U20>::from_slice(hash));
    let url = format!(
        "http://la.cdn.gameon.jp/la/patch/objects/{}/{}",
        &hash_str[..2],
        &hash_str[2..]
    );
    url
}
