use std::error::Error;
use std::ffi::OsString;
use std::io::{Read, Write};

use clap::Clap;
use ini::Ini;
use reqwest::Client;

#[derive(Clap)]
struct Opts {
    #[clap(long, default_value = "4")]
    disk_threads: usize,
    #[clap(long, default_value = "")]
    output_dir: String,
    #[clap(long, default_value = "64")]
    network_threads: usize,
}

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
    let index_str = version_ini.get_from(Some("VERSION"), "INDEX").unwrap_or("");

    eprintln!("Current version is {}", index_str);

    if index_str.is_empty() {
        eprintln!("Invalid  VERSION.INDEX in version.ini");
        return Ok(1);
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
