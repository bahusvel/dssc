extern crate fnv;
extern crate slab;

use self::fnv::FnvHashMap;
use self::fnv::FnvHashSet;
use self::slab::Slab;

use super::varint::{put_uvarint, uvarint};
use super::Compressor;

use std::mem::transmute;
use std::fmt;

const EDEN_SIZE: usize = 10;
const CACHE_SIZE: usize = 255 - EDEN_SIZE;
const CHUNK_SIZE: usize = 4;

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub struct Match {
    line: usize,
    offset: usize,
}

impl Match {
    fn next_nth_chunk(&self, n: usize) -> Self {
        Match {
            line: self.line,
            offset: self.offset + n * CHUNK_SIZE,
        }
    }
    fn to_block(&self, needle_off: usize, len: usize) -> Block {
        Block {
            block_type: BlockType::Delta {
                line: self.line,
                offset: self.offset,
            },
            needle_off: needle_off,
            len: len,
        }
    }
}

#[derive(Debug, PartialEq)]
enum BlockType {
    Delta { line: usize, offset: usize },
    Original,
}

struct Block {
    block_type: BlockType,
    needle_off: usize,
    len: usize,
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?}[{}-{})",
            self.block_type,
            self.needle_off,
            self.needle_off + self.len
        )
    }
}

impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (self as &fmt::Display).fmt(f)
    }
}

impl Block {
    fn fit(&mut self, needle: &[u8], haystack: &Vec<u8>, left_bound: usize, right_bound: usize) {
        if self.block_type == BlockType::Original {
            return;
        }
        let (line, offset) = if let BlockType::Delta { line, offset } = self.block_type {
            (line, offset)
        } else {
            (0, 0)
        };

        let mut bi = 1;
        let mut fi = self.len;

        //backward search
        while self.needle_off > bi + left_bound && offset >= bi &&
            needle[self.needle_off - bi] == haystack[offset - bi]
        {
            bi += 1;
        }
        bi -= 1;

        //forward search
        while (self.needle_off + fi) < right_bound && (offset + fi) < haystack.len() &&
            needle[self.needle_off + fi] == haystack[offset + fi]
        {
            fi += 1;
        }

        self.len += bi + (fi - self.len);
        self.needle_off -= bi;
        self.block_type = BlockType::Delta {
            line: line,
            offset: offset - bi,
        }
    }

    fn encode(&self, needle: &[u8], buf: &mut Vec<u8>) {
        let mut varint_buf = [0; 10];
        match self.block_type {
            BlockType::Delta { line, offset } => {
                //eprintln!("{},{},{}", line, self.len, offset);
                let varint_len = put_uvarint(&mut varint_buf, (line + 1) as u64);
                buf.extend_from_slice(&varint_buf[0..varint_len]);
                let varint_len = put_uvarint(&mut varint_buf, self.len as u64);
                buf.extend_from_slice(&varint_buf[0..varint_len]);
                let varint_len = put_uvarint(&mut varint_buf, offset as u64);
                buf.extend_from_slice(&varint_buf[0..varint_len]);
            }
            BlockType::Original => {
                buf.push(0 as u8);
                let varint_len = put_uvarint(&mut varint_buf, self.len as u64);
                buf.extend_from_slice(&varint_buf[0..varint_len]);
                buf.extend_from_slice(&needle[self.needle_off..self.needle_off + self.len])
            }
        }
    }

    fn decode<'a>(buf: &'a [u8], cache: &'a mut ChunkMap) -> (&'a [u8], usize) {
        let mut i = 0;
        let (line, varint_len) = uvarint(&buf);
        assert!(varint_len > 0);
        i += varint_len as usize;
        let (length, varint_len) = uvarint(&buf[i..]);
        assert!(varint_len > 0);
        let length = length as usize;
        i += varint_len as usize;
        if line == 0 {
            (&buf[i..i + length], i + length)
        } else {
            let line = (line - 1) as usize;
            let (offset, varint_len) = uvarint(&buf[i..]);
            assert!(varint_len > 0);
            let offset = offset as usize;
            i += varint_len as usize;
            //eprintln!("{},{},{}", line, length, offset);
            cache.entries[line].1 += length;
            (&cache.entries[line].0[offset..offset + length], i)
        }
    }
}

pub struct ChunkMap {
    map: FnvHashMap<u32, FnvHashSet<Match>>,
    entries: Slab<(Vec<u8>, usize)>,
    insert_threshold: f32,
}

pub fn chunk_to_u32(chunk: [u8; 4]) -> u32 {
    unsafe { transmute::<[u8; 4], u32>(chunk) }
}

pub fn slice_to_u32(s: &[u8]) -> u32 {
    assert!(s.len() == 4);
    let chunk = [s[0], s[1], s[2], s[3]];
    chunk_to_u32(chunk)
}

