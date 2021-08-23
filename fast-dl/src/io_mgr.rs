use std::fs::OpenOptions;
use std::io::Result as IoResult;
use std::path::Path;

use memmap2::{MmapMut, MmapOptions};

pub fn create_mmap<P: AsRef<Path>>(path: P, offset: u64, len: usize) -> IoResult<MmapMut> {
    let f = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)?;
    let mut opts = MmapOptions::new();
    opts.offset(offset).len(len);
    let m = unsafe { opts.map_mut(&f) }?;
    Ok(m)
}
