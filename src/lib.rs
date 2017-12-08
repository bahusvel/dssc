pub mod varint;
mod cache;

use std::io::{Write, Read};
use std::str::from_utf8;
use self::varint::{put_uvarint, uvarint};
use self::cache::{VecCache, DSSCache};

const INSERT_THRESHOLD: f32 = 0.5;

pub struct DSSCEncoder {
    cache: VecCache,
}

impl DSSCEncoder {
    pub fn new() -> DSSCEncoder {
        DSSCEncoder { cache: Vec::new() }
    }

    pub fn encode(&mut self, buf: &[u8]) -> Vec<u8> {
        let mut best = (0, (0, 0));
        let delta;
        if self.cache.len() != 0 {
            for entry in 0..self.cache.len() {
                let cres = DSSCEncoder::convolve(&buf, &self.cache[entry].data);
                if cres.1 > (best.1).1 {
                    best = (entry, cres)
                }
            }
            delta = DSSCEncoder::delta(&buf, &self.cache[best.0].data, (best.1).0);
            self.cache[best.0].hits += 1;
        } else {
            delta = buf.to_vec();
        }

        let mut comp = vec![best.0 as u8];

        let mut offset_buf = [0; 10];
        let offset_len = put_uvarint(&mut offset_buf, (best.1).0 as u64);
        comp.extend_from_slice(&offset_buf[0..offset_len]);

        DSSCEncoder::zrle(&delta, &mut comp);
        let cr = comp.len() as f32 / buf.len() as f32;

        if cr > INSERT_THRESHOLD {
            self.cache.cache_insert(&buf);
        }

        eprintln!(
            "cr {}/{}={} offset {} matched {}",
            comp.len(),
            buf.len(),
            cr,
            (best.1).0,
            (best.1).1,
        );

        comp
    }

    fn delta(buf: &[u8], deltasource: &[u8], offset: usize) -> Vec<u8> {
        let overrun = (offset + buf.len()) as isize - deltasource.len() as isize;
        let slice = if overrun > 0 {
            &deltasource[offset..offset + (buf.len() - overrun as usize)]
        } else {
            &deltasource[offset..offset + buf.len()]
        };
        let mut d: Vec<u8> = slice.iter().zip(buf).map(|(x, y)| x ^ y).collect();
        if overrun > 0 {
            d.extend_from_slice(&buf[buf.len() - overrun as usize..buf.len()]);
        }
        d
    }

    fn zrle(buf: &[u8], out: &mut Vec<u8>) {
        let mut zcount = 0u8; // FIXME I need to handle cases with more than 255 zeroes
        for i in 0..buf.len() {
            if buf[i] == 0 {
                zcount += 1;
            } else if zcount > 0 {
                out.push(0);
                out.push(zcount);
                out.push(buf[i]);
                zcount = 0;
            } else {
                out.push(buf[i]);
            }
        }
        if zcount != 0 {
            out.push(0);
            out.push(zcount);
        }
    }

    //return (offset, score)
    fn convolve(needle: &[u8], haystack: &[u8]) -> (usize, usize) {
        let mut best = (0, 0);
        for offset in 0..haystack.len() {
            let overrun = (offset + needle.len()) as isize - haystack.len() as isize;
            let mut score = 0usize;
            let slice = if overrun > 0 {
                &haystack[offset..offset + (needle.len() - overrun as usize)]
            } else {
                &haystack[offset..offset + needle.len()]
            };
            score += slice.iter().zip(needle).fold(
                0,
                |acc, (&x, &y)| if (x ^ y == 0) {
                    acc + 1
                } else {
                    acc
                },
            );
            if score > best.1 {
                best = (offset, score)
            }
        }
        best
    }
}

pub struct DSSCDecoder {
    cache: VecCache,
}

impl DSSCDecoder {
    pub fn new() -> DSSCDecoder {
        DSSCDecoder { cache: Vec::new() }
    }

    pub fn decode(&mut self, buf: &[u8]) -> Vec<u8> {
        let (offset, offset_len) = uvarint(&buf[1..]);
        if offset_len <= 0 {
            panic!("Offset is wrong")
        }
        let mut delta = DSSCDecoder::zrld(&buf[1 + offset_len as usize..]);
        if self.cache.len() == 0 {
            self.cache.cache_insert(&delta);
            return delta;
        }
        let sum = delta.iter().fold(0, |acc, &x| acc + x as usize);
        DSSCDecoder::undelta(
            &mut delta,
            &self.cache[buf[0] as usize].data,
            offset as usize,
        );
        self.cache[buf[0] as usize].hits += 1;
        let cr = buf.len() as f32 / delta.len() as f32;
        if cr > INSERT_THRESHOLD {
            self.cache.cache_insert(&delta);
        }
        delta
    }

    fn undelta(buf: &mut [u8], deltasource: &[u8], offset: usize) {
        let delta_len = if deltasource.len() - offset < buf.len() {
            deltasource.len() - offset
        } else {
            buf.len()
        };
        for i in 0..delta_len {
            buf[i] ^= deltasource[offset + i];
        }
    }

    fn zrld(buf: &[u8]) -> Vec<u8> {
        let mut was_zero = false;
        let mut out = Vec::new();
        for i in 0..buf.len() {
            if buf[i] == 0 {
                was_zero = true;
            } else if was_zero {
                for _ in 0..buf[i] {
                    out.push(0)
                }
                was_zero = false;
            } else {
                out.push(buf[i]);
            }
        }
        out
    }
}

#[test]
pub fn encode() {
    let mut encoder = DSSCEncoder { cache: Vec::new() };
    encoder.encode("Hello1World".as_bytes());
    encoder.encode("Hello World".as_bytes());
}

#[test]
pub fn zrld() {
    println!("zrld {:?}", DSSCDecoder::zrld(&[0, 5, 17, 0, 5]));
}

#[test]
pub fn undelta() {
    let mut buf = [0, 0, 0, 0, 0, 17, 0, 0, 0, 0, 0];
    DSSCDecoder::undelta(
        &mut buf,
        &[72, 101, 108, 108, 111, 49, 87, 111, 114, 108, 100],
        0,
    );
    println!("undelta {:?}", &buf);
}

#[test]
pub fn full_circle() {
    let mut encoder = DSSCEncoder { cache: Vec::new() };
    let mut decoder = DSSCDecoder { cache: Vec::new() };
    println!(
        "{:?}",
        from_utf8(&decoder.decode(&encoder.encode("Hello1World".as_bytes())))
    );
    println!(
        "{:?}",
        from_utf8(&decoder.decode(&encoder.encode("Hello World".as_bytes())))
    );
}
