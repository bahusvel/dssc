extern crate fnv;
extern crate slab;
extern crate bio;

use self::fnv::FnvHashMap;
use self::fnv::FnvHashSet;
use self::slab::Slab;
use self::bio::data_structures::interval_tree::IntervalTree;

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

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Chain {
    line: usize,
    needle_offset: usize,
    line_offset: usize,
    length: usize,
}

impl Chain {
    fn from_chunks(chunk: &Match, needle_offset: usize, c_length: usize) -> Self {
        Chain {
            line: chunk.line,
            needle_offset: needle_offset,
            line_offset: chunk.offset,
            length: c_length,
        }
    }

    fn find_interesctions<'a>(
        &self,
        itree: &'a IntervalTree<usize, Chain>,
    ) -> impl Iterator<Item = &'a Chain> {
        let self_clone = *self;
        itree
            .find(self.needle_offset..self.needle_offset + self.length)
            .map(|c| c.data())
            .filter(move |c| **c != self_clone)
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
            .filter(|c| c.len() == 4)
            .map(|chunk| {
                self.map
                    .get(&slice_to_u32(chunk))
                    .map(|s| s.clone())
                    .unwrap_or(FnvHashSet::default())
            })
            .collect();

        println!("Chunks {:?}", c_matches);

        let mut chains = IntervalTree::new();

        // I should construct the chains differently, for each chunk, do chain search, find all chains. And pick the biggest chain. Then skip ahead that many chunks. And then continue.

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
                    chains.insert(
                        (i * CHUNK_SIZE)..(i * CHUNK_SIZE + c_length * CHUNK_SIZE),
                        Chain::from_chunks(&m, i * CHUNK_SIZE, c_length * CHUNK_SIZE),
                    );
                }
            }
        }

        println!("Chains {:?}", chains);

        let mut use_chains = IntervalTree::new();

        for c in chains.find(0..needle.len()) {
            //let cc = c.clone();
            let cc = c.data();
            if cc.find_interesctions(&use_chains).count() != 0 {
                continue;
            }
            let largest = cc.find_interesctions(&chains).max_by(
                |a, b| a.length.cmp(&b.length),
            );
            if largest.is_none() || cc.length >= largest.unwrap().length {
                use_chains.insert(c.interval().clone(), *cc)
            }
        }

        println!("Using {:?}", use_chains);

        // fill remaining with matches
        // expand chains and matches
        // encode
    }
}


#[test]
pub fn nchunk_test() {
    let mut map = ChunkMap::new();
    map.insert("Hello Denis Worlds".as_bytes().to_vec());
    map.insert("Test Worlds".as_bytes().to_vec());
    map.insert("Test Bananas".as_bytes().to_vec());
    map.chunk_match("Hello Test Worlds".as_bytes());
}
