use std::fs::File;
use std::io::{copy, Error, Seek, SeekFrom};
use std::iter::Iterator;

use la_tools::git_object;
use la_tools::git_object::Digest;

fn main() -> Result<(), Error> {
    std::process::exit(try_main()?)
}

fn try_main() -> Result<i32, Error> {
    let args: Vec<String> = std::env::args().collect();
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
