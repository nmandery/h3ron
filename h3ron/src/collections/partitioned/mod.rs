use std::borrow::BorrowMut;
use std::hash::{BuildHasher, Hash, Hasher};
use std::iter::FromIterator;

use rayon::prelude::*;

use crate::collections::ContainsIndex;
use crate::Index;

use super::RandomState;

#[cfg(feature = "use-serde")]
pub mod serde;

/// A Map consisting of multiple hashmaps (partitions) each handled by its own thread. The purpose is
/// acceleration of batch operations by splitting them across multiple threads. Like populating the
/// map faster from serialized data.
///
/// Inspired by the [vector hasher](https://github.com/pola-rs/polars/blob/0b145967533691249d094614c5315fa03a693fd9/polars/polars-core/src/vector_hasher.rs)
/// of the `polars` crate.
#[derive(Clone)]
pub struct ThreadPartitionedMap<K, V, S = RandomState> {
    build_hasher: S,
    partitions: Vec<hashbrown::HashMap<K, V, S>>,
}

impl<K, V, S> ThreadPartitionedMap<K, V, S>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
    S: BuildHasher + Default + Send + Clone,
{
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let num_partitions = num_cpus::get();
        let partition_capacity = if capacity > 0 {
            // expecting an equal distribution of keys over all partitions
            1 + capacity / num_partitions
        } else {
            0
        };
        let build_hasher = S::default();
        let partitions =
            create_partitions(num_partitions, partition_capacity, build_hasher.clone());
        Self {
            build_hasher,
            partitions,
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        let additional_avg = additional / self.num_partitions();
        for partition in self.partitions.iter_mut() {
            let partition_additional =
                additional_avg.saturating_sub(partition.capacity() - partition.len());
            if partition_additional > 0 {
                partition.reserve(partition_additional);
            }
        }
    }

    pub fn num_partitions(&self) -> usize {
        self.partitions.len()
    }

    pub fn partitions(&self) -> &[hashbrown::HashMap<K, V, S>] {
        &self.partitions
    }

    pub fn take_partitions(&mut self) -> Vec<hashbrown::HashMap<K, V, S>> {
        let new_partitions = create_partitions(self.partitions.len(), 0, self.build_hasher.clone());
        std::mem::replace(&mut self.partitions, new_partitions)
    }

    pub fn len(&self) -> usize {
        self.partitions.iter().map(|p| p.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.partitions.iter().all(|p| p.is_empty())
    }

    /// `modify_fn` takes two values and creates the value to be stored. The first value
    /// is the one which was previously in the map, the second one is the new one.
    pub fn insert_or_modify<F>(&mut self, key: K, value: V, modify_fn: F) -> Option<V>
    where
        F: Fn(&V, V) -> V,
    {
        let (h, partition) = hash_and_partition(
            &key,
            self.num_partitions(),
            self.build_hasher.build_hasher(),
        );
        let raw_entry = self.partitions[partition]
            .raw_entry_mut()
            .from_key_hashed_nocheck(h, &key);

        match raw_entry {
            hashbrown::hash_map::RawEntryMut::Occupied(mut entry) => {
                let (_occupied_key, occupied_value) = entry.get_key_value_mut();
                Some(std::mem::replace(
                    occupied_value,
                    modify_fn(occupied_value, value),
                ))
            }
            hashbrown::hash_map::RawEntryMut::Vacant(entry) => {
                entry.insert_hashed_nocheck(h, key, value);
                None
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert_or_modify(key, value, |_old, new| new)
    }

    pub fn insert_many<I>(&mut self, iter: I)
    where
        I: Iterator<Item = (K, V)>,
    {
        self.insert_or_modify_many(iter, |old, new| {
            *old = new;
        });
    }

    ///
    /// `modify_fn` takes two values and creates the value to be stored. The first value
    /// is the one which was previously in the map, the second one is the new one.
    pub fn insert_or_modify_many<I, F>(&mut self, iter: I, modify_fn: F)
    where
        I: Iterator<Item = (K, V)>,
        F: Fn(&mut V, V) + Send + Sync,
    {
        let mut hashed_kv = hash_vectorized(iter, &self.build_hasher, self.num_partitions());
        if self.partitions.len() != hashed_kv.len() {
            // should not be reachable
            panic!("differing length of partitions");
        }
        self.reserve(hashed_kv.len()); // not taking modifications into account
        self.partitions = std::mem::take(&mut self.partitions)
            .drain(..)
            .zip(hashed_kv.drain(..))
            .collect::<Vec<_>>()
            .par_drain(..)
            .map(|(mut partition, mut partition_hashed_kv)| {
                for (h, (k, v)) in partition_hashed_kv.drain(..) {
                    // raw_entry_mut in `std` requires nightly. in hashbrown it is already stable
                    // https://github.com/rust-lang/rust/issues/56167
                    let raw_entry = partition.raw_entry_mut().from_key_hashed_nocheck(h, &k);

                    match raw_entry {
                        hashbrown::hash_map::RawEntryMut::Occupied(mut entry) => {
                            let (_occupied_key, occupied_value) = entry.get_key_value_mut();
                            modify_fn(occupied_value, v);
                        }
                        hashbrown::hash_map::RawEntryMut::Vacant(entry) => {
                            entry.insert_hashed_nocheck(h, k, v);
                        }
                    }
                }
                partition
            })
            .collect();
    }

    pub fn get_key_value(&self, key: &K) -> Option<(&K, &V)> {
        let (h, partition) =
            hash_and_partition(key, self.num_partitions(), self.build_hasher.build_hasher());
        self.partitions[partition]
            .raw_entry()
            .from_key_hashed_nocheck(h, key)
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.get_key_value(key).map(|(_, v)| v)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.get_key_value(key).is_some()
    }

    pub fn keys(&self) -> TPMKeys<K, V, S> {
        TPMKeys {
            tpm: self,
            current_partition: 0,
            current_keys_iter: None,
        }
    }

    pub fn iter(&self) -> TPMIter<K, V, S> {
        TPMIter {
            tpm: self,
            current_partition: 0,
            current_iter: None,
        }
    }

    pub fn drain(&mut self) -> TPMDrain<'_, K, V> {
        let num_elements = self.len();
        let inner = self.partitions.iter_mut().map(|p| p.drain()).collect();
        TPMDrain {
            current: 0,
            inner,
            num_elements,
        }
    }
}

impl<K, V, S> Default for ThreadPartitionedMap<K, V, S>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
    S: BuildHasher + Default + Send + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> FromIterator<(K, V)> for ThreadPartitionedMap<K, V, RandomState>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let it = iter.into_iter();
        let mut tpm = Self::with_capacity(it.size_hint().0);
        tpm.insert_many(it);
        tpm
    }
}