impl ChunkMap {
    pub fn new(insert_threshold: f32) -> Self {
        ChunkMap {
            map: FnvHashMap::default(),
            entries: Slab::with_capacity(CACHE_SIZE),
            insert_threshold,
        }
    }
    fn insert(&mut self, entry: Vec<u8>) {
        if self.entries.len() == CACHE_SIZE {
            let (i, _) = self.entries.iter().map(|x| x.1).enumerate().min().unwrap();
            self.remove(i);
        }
        let index = self.entries.insert((entry.clone(), 0));
        let ref mut map = self.map;
        for (ci, c) in entry.windows(4).enumerate() {
            let ic = slice_to_u32(c);
            map.entry(ic).or_insert(FnvHashSet::default()).insert(
                Match {
                    line: index,
                    offset: ci,
                },
            );
        }
    }
    fn remove(&mut self, entry_index: usize) -> Vec<u8> {
        let entry = self.entries.remove(entry_index);
        for c in entry.0.windows(4) {
            let ic = slice_to_u32(c);
            self.map.get_mut(&ic).map(
                |v| v.retain(|m| m.line != entry_index),
            );
        }
        entry.0
    }
}

impl Compressor for ChunkMap {
    fn encode(&mut self, needle: &[u8], buf: &mut Vec<u8>) {
        //let mut matches: Vec<Option<Match>> = Vec::new();
        let c_matches: Vec<FnvHashSet<Match>> = needle
            .chunks(4)
            .filter(|c| c.len() == 4)
            .map(|chunk| {
                self.map
                    .get(&slice_to_u32(chunk))
                    .map(|s| s.clone())
                    .unwrap_or(FnvHashSet::default())
            })
            .collect();

        //println!("Chunks {:?}", c_matches);

        let mut chains = Vec::new();
        let mut i = 0;
        while i < c_matches.len() {
            let mut c_chains = Vec::with_capacity(c_matches[i].len());
            for m in &c_matches[i] {
                let mut n = 1;
                while i + n < c_matches.len() && c_matches[i + n].contains(&m.next_nth_chunk(n)) {
                    n += 1;
                }
                c_chains.push(m.to_block(i * CHUNK_SIZE, n * CHUNK_SIZE));
            }
            if let Some(block) = c_chains.into_iter().max_by(|a, b| a.len.cmp(&b.len)) {
                i += block.len / 4;
                chains.push(block);
            } else {
                i += 1;
            }
        }
        //println!("Chains {:?}", chains);

        for bi in 0..chains.len() {
            let lb = if bi == 0 {
                0
            } else {
                let last = &chains[bi - 1];
                last.needle_off + last.len - 1
            };

            let rb = chains.get(bi + 1).map(|n| n.needle_off).unwrap_or(
                needle.len(),
            );
            let block = &mut chains[bi];
            if let BlockType::Delta { line, offset: _ } = block.block_type {
                block.fit(needle, &self.entries[line].0, lb, rb);
            }
        }

        //println!("Fitted {:?}", chains);

        let mut last_end = 0;
        let mut bi = 0;
        let old_buf_len = buf.len();
        while {
            let next_off = chains.get(bi).map(|b| b.needle_off).unwrap_or(needle.len());
            let b = Block {
                block_type: BlockType::Original,
                needle_off: last_end,
                len: next_off - last_end,
            };
            let block = if b.len != 0 {
                &b
            } else {
                let block = &chains[bi];
                if let BlockType::Delta { line, offset: _ } = block.block_type {
                    self.entries[line].1 += block.len;
                }
                bi += 1;
                block
            };

            //println!("{:?}", block);

            block.encode(needle, buf);
            last_end = block.needle_off + block.len;

            //println!("{:?}", last_end);

            bi < chains.len() || last_end < needle.len()
        }
        {}
        let clen = buf.len() - old_buf_len;
        let cr = clen as f32 / needle.len() as f32;
        if cr > self.insert_threshold {
            self.insert(needle.to_vec());
        }
    }
    fn decode(&mut self, mut in_buf: &[u8], out_buf: &mut Vec<u8>) {
        let old_buf_len = out_buf.len();
        let in_buf_len = in_buf.len();
        while in_buf.len() != 0 {
            let (data, size) = Block::decode(in_buf, self);
            out_buf.extend_from_slice(data);
            in_buf = &in_buf[size..];
        }
        let dlen = out_buf.len() - old_buf_len;
        let cr = in_buf_len as f32 / dlen as f32;
        if cr > self.insert_threshold {
            //eprintln!("Inserting {}", cr);
            self.insert(out_buf[old_buf_len..].to_vec());
        }
    }
}


#[test]
pub fn nchunk_test() {
    let mut map = ChunkMap::new();
    map.insert("Hello Denis Worlds".as_bytes().to_vec());
    map.insert("Test Worlds".as_bytes().to_vec());
    map.insert("Test Bananas".as_bytes().to_vec());

    let mut buf = Vec::new();
    map.encode("Hello Test Worlds".as_bytes(), &mut buf);

    println!("Finished {:?}", buf);
}
