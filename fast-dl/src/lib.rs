use std::collections::BTreeSet;
use std::error::Error;
use std::ffi::OsString;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::fs::{create_dir_all, File};
use std::io::{Seek, SeekFrom};
use std::path::Path;

use clap::Clap;
use ini::Ini;
use reqwest::Client;

use la_tools::git_index;

struct FinalFile {
    hash: git_index::Hash,
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
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MainError {
    InvalidVersionIni,
    InvalidGitIndex,
}

impl Display for MainError {
    fn fmt(self: &Self, f: &mut Formatter) -> FmtResult {
        write!(f, "{:?}", self)
    }
}

impl Error for MainError {}

#[tokio::main]
pub async fn try_main<I, T>(itr: I) -> Result<i32, Box<dyn Error>>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let opts = Opts::parse_from(itr);

    let client = Client::builder()
        .user_agent("PmangDownloader_27cf2b254140ab9a07a7b8615e18d902c0a26edc")
        .build()?;

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
        return Ok(1);
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
        return Ok(2);
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
        return Ok(3);
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
