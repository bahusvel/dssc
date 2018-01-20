#![feature(iterator_step_by)]
pub mod chunked;
pub mod convolve;
pub mod flate;
mod cache;
pub mod varint;

pub trait Compressor: Send {
    fn encode(&mut self, buf: &[u8]) -> Vec<u8>;
    fn decode(&mut self, buf: &[u8]) -> Vec<u8>;
}

#[test]
pub fn full_circle() {
    use std::str::from_utf8;
    use self::convolve::ConvolveCompressor;
    use self::chunked::ChunkedCompressor;
    let mut encoder = DSSCEncoder {
        cache: Vec::new(),
        comp: &ChunkedCompressor {},
    };
    let mut decoder = DSSCDecoder {
        cache: Vec::new(),
        comp: &ChunkedCompressor {},
    };
    println!(
        "{:?}",
        from_utf8(&decoder.decode(&encoder.encode("Hello1Worlds".as_bytes())))
    );
    println!(
        "{:?}",
        from_utf8(&decoder.decode(&encoder.encode("Hello Worlds".as_bytes())))
    );
}
