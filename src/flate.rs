extern crate flate2;

use std::io::Write;

use super::Compressor;
use self::flate2::write::{DeflateDecoder, DeflateEncoder};
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
