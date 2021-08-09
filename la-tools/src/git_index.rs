use std::mem;

use byteorder::NetworkEndian;
use static_assertions::assert_eq_size;
use zerocopy::byteorder::{I32, U16, U32};
use zerocopy::{AsBytes, FromBytes, LayoutVerified, Unaligned};

// https://git-scm.com/docs/index-format

#[derive(AsBytes, Debug, FromBytes, Unaligned)]
#[repr(C)]
pub struct FileHeader {
    pub magic: [u8; 4],
    pub version: U32<NetworkEndian>,
    pub entry_count: U32<NetworkEndian>,
}

assert_eq_size!(FileHeader, [u8; 12]);

#[derive(AsBytes, Debug, FromBytes, Unaligned)]
#[repr(C)]
pub struct EntryHeader {
    pub ctime_s: I32<NetworkEndian>,
    pub ctime_ns: I32<NetworkEndian>,
    pub mtime_s: I32<NetworkEndian>,
    pub mtime_ns: I32<NetworkEndian>,
    pub dev: U32<NetworkEndian>,
    pub ino: U32<NetworkEndian>,
    pub mode: U32<NetworkEndian>,
    pub uid: U32<NetworkEndian>,
    pub gid: U32<NetworkEndian>,
    pub size: U32<NetworkEndian>,
    pub sha1: [u8; 20],
    pub flags: U16<NetworkEndian>,
}

assert_eq_size!(EntryHeader, [u8; 62]);

#[derive(Debug)]
pub struct ViewEntry<'a> {
    pub header: &'a EntryHeader,
    pub name: &'a [u8],
}

#[derive(Debug)]
pub struct ViewEntryMut<'a> {
    pub header: &'a mut EntryHeader,
    pub name: &'a mut [u8],
}

#[derive(Debug)]
pub struct View<'a> {
    pub header: &'a FileHeader,
    pub entries: Vec<ViewEntry<'a>>,
    pub footer: &'a [u8],
}

#[derive(Debug)]
pub struct ViewMut<'a> {
    pub header: &'a mut FileHeader,
    pub entries: Vec<ViewEntryMut<'a>>,
    pub footer: &'a mut [u8],
}

// based on BufferView from fuchsia packet crate
struct SliceReader<'a>(&'a [u8]);

impl<'a> SliceReader<'a> {
    fn iter(&self) -> std::slice::Iter<u8> {
        self.0.iter()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn take_front(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() >= n {
            let (prefix, rest) = mem::replace(&mut self.0, &[]).split_at(n);
            self.0 = rest;
            Some(prefix)
        } else {
            None
        }
    }

    fn take_obj_front<T: FromBytes + Unaligned>(&mut self) -> Option<&'a T> {
        let head_bytes = self.take_front(mem::size_of::<T>())?;
        Some(LayoutVerified::<&[u8], T>::new_unaligned(head_bytes)?.into_ref())
    }
}

struct SliceReaderMut<'a>(&'a mut [u8]);

impl<'a> SliceReaderMut<'a> {
    fn iter(&self) -> std::slice::Iter<u8> {
        self.0.iter()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn take_front(&mut self, n: usize) -> Option<&'a mut [u8]> {
        if self.0.len() >= n {
            let (prefix, rest) = mem::replace(&mut self.0, &mut []).split_at_mut(n);
            self.0 = rest;
            Some(prefix)
        } else {
            None
        }
    }

    fn take_obj_front<T: AsBytes + FromBytes + Unaligned>(&mut self) -> Option<&'a mut T> {
        let head_bytes = self.take_front(mem::size_of::<T>())?;
        Some(LayoutVerified::<&mut [u8], T>::new_unaligned(head_bytes)?.into_mut())
    }
}

fn round_up(x: usize, increment: usize) -> usize {
    (x + increment - 1) / increment * increment
}

fn take_name<'a>(reader: &mut SliceReader<'a>) -> Option<&'a [u8]> {
    let nul_pos = reader.iter().position(|&x| x == b'\0')?;
    // size of entire entry including name is NUL padded to be multiple of 8
    let header_size = mem::size_of::<EntryHeader>();
    let size = round_up(nul_pos + header_size + 1, 8) - header_size;
    let (text_bytes, nul_bytes) = reader.take_front(size)?.split_at(nul_pos);
    if !nul_bytes.iter().all(|&x| x == b'\0') {
        return None;
    }
    Some(text_bytes)
}

fn take_name_mut<'a>(reader: &mut SliceReaderMut<'a>) -> Option<&'a mut [u8]> {
    let nul_pos = reader.iter().position(|&x| x == b'\0')?;
    // size of entire entry including name is NUL padded to be multiple of 8
    let header_size = mem::size_of::<EntryHeader>();
    let size = round_up(nul_pos + header_size + 1, 8) - header_size;
    let (text_bytes, nul_bytes) = reader.take_front(size)?.split_at_mut(nul_pos);
    if !nul_bytes.iter().all(|&x| x == b'\0') {
        return None;
    }
    Some(text_bytes)
}

pub fn parse<'a>(bin: &'a [u8]) -> Option<View<'a>> {
    let mut reader = SliceReader(&bin);
    let header = reader.take_obj_front::<FileHeader>()?;
    if header.version.get() != 2 {
        return None;
    }
    let mut entries = Vec::<ViewEntry<'a>>::new();
    for _ in 0..header.entry_count.get() {
        let entry_header = reader.take_obj_front::<EntryHeader>()?;
        let name = take_name(&mut reader)?;
        entries.push(ViewEntry::<'a> {
            header: entry_header,
            name,
        });
    }
    let footer = reader.take_front(reader.len())?;
    Some(View::<'a> {
        header,
        entries,
        footer,
    })
}

pub fn parse_mut<'a>(bin: &'a mut [u8]) -> Option<ViewMut<'a>> {
    let mut reader = SliceReaderMut(bin);
    let header = reader.take_obj_front::<FileHeader>()?;
    if header.version.get() != 2 {
        return None;
    }
    let mut entries = Vec::<ViewEntryMut<'a>>::new();
    for _ in 0..header.entry_count.get() {
        let entry_header = reader.take_obj_front::<EntryHeader>()?;
        let name = take_name_mut(&mut reader)?;
        entries.push(ViewEntryMut::<'a> {
            header: entry_header,
            name,
        });
    }
    let footer = reader.take_front(reader.len())?;
    Some(ViewMut::<'a> {
        header,
        entries,
        footer,
    })
}
