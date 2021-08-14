use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io::{copy, Seek, SeekFrom};
use std::iter::Iterator;

use la_tools::git_object;
use la_tools::git_object::Digest;

pub fn try_main<I, T>(itr: I) -> Result<i32, Box<dyn Error>>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args: Vec<String> = itr
        .into_iter()
        .map(|i| i.into().to_string_lossy().into())
        .collect();
    if args.len() < 2 {
        eprintln!("Usage: hash-git-object <file>");
        return Ok(1);
    }

    let file_name = &args[1];
    let file_size = {
        let mut f = File::open(file_name)?;
        f.seek(SeekFrom::End(0))?
    };

    let mut f = File::open(file_name)?;

    let mut digest = git_object::hash_sync(file_size);
    copy(&mut f, &mut digest)?;
    let value = digest.finalize();
    println!("{:x}", value);

    Ok(0)
}
