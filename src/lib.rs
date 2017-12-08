pub mod varint;

use std::io::{Write, Read};
use std::cmp::Ordering;
use std::str::from_utf8;
use self::varint::{put_uvarint, uvarint};

const INSERT_THRESHOLD: usize = 1000;
const CACHE_SIZE: usize = 256;

#[derive(Eq)]
struct CacheEntry {
    hits: usize,
    data: Vec<u8>,
}

impl Ord for CacheEntry {
    fn cmp(&self, other: &CacheEntry) -> Ordering {
        self.hits.cmp(&other.hits)
    }
}

impl PartialOrd for CacheEntry {
    fn partial_cmp(&self, other: &CacheEntry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for CacheEntry {
    fn eq(&self, other: &CacheEntry) -> bool {
        self.hits == other.hits
    }
}

type VecCache = Vec<CacheEntry>;

trait DSSCache {
    fn cache_insert(&mut self, buf: &[u8]);
}

pub struct DSSCEncoder {
    cache: VecCache,
}

impl DSSCache for VecCache {
    fn cache_insert(&mut self, buf: &[u8]) {
        self.sort_unstable();
        let len = self.len();
        if len == CACHE_SIZE {
            self[len - 1] = CacheEntry {
                hits: 0,
                data: buf.to_vec(),
            }
        } else {
            self.push(CacheEntry {
                hits: 0,
                data: buf.to_vec(),
            })
        }
    }
}

impl DSSCEncoder {
    pub fn new() -> DSSCEncoder {
        DSSCEncoder { cache: Vec::new() }
    }

    pub fn encode(&mut self, buf: &[u8]) -> Vec<u8> {
        let mut best = (0, (0, <usize>::max_value()));
        let delta;
        if self.cache.len() != 0 {
            for entry in 0..self.cache.len() {
                let cres = DSSCEncoder::convolve(&buf, &self.cache[entry].data);
                if cres.1 < (best.1).1 {
                    best = (entry, cres)
                }
            }
            delta = DSSCEncoder::delta(&buf, &self.cache[best.0].data, (best.1).0);
            self.cache[best.0].hits += 1;
            if (best.1).1 > INSERT_THRESHOLD {
                self.cache.cache_insert(&buf);
            }
        /*
            println!(
                "delta: {:?} from {:?}@{}",
                delta,
                &self.cache[best.0].data,
                (best.1).0
            );
            */
        } else {
            self.cache.cache_insert(&buf);
            delta = buf.to_vec();
        }
        let mut offset_buf = [0; 10];
        let offset_len = put_uvarint(&mut offset_buf, (best.1).0 as u64);
        let mut comp = vec![best.0 as u8];
        comp.extend_from_slice(&offset_buf[0..offset_len]);
        DSSCEncoder::zrle(&delta, &mut comp);
        /*
        println!("comp: {:?}", comp);
        */
        eprintln!(
            "compression ratio {}/{} = {}",
            comp.len(),
            buf.len(),
            comp.len() as f32 / buf.len() as f32
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
        let mut best = (0, <usize>::max_value());
        for offset in 0..haystack.len() {
            let overrun = (offset + needle.len()) as isize - haystack.len() as isize;
            let mut score = 0usize;
            let slice = if overrun > 0 {
                score += needle[needle.len() - overrun as usize..needle.len()]
                    .iter()
                    .fold(0, |acc, &x| acc + x as usize);
                &haystack[offset..offset + (needle.len() - overrun as usize)]
            } else {
                &haystack[offset..offset + needle.len()]
            };
            score += slice.iter().zip(needle).fold(0, |acc, (&x, &y)| {
                acc + (x ^ y) as usize
            });
            if score < best.1 {
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
        if sum > INSERT_THRESHOLD {
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
