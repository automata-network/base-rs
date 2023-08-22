use std::prelude::v1::*;

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

#[derive(Debug, Clone)]
pub struct LruMap<K, V> {
    seq: usize,
    order: BTreeMap<usize, K>,
    value: BTreeMap<K, (V, usize)>,
    limit: usize,
}

impl<K, V> LruMap<K, V>
where
    K: Ord + Clone + std::fmt::Debug,
    V: Clone,
{
    pub fn new(limit: usize) -> Self {
        Self {
            seq: 0,
            order: BTreeMap::new(),
            value: BTreeMap::new(),
            limit,
        }
    }

    pub fn clear(&mut self) {
        self.order.clear();
        self.value.clear();
    }

    pub fn contains_key(&self, k: &K) -> bool {
        self.value.contains_key(k)        
    }


    pub fn peek(&self, k: &K) -> Option<&V> {
        self.value.get(k).map(|item| &item.0)
    }

    pub fn modify(&mut self, k: &K) -> Option<&mut V> {
        match self.value.get_mut(k) {
            Some((v, _)) => Some(v),
            None => None
        }
    }

    pub fn get(&mut self, k: &K) -> Option<&V> {
        match self.value.get_mut(k) {
            Some((n, seq)) => {
                self.seq += 1;
                match self.order.remove(seq) {
                    Some(k) => {
                        *seq = self.seq;
                        self.order.insert(self.seq, k);
                    }
                    None => {
                        *seq = self.seq;
                        self.order.insert(self.seq, k.clone());
                    }
                }
                Some(n)
            }
            None => None,
        }
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.seq += 1;
        match self.value.entry(k.clone()) {
            Entry::Occupied(mut entry) => {
                let old_seq = entry.get().1;
                entry.get_mut().1 = self.seq;
                self.order.remove(&old_seq);
                self.order.insert(self.seq, k);
                return None;
            }
            Entry::Vacant(entry) => {
                entry.insert((v, self.seq));
                self.order.insert(self.seq, k);
            }
        };
        if self.order.len() % 10000 == 0 {
            glog::info!("lru tree len: {}", self.order.len());
        }
        let mut out_val = None;
        while self.order.len() > self.limit {
            let n = self.order.iter().next().unwrap();
            // glog::info!(
            //     "evitent key: {} -> {}, self.value.len()={} ==> {:?}",
            //     self.order.len(),
            //     self.limit,
            //     self.value.len(),
            //     n,
            // );
            let seq = *n.0;
            let key = n.1.clone();
            self.order.remove(&seq);
            out_val = self.value.remove(&key).map(|item| item.0);
        }
        out_val
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        match self.value.remove(k) {
            Some((n, seq)) => {
                self.order.remove(&seq);
                return Some(n);
            }
            None => None,
        }
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }

    pub fn append(&mut self, other: &mut BTreeMap<K, V>) -> Vec<V> {
        let mut outs = Vec::new();
        let mut new = BTreeMap::new();
        std::mem::swap(other, &mut new);
        for (k, v) in new {
            if let Some(val) = self.insert(k, v) {
                outs.push(val);
            }
        }
        outs
    }

    pub fn stat(&self) {
        glog::info!("order:{}, value:{}", self.order.len(), self.value.len());
    }
}