extern crate flate2;
extern crate zstd;
extern crate zstd_safe;

use self::zstd::block;
use self::flate2::write::{DeflateDecoder, DeflateEncoder};
use self::flate2::Compression;
use super::varint::{put_uvarint, uvarint};

use std::io::{Error, ErrorKind, Read, Write};
use std::ptr;

use super::Compressor;

pub struct ZstdBlock {
    encoder: block::Compressor,
    decoder: block::Decompressor,
    level: i32,
}

impl ZstdBlock {
    fn new(level: i32, dict: Option<Vec<u8>>) -> Self {
        if dict.is_some() {
            let dict = dict.unwrap();
            ZstdBlock {
                encoder: block::Compressor::with_dict(dict.clone()),
                decoder: block::Decompressor::with_dict(dict),
                level: level,
            }
        } else {
            ZstdBlock {
                encoder: block::Compressor::new(),
                decoder: block::Decompressor::new(),
                level: level,
            }
        }
    }
}

impl Default for ZstdBlock {
    fn default() -> Self {
        ZstdBlock::new(0, None)
    }
}

impl Compressor for ZstdBlock {
    fn encode(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>) {
        let buffer_len = zstd_safe::compress_bound(in_buf.len());
        let mut varint_buf = [0; 10];
        let varint_len = put_uvarint(&mut varint_buf, in_buf.len() as u64);
        out_buf.extend_from_slice(&varint_buf[0..varint_len]);

        let original_len = out_buf.len();
        out_buf.reserve(buffer_len);

        unsafe { out_buf.set_len(original_len + buffer_len) }
        let len = self.encoder
            .compress_to_buffer(in_buf, &mut out_buf[original_len + 4..], self.level)
            .expect("Compression failed");
        unsafe { out_buf.set_len(original_len + len) }
    }

    fn decode(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>) {
        let original_len = out_buf.len();
        let (decomp_len, varint_len) = uvarint(&in_buf);
        out_buf.reserve(decomp_len as usize);

        unsafe { out_buf.set_len(original_len + decomp_len as usize) }
        self.decoder
            .decompress_to_buffer(&in_buf[varint_len as usize..], &mut out_buf[original_len..])
            .expect("Decompression failed");
    }
}


pub struct FlateStream {
    encoder: DeflateEncoder<WriteProxy<Vec<u8>>>,
    decoder: DeflateDecoder<WriteProxy<Vec<u8>>>,
}

impl Default for FlateStream {
    fn default() -> Self {
        FlateStream {
            encoder: DeflateEncoder::new(WriteProxy::new(), Compression::best()),
            decoder: DeflateDecoder::new(WriteProxy::new()),
        }
    }
}

impl Compressor for FlateStream {
    fn encode(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>) {
        let guard = self.encoder.get_mut().set(out_buf);
        self.encoder.write(&in_buf);
        self.encoder.flush();
        drop(guard);
    }
    fn decode(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>) {
        let guard = self.encoder.get_mut().set(out_buf);
        self.encoder.write(&in_buf);
        self.encoder.flush();
        drop(guard);
    }
}
/*

pub struct ReadProxy {
    inner: Option<Box<Read>>,
}

impl ReadProxy {
    pub fn new() -> Self {
        ReadProxy { inner: None }
    }

    pub fn take(&mut self) -> Option<Box<Read>> {
        self.inner.take()
    }

    pub fn set<R: Read + Sized>(&mut self, reader: Box<Read>) {
        self.inner = Some(reader);
    }
}

impl Read for ReadProxy {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        if self.inner.is_none() {
            Err(Error::new(ErrorKind::Other, "No reader in proxy"))
        } else {
            self.inner.as_mut().unwrap().read(buf)
        }
    }
} */

pub struct ProxyGuard<W: Write + Sized> {
    proxy: *mut WriteProxy<W>,
}

impl<W: Write + Sized> Drop for ProxyGuard<W> {
    fn drop(&mut self) {
        unsafe { (&mut *self.proxy).take() }
    }
}

pub struct WriteProxy<W: Write + Sized> {
    inner: *mut W,
}
unsafe impl<W: Write + Sized> Send for WriteProxy<W> {}

impl<W: Write + Sized> Write for WriteProxy<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        if self.inner.is_null() {
            Err(Error::new(ErrorKind::Other, "No writer in proxy"))
        } else {
            unsafe { (&mut *self.inner).write(buf) }
        }
    }

    fn flush(&mut self) -> Result<(), Error> {
        if self.inner.is_null() {
            Err(Error::new(ErrorKind::Other, "No writer in proxy"))
        } else {
            unsafe { (&mut *self.inner).flush() }
        }
    }
}

impl<W: Write + Sized> WriteProxy<W> {
    pub fn new() -> Self {
        WriteProxy { inner: ptr::null_mut() }
    }

    pub fn set(&mut self, writer: &mut W) -> ProxyGuard<W> {
        self.inner = writer as *mut W;
        ProxyGuard { proxy: self }
    }

    fn take(&mut self) {
        self.inner = ptr::null_mut();
    }
}
