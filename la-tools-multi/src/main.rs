use std::ffi::OsString;
use std::error::Error;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::process::exit(try_main(std::env::args_os())?)
}

pub fn try_main<I, T>(itr: I) -> Result<i32, Box<dyn Error>> where I: IntoIterator<Item = T>, T: Into<OsString> + Clone {
    extract_git_object::try_main(itr)
}
