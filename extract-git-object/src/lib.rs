use std::error::Error;
use std::ffi::OsString;
use std::io::{copy, stdin, stdout};

use la_tools::git_object;

pub fn try_main<I, T>(_itr: I) -> Result<i32, Box<dyn Error>> where I: IntoIterator<Item = T>, T: Into<OsString> + Clone {
    let in_file = stdin();
    let mut out_file = stdout();

    let mut decode_read = git_object::decode_sync(in_file);

    copy(&mut decode_read, &mut out_file)?;

    Ok(0)
}
