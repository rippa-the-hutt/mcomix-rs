//! A tiny LRU cache with a byte budget, used to keep decoded page textures
//! around so that revisiting nearby pages is instant (mirrors MComix's
//! `max pages to cache` behaviour).

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

pub struct LruCache<K: Eq + Hash + Clone, V> {
    map: HashMap<K, (V, usize)>,
    order: VecDeque<K>,
    capacity: usize,
    max_bytes: usize,
    bytes: usize,
}

impl<K: Eq + Hash + Clone, V> LruCache<K, V> {
    pub fn new(capacity: usize, max_bytes: usize) -> Self {
        LruCache {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity: capacity.max(1),
            max_bytes,
            bytes: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Returns a reference to the value, promoting the entry to
    /// most-recently-used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if self.map.contains_key(key) {
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                let k = self.order.remove(pos).unwrap();
                self.order.push_back(k);
            }
            self.map.get(key).map(|(v, _)| v)
        } else {
            None
        }
    }

    /// Insert or update an entry (value + its approximate size in bytes).
    pub fn put(&mut self, key: K, value: V, bytes: usize) {
        if let Some(entry) = self.map.get_mut(&key) {
            self.bytes = self.bytes.saturating_sub(entry.1);
            entry.0 = value;
            entry.1 = bytes;
            self.bytes = self.bytes.saturating_add(bytes);
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                let k = self.order.remove(pos).unwrap();
                self.order.push_back(k);
            }
        } else {
            self.map.insert(key.clone(), (value, bytes));
            self.order.push_back(key);
            self.bytes = self.bytes.saturating_add(bytes);
        }
        self.evict();
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some((value, bytes)) = self.map.remove(key) {
            self.bytes = self.bytes.saturating_sub(bytes);
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                self.order.remove(pos);
            }
            Some(value)
        } else {
            None
        }
    }

    /// Drop entries whose key does not satisfy `f` (used to trim pages that
    /// fell out of the wanted window).
    pub fn retain(&mut self, mut f: impl FnMut(&K) -> bool) {
        let drop_keys: Vec<K> = self.order.iter().filter(|k| !f(k)).cloned().collect();
        for k in drop_keys {
            self.remove(&k);
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
        self.bytes = 0;
    }

    fn evict(&mut self) {
        while self.map.len() > self.capacity || (self.bytes > self.max_bytes && self.map.len() > 1) {
            let Some(key) = self.order.pop_front() else {
                break;
            };
            if let Some((_, bytes)) = self.map.remove(&key) {
                self.bytes = self.bytes.saturating_sub(bytes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used() {
        let mut c = LruCache::new(3, 1000);
        c.put(1, "a", 1);
        c.put(2, "b", 1);
        c.put(3, "c", 1);
        let _ = c.get(&1); // promote 1
        c.put(4, "d", 1); // evicts 2 (LRU)
        assert!(!c.contains(&2));
        assert!(c.contains(&1));
        assert!(c.contains(&3));
        assert!(c.contains(&4));
    }

    #[test]
    fn respects_byte_budget() {
        let mut c = LruCache::new(100, 10);
        c.put(1, "x", 6);
        c.put(2, "y", 6);
        // total 12 > 10 -> evicts one
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn retain_drops_only_unwanted() {
        let mut c = LruCache::new(10, 1000);
        c.put(1, "a", 1);
        c.put(2, "b", 1);
        c.put(3, "c", 1);
        c.retain(|k| *k != 2);
        assert!(!c.contains(&2));
        assert!(c.contains(&1) && c.contains(&3));
    }
}
