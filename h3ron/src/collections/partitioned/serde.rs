use std::fmt;
use std::hash::{BuildHasher, Hash};
use std::iter::FromIterator;
use std::marker::PhantomData;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::collections::{RandomState, ThreadPartitionedMap};

impl<K, V, S> Serialize for ThreadPartitionedMap<K, V, S>
where
    K: Hash + Eq + Send + Sync + Serialize,
    V: Send + Sync + Serialize,
    S: BuildHasher + Default + Send + Clone,
{
    fn serialize<SER>(&self, serializer: SER) -> Result<SER::Ok, SER::Error>
    where
        SER: Serializer,
    {
        // serialize as a standard hashmap, so this can also be deserialized using `std::collections::HashMap`
        // and friends.
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self.iter() {
            map.serialize_entry(&k, v)?;
        }
        map.end()
    }
}

struct ThreadPartitionedMapVisitor<K, V> {
    #[allow(clippy::type_complexity)]
    marker: PhantomData<fn() -> ThreadPartitionedMap<K, V, RandomState>>,
}

impl<'de, K, V> Visitor<'de> for ThreadPartitionedMapVisitor<K, V>
where
    K: Hash + Eq + Send + Sync + Deserialize<'de>,
    V: Send + Sync + Deserialize<'de>,
{
    type Value = ThreadPartitionedMap<K, V, RandomState>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("ThreadPartitionedMap failed")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, <M as MapAccess<'de>>::Error>
    where
        M: MapAccess<'de>,
    {
        let mut entries = Vec::with_capacity(access.size_hint().unwrap_or(4096));
        while let Some((k, v)) = access.next_entry::<K, V>()? {
            entries.push((k, v));
        }
        Ok(Self::Value::from_iter(entries))
    }
}

impl<'de, K, V> Deserialize<'de> for ThreadPartitionedMap<K, V, RandomState>
where
    K: Hash + Eq + Send + Sync + Deserialize<'de>,
    V: Send + Sync + Deserialize<'de>,
{
    fn deserialize<DES>(deserializer: DES) -> Result<Self, DES::Error>
    where
        DES: Deserializer<'de>,
    {
        deserializer.deserialize_map(ThreadPartitionedMapVisitor {
            marker: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    use crate::collections::ThreadPartitionedMap;

    #[test]
    fn serde_roundtrip() {
        let in_vec: Vec<_> = (0_u64..1_000).map(|i| (i, i)).collect();
        let tpm = ThreadPartitionedMap::from_iter(in_vec.clone());

        let byte_data = bincode::serialize(&tpm).unwrap();

        let mut tpm_de =
            bincode::deserialize::<ThreadPartitionedMap<u64, u64>>(&byte_data).unwrap();

        assert_eq!(tpm_de.len(), tpm.len());
        let mut out_vec: Vec<_> = tpm_de.drain().collect();
        out_vec.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(in_vec, out_vec);
    }
}
