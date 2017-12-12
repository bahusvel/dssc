#![feature(iterator_step_by)]

pub mod varint;
mod cache;
pub mod chunked;
pub mod convolve;
pub mod flate;

use self::cache::{VecCache, DSSCache};

pub struct DSSCEncoder<'a> {
    cache: VecCache,
    comp: &'a mut Compressor,
    insert_threshold: f32,
}

pub trait Compressor {
    fn compress(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>, cache: &VecCache) -> usize;
    fn decompress(&mut self, in_buf: &[u8], out_buf: &mut Vec<u8>, cache: &VecCache) -> usize;
}

impl<'a> DSSCEncoder<'a> {
    pub fn new(comp: &'a mut Compressor, insert_threshold: f32) -> DSSCEncoder {
        DSSCEncoder {
            cache: Vec::new(),
            comp: comp,
            insert_threshold: insert_threshold,
        }
    }

    pub fn encode(&mut self, buf: &[u8]) -> Vec<u8> {
        let mut out_buf = Vec::new();
        let hit_index = self.comp.compress(buf, &mut out_buf, &self.cache);

        if self.cache.len() != 0 {
            self.cache[hit_index].hits += 1;
        }
        let cr = out_buf.len() as f32 / buf.len() as f32;
        eprintln!(
            "cr {}/{}={} cache entry {}",
            out_buf.len(),
            buf.len(),
            cr,
            hit_index,
        );
        if cr > self.insert_threshold {
            self.cache.cache_insert(&buf);
        }

        out_buf
    }
}

pub struct DSSCDecoder<'a> {
    cache: VecCache,
    comp: &'a mut Compressor,
    insert_threshold: f32,
}

impl<'a> DSSCDecoder<'a> {
    pub fn new(comp: &'a mut Compressor, insert_threshold: f32) -> DSSCDecoder {
        DSSCDecoder {
            cache: Vec::new(),
            comp: comp,
            insert_threshold: insert_threshold,
        }
    }

    pub fn decode(&mut self, buf: &[u8]) -> Vec<u8> {
        let mut out_buf = Vec::new();
        let hit_index = self.comp.decompress(buf, &mut out_buf, &self.cache);

        if self.cache.len() != 0 {
            self.cache[hit_index].hits += 1;
        }
        let cr = buf.len() as f32 / out_buf.len() as f32;
        if cr > self.insert_threshold {
            self.cache.cache_insert(&out_buf);
        }

        out_buf
    }
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
