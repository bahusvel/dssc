extern crate zstd;
extern crate flate2;

use self::zstd::stream;
use self::flate2::write::{DeflateDecoder, DeflateEncoder};
use self::flate2::Compression;

use std::io::Write;
use std::io::Read;

use super::Compressor;
/*
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
*/


pub struct FlateStream {
    encoder: DeflateEncoder<Vec<u8>>,
    decoder: DeflateDecoder<Vec<u8>>,
}

impl Default for FlateStream {
    fn default() -> Self {
        FlateStream {
            encoder: DeflateEncoder::new(Vec::new(), Compression::best()),
            decoder: DeflateDecoder::new(Vec::new()),
        }
    }
}

impl Compressor for FlateStream {
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


pub struct ReadProxy {
    inner: Option<&Read>,
}

impl Read for ReadProxy {}

pub struct WriteProxy {
    inner: Option<&Read>,
}

impl Write for WriteProxy {}
