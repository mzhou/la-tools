use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Result as IoResult;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use memmap2::{MmapMut, MmapOptions};

pub struct IoMgr {
    d: HashMap<IoMgrKey, IoMgrEntry>,
}

struct IoMgrEntry {
    value: Weak<MappedFile>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IoMgrKey {
    path: PathBuf,
    offset: u64,
    len: usize,
}

pub struct MappedFile {
    m: MmapMut,
}

impl IoMgr {
    pub fn acquire(&mut self, key: &IoMgrKey) -> IoResult<Arc<MappedFile>> {
        if let Some(v) = self.d.get_mut(key) {
            let arc_option = v.value.upgrade();
            return Ok(match arc_option {
                None => {
                    let arc = Arc::new(MappedFile::create(&key.path, key.offset, key.len)?);
                    v.value = Arc::downgrade(&arc);
                    arc
                }
                Some(arc) => arc,
            });
        }
        let arc = Arc::new(MappedFile::create(&key.path, key.offset, key.len)?);
        let entry = IoMgrEntry {
            value: Arc::downgrade(&arc),
        };
        self.d.insert(key.clone(), entry);
        Ok(arc)
    }

    pub fn new() -> Self {
        Self { d: HashMap::new() }
    }
}

impl MappedFile {
    pub fn create<P: AsRef<Path>>(path: P, offset: u64, len: usize) -> IoResult<Self> {
        let f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)?;
        let mut opts = MmapOptions::new();
        opts.offset(offset).len(len);
        Ok(Self {
            m: unsafe { opts.map_mut(&f) }?,
        })
    }

    pub fn len(&self) -> usize {
        self.m.len()
    }

    pub fn mutate<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [u8]) -> (),
    {
        f(&mut self.m)
    }
}
