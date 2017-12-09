#![feature(iterator_step_by)]

pub mod varint;
mod cache;

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

const CHUNK_SIZE: usize = 4;

// for each haystack returns a list of indexes where each chunk of needle was found, 0 means not found
fn chunk_match(needle: &[u8], haystacks: &Vec<Vec<u8>>) -> Vec<Vec<usize>> {
    let mut results = Vec::new();
    for haystack in haystacks {
        results.push(
            needle
                .chunks(CHUNK_SIZE)
                .map(|chunk| {
                    for hi in 0..haystack.len() - CHUNK_SIZE {
                        if &haystack[hi..hi + CHUNK_SIZE] == chunk {
                            return hi + 1;
                        }
                    }
                    return 0;
                })
                .collect(),
        )
    }
    return results;
}

#[derive(Debug, PartialEq)]
enum BlockType {
    Delta,
    Original,
}

#[derive(Debug)]
struct Block {
    block_type: BlockType,
    needle_off: usize,
    offset: usize,
    len: usize,
}

impl Block {
    fn fit(&mut self, needle: &[u8], haystack: &Vec<u8>) {
        if self.block_type != BlockType::Delta {
            return;
        }
        let mut bi = 0;
        let mut od = 0;
        let mut fi = self.len;
        while (self.needle_off - bi) > 0 && (self.offset - bi) > 0 &&
            needle[self.needle_off - bi] == haystack[self.offset - bi]
        {
            od += 1;
            self.len += 1;
            bi += 1;
        }
        while (self.needle_off + fi) < needle.len() && (self.offset + fi) < haystack.len() &&
            needle[self.needle_off + fi] == haystack[self.offset + fi]
        {
            self.len += 1;
            fi += 1;
        }
        self.offset -= od;
        self.needle_off -= od;
    }

    fn encode(&self, needle: &[u8], buf: &mut Vec<u8>) {
        let mut varint_buf = [0; 10];
        match self.block_type {
            BlockType::Delta => {
                let offset_len = put_uvarint(&mut varint_buf, (self.offset + 1) as u64);
                buf.extend_from_slice(&varint_buf[0..offset_len]);
                let len_len = put_uvarint(&mut varint_buf, (self.len) as u64);
                buf.extend_from_slice(&varint_buf[0..len_len]);
            }
            BlockType::Original => {
                buf.push(0 as u8);
                let len_len = put_uvarint(&mut varint_buf, (self.len) as u64);
                buf.extend_from_slice(&varint_buf[0..len_len]);
                buf.extend_from_slice(&needle[self.needle_off..self.needle_off + self.len])
            }
        }
    }

    fn decode_blocks(buf: &[u8], haystack: &Vec<u8>) -> Vec<u8> {
        let mut data = Vec::new();
        let mut bi = 0;
        while bi < buf.len() {
            if buf[bi] == 0 {
                //original
                bi += 1;
                let (len, len_len) = uvarint(&buf[bi..]);
                if len_len <= 0 {
                    panic!("Something is wrong with length varint");
                }
                bi += len_len as usize;
                data.extend_from_slice(&buf[bi..bi + len as usize]);
                bi += len as usize;
            } else {
                let (mut offset, offset_len) = uvarint(&buf[bi..]);
                if offset_len <= 0 {
                    panic!("Something is wrong with offset varint");
                }
                offset -= 1;
                bi += offset_len as usize;
                let (len, len_len) = uvarint(&buf[bi..]);
                if len_len <= 0 {
                    panic!("Something is wrong with length varint");
                }
                bi += len_len as usize;
                data.extend_from_slice(&haystack[offset as usize..offset as usize + len as usize]);
            }
        }
        data
    }
}


fn expand_blocks(needle: &[u8], haystack: &Vec<u8>, result: &Vec<usize>) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut ri = 0;
    while ri < result.len() {
        if result[ri] != 0 {
            let bi = ri;
            let (offset, mut len) = (result[ri] - 1, CHUNK_SIZE);
            ri += 1;
            while ri < result.len() && result[ri] == result[ri - 1] + CHUNK_SIZE {
                len += CHUNK_SIZE;
                ri += 1;
            }
            let mut block = Block {
                block_type: BlockType::Delta,
                needle_off: bi * CHUNK_SIZE,
                offset: offset,
                len: len,
            };
            block.fit(needle, haystack);
            blocks.push(block);
        } else {
            let block = if let Some(last_block) = blocks.last() {
                Block {
                    block_type: BlockType::Original,
                    needle_off: last_block.needle_off + last_block.len,
                    offset: 0,
                    len: 0,
                }
            } else {
                Block {
                    block_type: BlockType::Original,
                    needle_off: 0,
                    offset: 0,
                    len: 0,
                }
            };
            blocks.push(block);
            while ri < result.len() && result[ri] == 0 {
                ri += 1;
            }
        }
    }
    for i in 0..blocks.len() - 1 {
        if blocks[i].block_type == BlockType::Original {
            blocks[i].len = blocks[i + 1].needle_off - blocks[i].needle_off;
        }
    }
    if let Some(ref mut last_block) = blocks.last_mut() {
        if last_block.block_type == BlockType::Original {
            last_block.len = needle.len() - last_block.needle_off;
        }
    }
    blocks.retain(|b| b.len != 0);
    blocks
}

fn chunk_compressor(needle: &[u8], haystacks: &Vec<Vec<u8>>) -> Vec<u8> {
    let matches = chunk_match(needle, haystacks);
    println!("matches {:?}", matches);
    let max: usize = matches
        .iter()
        .map(|m| m.iter().filter(|&&o| o != 0).count())
        .max()
        .expect("haystacks are empty");
    let mut max_block = (0, 0, None);
    for (hi, result) in matches.iter().enumerate() {
        if result.iter().filter(|&&o| o != 0).count() != max {
            continue;
        }
        let blocks = expand_blocks(needle, &haystacks[hi], result);
        let score = blocks
            .iter()
            .filter(|b| b.block_type == BlockType::Delta)
            .map(|b| b.len)
            .sum();
        if max_block.0 <= score {
            max_block = (score, hi, Some(blocks))
        }
    }
    println!("{:?}", max_block.2);
    let mut buf = vec![max_block.1 as u8];
    for block in max_block.2.expect("No candidate was found") {
        block.encode(needle, &mut buf);
    }
    println!("{:?}", needle);
    println!("{:?}", buf);
    buf
}

fn chunk_decompressor(buf: &[u8], haystacks: &Vec<Vec<u8>>) -> Vec<u8> {
    let hi = buf[0] as usize;
    Block::decode_blocks(&buf[1..], &haystacks[hi])
}

#[test]
pub fn chunk_test() {
    let haystacks = vec![
        "Hello Denis Worlds".as_bytes().to_vec(),
        "Test Worlds".as_bytes().to_vec(),
        "Test Bananas".as_bytes().to_vec(),
    ];
    let compressed = chunk_compressor("Hello Test Worlds".as_bytes(), &haystacks);
    println!("{:?}", chunk_decompressor(&compressed, &haystacks));
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