fn create_partitions<K, V, S>(
    num_partitions: usize,
    partition_capacity: usize,
    build_hasher: S,
) -> Vec<hashbrown::HashMap<K, V, S>>
where
    K: Hash + Eq,
    S: BuildHasher + Clone,
{
    (0..num_partitions)
        .map(|_| {
            hashbrown::HashMap::with_capacity_and_hasher(
                partition_capacity,
                // all partitions must use the hasher with the same seed to generate the same
                // hashes
                build_hasher.clone(),
            )
        })
        .collect()
}

fn hash_vectorized<K, V, S, I>(
    iter: I,
    build_hasher: &S,
    num_partitions: usize,
) -> Vec<Vec<(u64, (K, V))>>
where
    K: Hash,
    S: BuildHasher,
    I: Iterator<Item = (K, V)>,
{
    let mut out_vecs: Vec<_> = (0..num_partitions)
        .map(|_| Vec::with_capacity(iter.size_hint().0 / num_partitions))
        .collect();

    iter.for_each(|(k, v)| {
        let (h, partition) = hash_and_partition(&k, num_partitions, build_hasher.build_hasher());
        out_vecs[partition].push((h, (k, v)));
    });
    out_vecs
}

#[inline]
fn hash_and_partition<K, H>(key: &K, num_partitions: usize, mut hasher: H) -> (u64, usize)
where
    H: Hasher,
    K: Hash,
{
    key.hash(&mut hasher);
    let h = hasher.finish();
    (h, h_partition(h, num_partitions as u64) as usize)
}

