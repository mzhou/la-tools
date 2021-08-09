use std::io::{copy, stdin, stdout, Result};

use la_tools::git_object;

fn main() -> Result<()> {
    let in_file = stdin();
    let mut out_file = stdout();

    let mut decode_read = git_object::decode_sync(in_file);

    copy(&mut decode_read, &mut out_file)?;

    Ok(())
}
