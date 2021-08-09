use std::cmp::min;
use std::io::{Error, ErrorKind, Read, Result};

use flate2::read::{ZlibDecoder, ZlibEncoder};
use flate2::Compression;

pub struct GitObjectReadSync<R: Read> {
    header_skipped: bool,
    r: R,
}

struct U8ReadSync {
    buf: Vec<u8>,
    head: usize,
}

impl<R: Read> Read for GitObjectReadSync<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.header_skipped {
            {
                let mut buf = [0u8; 5];
                self.r.read_exact(&mut buf)?;
                if &buf != b"blob " {
                    return Err(Error::new(ErrorKind::InvalidData, "git_object bad magic"));
                }
            }
            loop {
                let mut buf = [0u8; 1];
                self.r.read_exact(&mut buf)?;
                if buf[0] == b'\0' {
                    break;
                }
                if !(buf[0] >= b'0' && buf[0] <= b'9') {
                    return Err(Error::new(ErrorKind::InvalidData, "git_object bad size"));
                }
            }
            self.header_skipped = true;
        }
        self.r.read(buf)
    }
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
    ZlibEncoder::new(prefix.chain(read), Compression::fast())
}

pub fn decode_sync<'a, R: Read + 'a>(read: R) -> impl Read + 'a {
    GitObjectReadSync {
        header_skipped: false,
        r: ZlibDecoder::new(read),
    }
}