#[inline]
fn h_partition(h: u64, num_partitions: u64) -> u64 {
    // Based on https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
    // and used instead of modulo (`h % num_partitions`)
    ((h as u128 * num_partitions as u128) >> 64) as u64
}

pub struct TPMKeys<'a, K, V, S> {
    tpm: &'a ThreadPartitionedMap<K, V, S>,
    current_partition: usize,
    current_keys_iter: Option<hashbrown::hash_map::Keys<'a, K, V>>,
}

impl<'a, K, V, S> Iterator for TPMKeys<'a, K, V, S>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
    S: BuildHasher + Default + Send + Clone,
{
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(current_keys_iter) = self.current_keys_iter.borrow_mut() {
                if let Some(next_key) = current_keys_iter.next() {
                    return Some(next_key);
                } else {
                    self.current_partition += 1;
                }
            }
            if let Some(partition) = self.tpm.partitions.get(self.current_partition) {
                self.current_keys_iter = Some(partition.keys())
            } else {
                return None;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.tpm.len(), None)
    }
}

pub struct TPMIter<'a, K, V, S> {
    tpm: &'a ThreadPartitionedMap<K, V, S>,
    current_partition: usize,
    current_iter: Option<hashbrown::hash_map::Iter<'a, K, V>>,
}

impl<'a, K, V, S> Iterator for TPMIter<'a, K, V, S>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
    S: BuildHasher + Default + Send + Clone,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(current_iter) = self.current_iter.borrow_mut() {
                if let Some(next_kv) = current_iter.next() {
                    return Some(next_kv);
                } else {
                    self.current_partition += 1;
                }
            }
            if let Some(partition) = self.tpm.partitions.get(self.current_partition) {
                self.current_iter = Some(partition.iter())
            } else {
                return None;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.tpm.len(), None)
    }
}

pub struct TPMDrain<'a, K, V> {
    current: usize,
    inner: Vec<hashbrown::hash_map::Drain<'a, K, V>>,
    num_elements: usize,
}

impl<'a, K, V> Iterator for TPMDrain<'a, K, V>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(kv) = self.inner[self.current].next() {
                return Some(kv);
            }
            self.current += 1;
            if self.current >= self.inner.len() {
                return None;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.num_elements, None)
    }
}

impl<I: Index + Hash + Eq + Send + Sync, V: Send + Sync> ContainsIndex<I>
    for ThreadPartitionedMap<I, V>
{
    fn contains_index(&self, index: &I) -> bool {
        self.contains(index)
    }
}

#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    use crate::collections::ThreadPartitionedMap;
    use crate::H3Edge;
    use crate::Index;

    #[test]
    fn from_and_to_vec_h3edge() {
        let in_vec: Vec<_> = (0_u64..1_000_000).map(|i| (H3Edge::new(i), i)).collect();
        let mut tpm = ThreadPartitionedMap::from_iter(in_vec.clone());
        assert_eq!(tpm.len(), 1_000_000);
        assert_eq!(tpm.get(&H3Edge::new(613777)), Some(&613777));
        let mut out_vec: Vec<_> = tpm.drain().collect();
        out_vec.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(in_vec, out_vec);
    }

    #[test]
    fn insert_single() {
        let mut tpm: ThreadPartitionedMap<u64, u64> = Default::default();
        assert_eq!(tpm.insert(4, 1), None);
        assert_eq!(tpm.insert(4, 2), Some(1));
        assert_eq!(tpm.insert(5, 1), None);
        assert_eq!(tpm.len(), 2);
        assert_eq!(tpm.insert_or_modify(5, 2, |old, new| new + *old), Some(1));
        assert_eq!(tpm.get(&5), Some(&3));
        assert_eq!(tpm.len(), 2);
    }
}
