use std::collections::BTreeSet;
use std::error::Error;
use std::ffi::OsString;
use std::fmt::{Display, Formatter, Result as FmtResult};

use clap::Clap;
use ini::Ini;
use reqwest::Client;

use la_tools::git_index;

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

    eprintln!("Calculating directories");

    let dirs: BTreeSet<String> = index
        .entries
        .iter()
        .filter_map(|e| {
            let s = std::str::from_utf8(e.name).ok()?;
            Some(s[..s.rfind('/')?].into())
        })
        .collect();

    eprintln!("Found {} directories:", dirs.len());

    for d in dirs.iter() {
        eprintln!("    {}", d);
    }

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
