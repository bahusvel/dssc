#![feature(iterator_step_by)]
#![feature(conservative_impl_trait)]
pub mod chunked;
pub mod other;
mod cache;
pub mod chunkmap;
pub mod varint;

pub trait Compressor: Send {
    fn encode(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>);
    fn decode(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>);
}

/*
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
*/
