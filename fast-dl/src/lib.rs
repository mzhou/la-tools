use std::error::Error;
use std::ffi::OsString;
use std::io::{Read, Write};

use clap::Clap;
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
    let version_ini = client
        .get("http://games.cdn.gameon.jp/lostark/version.ini")
        .send()
        .await;
    eprintln!("{:?}", version_ini);

    Ok(0)
}

fn get_fallback_output_dir() -> String {
    "C:\\GameOn\\LOST ARK".into()
}
