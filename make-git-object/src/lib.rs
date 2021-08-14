use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io::{copy, stdout, Seek, SeekFrom};

use la_tools::git_object;

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
        eprintln!("Usage: make-git-object <file>");
        return Ok(1);
    }

    let file_name = &args[1];
    let file_size = {
        let mut f = File::open(file_name)?;
        f.seek(SeekFrom::End(0))?
    };

    let f = File::open(file_name)?;
    let mut git_obj_read = git_object::encode_sync(file_size, f);

    let mut out = stdout();

    copy(&mut git_obj_read, &mut out)?;

    Ok(0)
}
