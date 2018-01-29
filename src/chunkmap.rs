extern crate fnv;
extern crate slab;

use self::fnv::FnvHashMap;
use self::fnv::FnvHashSet;
use self::slab::Slab;

use super::varint::{put_uvarint, uvarint};
use super::Compressor;

use std::fmt;
use std::hash::{Hash, Hasher};

const EDEN_SIZE: usize = 10;
const CACHE_SIZE: usize = 255 - EDEN_SIZE;
const CHUNK_SIZE: usize = 4;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Match {
    line: u32,
    offset: u32,
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
    map: FnvHashMap<u32, Vec<Match>>,
    entries: Slab<(Vec<u8>, usize)>,
    insert_threshold: f32,
}

pub fn slice_to_u32(s: &[u8]) -> u32 {
    assert!(s.len() == 4);
    unsafe { *(s.as_ptr() as *const u32) }
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
            map.entry(ic).or_insert(Vec::new()).push(Match {
                line: index as u32,
                offset: ci as u32,
            });
        }
    }
    fn remove(&mut self, entry_index: usize) -> Vec<u8> {
        let entry = self.entries.remove(entry_index);
        for c in entry.0.windows(4) {
            let ic = slice_to_u32(c);
            self.map
                .get_mut(&ic)
                .map(|v| v.retain(|m| m.line != entry_index as u32));
        }
        entry.0
    }
}

fn differs_at(a: &[u8], b: &[u8]) -> usize {
    let max = a.len().min(b.len());
    let ap = a.as_ptr() as *const usize;
    let bp = b.as_ptr() as *const usize;
    let mut in8 = 0;
    while in8 < max / 8 && unsafe { *ap.offset(in8 as isize) == *bp.offset(in8 as isize) } {
        in8 += 1;
    }
    let mut i = in8 * 8;
    while i < max && a[i] == b[i] {
        i += 1;
    }
    i
}

fn differs_back(a: &[u8], b: &[u8]) -> usize {
    let al = a.len();
    let bl = b.len();
    let max = al.min(bl);
    let mut i = 1;
    while i < max && a[al - i] == b[bl - i] {
        i += 1;
    }
    i - 1
}

impl Compressor for ChunkMap {
    fn encode(&mut self, needle: &[u8], buf: &mut Vec<u8>) {
        let chunks: Vec<u32> = needle
            .chunks(4)
            .filter(|c| c.len() == 4)
            .map(|c| slice_to_u32(c))
            .collect();

        let mut chains: Vec<Block> = Vec::new();
        let mut ci = 0;
        let mut last_end = 0;
        while ci < chunks.len() {
            let m = self.map.get(&chunks[ci]);
            if m.is_none() || m.unwrap().len() == 0 {
                ci += 1;
                continue;
            }
            let matches = m.unwrap();
            let block = matches
                .iter()
                .map(|m| {
                    let line = &self.entries[m.line as usize].0;
                    let diff_back =
                        differs_back(&needle[last_end..ci * 4], &line[..m.offset as usize]);
                    let diff_forward =
                        differs_at(&needle[ci * 4 + 4..], &line[4 + m.offset as usize..]);
                    Block {
                        block_type: BlockType::Delta {
                            line: m.line as usize,
                            offset: m.offset as usize - diff_back,
                        },
                        needle_off: ci * 4 - diff_back,
                        len: diff_forward + 4 + diff_back,
                    }
                })
                .max_by(|a, b| a.len.cmp(&b.len))
                .unwrap();

            ci += ((block.len + 3) & !0x03) / 4;
            // it was last.needle_off + last.len -1, but still works, dunno why.
            last_end = block.needle_off + block.len;
            chains.push(block);
        }

        //println!("Chains {:?}", chains);

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
        } {}
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
/*
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
*/

#[test]
pub fn diff_test() {
    let a = b"helloworls";
    let b = b"helloworld";
    println!("{:?}", differs_at(a, b));
}
