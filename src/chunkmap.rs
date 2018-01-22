extern crate fnv;
extern crate slab;

use self::fnv::FnvHashMap;
use self::fnv::FnvHashSet;
use self::slab::Slab;

use std::collections::LinkedList;
use std::mem::transmute;
use std::iter;

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
            offset: self.offset + CHUNK_SIZE,
        }
    }
}

pub struct Chain {
    line: usize,
    start_chunk: usize,
    line_offset: usize,
    length: usize,
}

impl Chain {
    fn from_chunks(chunk: &Match, chunk_index: usize, c_length: usize) -> Self {
        Chain {
            line: chunk.line,
            start_chunk: chunk_index,
            line_offset: chunk.offset,
            length: c_length,
        }
    }
}

pub struct ChunkMap {
    map: FnvHashMap<u32, FnvHashSet<Match>>,
    entries: Slab<Vec<u8>>,
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
    pub fn new() -> Self {
        ChunkMap {
            map: FnvHashMap::default(),
            entries: Slab::with_capacity(CACHE_SIZE),
        }
    }
    pub fn insert(&mut self, entry: Vec<u8>) {
        let index = self.entries.insert(entry.clone());
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
    pub fn remove(&mut self, entry_index: usize) -> Vec<u8> {
        let entry = self.entries.remove(entry_index);
        for c in entry.windows(4) {
            let ic = slice_to_u32(c);
            self.map.get_mut(&ic).map(
                |v| v.retain(|m| m.line != entry_index),
            );
        }
        entry
    }

    pub fn lookup_index(&self, entry_index: usize) -> &[u8] {
        &self.entries[entry_index]
    }

    fn chunk_match(&self, needle: &[u8]) {
        //let mut matches: Vec<Option<Match>> = Vec::new();
        let mut c_matches: Vec<FnvHashSet<Match>> = needle
            .chunks(4)
            .map(|chunk| {
                self.map
                    .get(&slice_to_u32(chunk))
                    .map(|s| s.clone())
                    .unwrap_or(FnvHashSet::default())
            })
            .collect();
        let mut chains = Vec::new();
        for i in 0..c_matches.len() - 1 {
            for m in c_matches[i].clone() {
                let mut c_length = 1;
                while i + c_length < c_matches.len() &&
                    c_matches[i + c_length].contains(&m.next_nth_chunk(c_length))
                {
                    c_matches[i + c_length].remove(&m.next_nth_chunk(c_length));
                    c_length += 1;
                }
                if c_length > 1 {
                    chains.push(Chain::from_chunks(&m, i, c_length));
                }
            }
        }
        // pick which chains to use
        // fill remaining with matches
        // expand chains and matches
        // encode
    }
}
