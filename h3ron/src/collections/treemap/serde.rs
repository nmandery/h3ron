use std::marker::PhantomData;

use roaring::RoaringTreemap;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::collections::H3Treemap;

impl<T> Serialize for H3Treemap<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut buffer = Vec::with_capacity(self.treemap.serialized_size());
        self.treemap
            .serialize_into(&mut buffer)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_bytes(&buffer)
    }
}

struct H3TreemapVisitor<T> {
    phantom_data: PhantomData<T>,
}

impl<'de, T> Visitor<'de> for H3TreemapVisitor<T> {
    type Value = H3Treemap<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("serialized roaring treemap")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let treemap = RoaringTreemap::deserialize_from(v).map_err(E::custom)?;
        Ok(H3Treemap {
            treemap,
            phantom_data: PhantomData::<T>::default(),
        })
    }
}

// TODO: deserialization does not ensure the data contained in the treemap are valid indexes.
impl<'de, T> Deserialize<'de> for H3Treemap<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(H3TreemapVisitor {
            phantom_data: PhantomData::<T>::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use crate::collections::H3Treemap;
    use crate::H3Cell;

    #[test]
    fn serde_roundtrip() {
        let idx = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        let mut treemap = H3Treemap::default();
        treemap.insert(idx);

        let mut serialized_bytes = vec![];
        bincode::serialize_into(&mut serialized_bytes, &treemap).unwrap();
        // dbg!(serialized_bytes.len());

        let deserialized: H3Treemap<H3Cell> =
            bincode::deserialize_from(serialized_bytes.as_slice()).unwrap();
        assert_eq!(deserialized.len(), 1);
        assert!(deserialized.contains(&idx));
    }
}
