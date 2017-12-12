extern crate flate2;

use std::io::Write;

use super::{Compressor, VecCache};
use super::varint::{put_uvarint, uvarint};
use self::flate2::write::{DeflateEncoder, DeflateDecoder};
use self::flate2::Compression;

pub struct FlateCompressor {
    encoder: DeflateEncoder<Vec<u8>>,
    decoder: DeflateDecoder<Vec<u8>>,
}

impl FlateCompressor {
    pub fn new() -> Self {
        FlateCompressor {
            encoder: DeflateEncoder::new(Vec::new(), Compression::best()),
            decoder: DeflateDecoder::new(Vec::new()),
        }
    }
}

impl Compressor for FlateCompressor {
    fn compress(&mut self, buf: &[u8], out_buf: &mut Vec<u8>, cache: &VecCache) -> usize {
        self.encoder.write(&buf);
        self.encoder.flush();
        out_buf.append(self.encoder.get_mut());
        0
    }
    fn decompress(&mut self, buf: &[u8], out_buf: &mut Vec<u8>, cache: &VecCache) -> usize {
        self.decoder.write(&buf);
        self.decoder.flush();
        out_buf.append(self.decoder.get_mut());
        0
    }
}
