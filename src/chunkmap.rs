extern crate fnv;

use self::fnv::FnvHashMap;

use std::collections::LinkedList;
use std::mem::transmute;

const EDEN_SIZE = 10;
const CACHE_SIZE = 255 - EDEN_SIZE;
const CHUNK_SIZE = 4;

struct Match {
    line: usize,
    offset: usize,
}

pub struct ChunkMap {
    map: FnvHashMap<u32, LinkedList<Match>>,
    entries: Slab<Vec<u8>>
}

pub fn chunk_to_u32(chunk: [u8; 4]) -> u32 {
    unsafe {transmute::<[u8; 4], u32>(chunk)}
}

pub slice_to_u32(s: &[u8]) -> u32 {
    assert!(slice.len() == 4);
    let chunk = [s[0], s[1], s[2], s[3]];
    chunk_to_u32(chunk)
}

impl ChunkMap {
    pub fn new() {
        ChunkMap {
            map: FnvHashMap::default(),
            entries: Slab::with_capacity(CACHE_SIZE),
        }
    }
    pub fn insert(&mut self, entry: Vec<u8>) {
        let index = self.entries.insert(entry.clone());
        let ref mut map = self.map;
        entry.into_iter().windows(4).enumerate().for_each(move |(c, ci)| {
            let ic = slice_to_u32(c);
            if let Some(ms) = map.get_mut(c) {
                ms.push(Match{line: index, offset: ci});
            } else {
                let list = LinkedList::new();
                list.push(Match{line: index, offset: ci});
                map.insert(list);
            }
        });
    }
    pub fn remove(entry_index: usize) -> Vec<u8> {}
    pub fn lookup_chunk(chunk: [u8]) -> Iterator<Match> {
    }
    pub fn lookup_index(entry_index: usize) -> &[u8] ()
}
