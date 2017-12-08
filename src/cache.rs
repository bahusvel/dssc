use std::cmp::Ordering;

const CACHE_SIZE: usize = 256;

#[derive(Eq)]
pub struct CacheEntry {
    pub hits: usize,
    pub data: Vec<u8>,
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

pub type VecCache = Vec<CacheEntry>;

pub trait DSSCache {
    fn cache_insert(&mut self, buf: &[u8]);
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
