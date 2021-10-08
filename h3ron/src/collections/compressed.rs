use std::marker::PhantomData;
use std::mem::size_of;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::collections::indexvec::IndexVec;
use crate::io::{deserialize_from_byte_slice, serialize_into};
use crate::Index;

#[derive(Serialize, Deserialize, Clone)]
pub struct CompressedStorage<T> {
    phantom_data: PhantomData<T>,
    compressed_bytes: Vec<u8>,
    num_elements: usize,
}

/// `Vec` to store `Index`-implementing types compressed.
///
/// Intended to keep seldom used data in memory while requiring a reduced amount of memory. Essentially
/// like linux `zram` for selected datasets.
///
/// The order of the elements in the vec is preserved
#[derive(Serialize, Deserialize, Clone)]
pub enum CompressedIndexVec<T> {
    Uncompressed(Vec<T>),
    Compressed(CompressedStorage<T>),
}

impl<'a, T> CompressedIndexVec<T>
where
    T: Index + DeserializeOwned + Serialize + Clone,
{
    #[allow(dead_code)]
    #[inline]
    pub fn len(&self) -> usize {
        match &self {
            Self::Uncompressed(inner_vec) => inner_vec.len(),
            Self::Compressed(compressed_storage) => compressed_storage.num_elements,
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[allow(dead_code)]
    pub fn to_vec(&'a self) -> Vec<T> {
        match self {
            Self::Uncompressed(inner_vec) => inner_vec.clone(),
            Self::Compressed(compressed_storage) => {
                let slice = compressed_storage.compressed_bytes.as_slice();
                deserialize_from_byte_slice(slice)
                    .expect("deserializing compressed index vec failed")
            }
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub fn is_compressed(&self) -> bool {
        match self {
            CompressedIndexVec::Uncompressed(_) => false,
            CompressedIndexVec::Compressed(_) => true,
        }
    }

    #[allow(dead_code)]
    pub fn size_of_data_uncompressed(&self) -> usize {
        sizeof_uncompressed::<T>(self.len())
    }

    #[allow(dead_code)]
    pub fn size_of_data_compressed(&self) -> Option<usize> {
        match self {
            Self::Uncompressed(_) => None,
            Self::Compressed(compressed_storage) => {
                Some(sizeof_compressed(compressed_storage.compressed_bytes.len()))
            }
        }
    }
}

#[inline(always)]
fn sizeof_uncompressed<T>(num_elements: usize) -> usize {
    size_of::<T>() * num_elements
}

#[inline(always)]
fn sizeof_compressed(num_elements: usize) -> usize {
    size_of::<u8>() * num_elements
}

/// create from a `Vec`
///
/// The construction is somewhat expensive as it includes the compression to determinate
/// the most space-efficient way to store the data.
impl<'a, T> From<Vec<T>> for CompressedIndexVec<T>
where
    T: Index + Serialize + Deserialize<'a> + Clone,
{
    fn from(mut input_vec: Vec<T>) -> Self {
        let mut compressed_bytes: Vec<u8> = vec![];
        serialize_into(&mut compressed_bytes, &input_vec, true)
            .expect("serializing into compressed index vec failed");
        if sizeof_uncompressed::<T>(input_vec.len()) > sizeof_compressed(compressed_bytes.len()) {
            compressed_bytes.shrink_to_fit();
            Self::Compressed(CompressedStorage {
                phantom_data: PhantomData::default(),
                compressed_bytes,
                num_elements: input_vec.len(),
            })
        } else {
            input_vec.shrink_to_fit();
            Self::Uncompressed(input_vec)
        }
    }
}

impl<'a, T> From<IndexVec<T>> for CompressedIndexVec<T>
where
    T: Index + Serialize + Deserialize<'a> + Clone,
{
    fn from(input_index_vec: IndexVec<T>) -> Self {
        let input_vec: Vec<_> = input_index_vec.into();
        input_vec.into()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use crate::H3Cell;

    use super::CompressedIndexVec;

    fn kring_compressedindexvec_roundtrip(k: u32) -> CompressedIndexVec<H3Cell> {
        let idx = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        let ring: Vec<_> = idx.k_ring(k).into();
        let compressed_ring: CompressedIndexVec<_> = ring.clone().into();

        println!(
            "k:{}, n:{}, size compressed: {:?}, size uncompressed: {}",
            k,
            ring.len(),
            compressed_ring.size_of_data_compressed(),
            compressed_ring.size_of_data_uncompressed()
        );
        if let Some(size_compressed) = compressed_ring.size_of_data_compressed() {
            assert!(size_compressed <= compressed_ring.size_of_data_uncompressed());
        }
        assert_eq!(ring.len(), compressed_ring.len());

        let ring2 = compressed_ring.to_vec();
        assert_eq!(ring, ring2);
        compressed_ring
    }

    #[test]
    fn test_compressedindexvec_roundtrip_k1() {
        let civ = kring_compressedindexvec_roundtrip(1);
        assert!(!civ.is_compressed());
    }

    #[test]
    fn test_compressedindexvec_roundtrip_k8() {
        let civ = kring_compressedindexvec_roundtrip(8);
        assert!(civ.is_compressed());
    }
}
