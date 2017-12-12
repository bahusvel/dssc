use super::{Compressor, VecCache};
use super::varint::{put_uvarint, uvarint};

pub struct ConvolveCompressor {}

impl Compressor for ConvolveCompressor {
    fn compress(&mut self, buf: &[u8], out_buf: &mut Vec<u8>, cache: &VecCache) -> usize {
        let mut best = (0, (0, 0));
        let delta = if cache.len() != 0 {
            for entry in 0..cache.len() {
                let cres = convolve(&buf, &cache[entry].data);
                if cres.1 > (best.1).1 {
                    best = (entry, cres)
                }
            }
            delta(&buf, &cache[best.0].data, (best.1).0)
        } else {
            buf.to_vec()
        };

        out_buf.push(best.0 as u8);

        let mut offset_buf = [0; 10];
        let offset_len = put_uvarint(&mut offset_buf, (best.1).0 as u64);
        out_buf.extend_from_slice(&offset_buf[0..offset_len]);

        zrle(&delta, out_buf);
        best.0
    }
    fn decompress(&mut self, buf: &[u8], out_buf: &mut Vec<u8>, cache: &VecCache) -> usize {
        let (offset, offset_len) = uvarint(&buf[1..]);
        if offset_len <= 0 {
            panic!("Offset is wrong")
        }
        let mut delta = zrld(&buf[1 + offset_len as usize..]);
        if cache.len() == 0 {
            out_buf.append(&mut delta);
            return 0;
        }
        undelta(&mut delta, &cache[buf[0] as usize].data, offset as usize);

        out_buf.append(&mut delta);
        return buf[0] as usize;
    }
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
            |acc, (&x, &y)| if x ^ y == 0 {
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
