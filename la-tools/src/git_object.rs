use std::cmp::min;
use std::io::{Read, Result};

struct U8ReadSync {
    buf: Vec<u8>,
    head: usize,
}

impl Read for U8ReadSync {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let amt = min(self.buf.len() - self.head, buf.len());
        buf[..amt].clone_from_slice(&self.buf[self.head..self.head + amt]);
        self.head += amt;
        Ok(amt)
    }
}

pub fn encode_sync<'a, R: Read + 'a>(size: u64, read: R) -> impl Read + 'a {
    let prefix = U8ReadSync {
        buf: format!("blob {}\0", size).as_bytes().to_vec(),
        head: 0,
    };
    flate2::read::ZlibEncoder::new(prefix.chain(read), flate2::Compression::fast())
}
