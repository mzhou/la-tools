use std::fs::File;
use std::io::{copy, stdout, Error, Read, Seek, SeekFrom, Write};
use std::iter::Iterator;

use la_tools::git_object;

fn main() -> Result<(), Error> {
    std::process::exit(try_main()?)
}

fn try_main() -> Result<i32, Error> {
    let args: Vec<String> = std::env::args().collect();
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

    /*
    let mut full = Vec::<u8>::default();
    git_obj_read.read_to_end(&mut full)?;
    out.write_all(&full)?;
    */

    Ok(0)
}
