extern crate flate2;
extern crate zstd;

use self::zstd::stream;
use self::flate2::write::{DeflateDecoder, DeflateEncoder};
use self::flate2::Compression;

use std::io::{Error, ErrorKind, Read, Write};
use std::ptr;

use super::Compressor;

pub struct ZstdStream {
    encoder: stream::Encoder<Vec<u8>>,
    decoder: stream::Decoder<Vec<u8>>,
}

impl ZstdStream {
    fn new(level: i32) -> Self {
        ZstdCompressor {
            encoder: stream::Encoder::new(Vec::new(), level),
            decoder: stream::Decoder::new(Vec::new()),
        }
    }
}

impl Default for ZstdStream {
    fn default() -> Self {
        ZstdStream::new(0);
    }
}

impl Compressor for ZstdStream {
    fn encode(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>) {
        self.encoder.write(&in_buf);
        self.encoder.flush();
        out_buf.append(self.encoder.get_mut());
    }
    fn decode(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>) {
        self.decoder.write(&in_buf);
        self.decoder.flush();
        out_buf.append(self.decoder.get_mut());
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
        WriteProxy {
            inner: ptr::null_mut(),
        }
    }

    pub fn set(&mut self, writer: &mut W) -> ProxyGuard<W> {
        self.inner = writer as *mut W;
        ProxyGuard { proxy: self }
    }

    fn take(&mut self) {
        self.inner = ptr::null_mut();
    }
}
