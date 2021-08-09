use std::error::Error;
use std::io::{stdin, stdout, Read, Write};

use hex::FromHex;

use la_tools::git_index;

fn main() -> Result<(), Box<dyn Error>> {
    std::process::exit(try_main()?)
}

fn patch_index(mut b: &mut [u8], name: &[u8], new_size: u32, new_hash: &[u8]) -> Option<()> {
    let mut index_view = git_index::parse_mut(&mut b)?;

    for entry in &mut index_view.entries {
        if entry.name == name {
            entry.header.size.set(new_size);
            entry.header.sha1.clone_from_slice(new_hash);
        }
    }

    Some(())
}

fn try_main() -> Result<i32, Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: make-git-object <name> <size> <hash>");
        return Ok(1);
    }

    let name_str = &args[1];
    let size_str = &args[2];
    let hash_str = &args[3];

    let name = name_str.as_bytes();
    let size = size_str.parse::<u32>()?;
    let hash = git_index::Hash::from_hex(hash_str)?;

    let mut data = Vec::<u8>::new();
    stdin().read_to_end(&mut data)?;

    if patch_index(&mut data, name, size, &hash) == None {
        eprintln!("Parse error");
        return Ok(2);
    }

    stdout().write_all(&data)?;

    Ok(0)
}
