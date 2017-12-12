extern crate flate2;

use std::io::Write;

use super::Compressor;
use self::flate2::write::{DeflateEncoder, DeflateDecoder};
use self::flate2::Compression;

pub struct FlateCompressor {
    encoder: DeflateEncoder<Vec<u8>>,
    decoder: DeflateDecoder<Vec<u8>>,
}

impl Default for FlateCompressor {
    fn default() -> Self {
        FlateCompressor {
            encoder: DeflateEncoder::new(Vec::new(), Compression::best()),
            decoder: DeflateDecoder::new(Vec::new()),
        }
    }
}

impl Compressor for FlateCompressor {
    fn encode(&mut self, buf: &[u8]) -> Vec<u8> {
        self.encoder.write(&buf);
        self.encoder.flush();
        let mut out = Vec::new();
        out.append(self.encoder.get_mut());
        out
    }
    fn decode(&mut self, buf: &[u8]) -> Vec<u8> {
        self.decoder.write(&buf);
        self.decoder.flush();
        let mut out = Vec::new();
        out.append(self.decoder.get_mut());
        out
    }
}
