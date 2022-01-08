use std::borrow::Borrow;
use std::iter::FromIterator;
use std::marker::PhantomData;
use std::mem::size_of;

use lz4_flex::{compress, decompress_into};
#[cfg(feature = "use-serde")]
use serde::{Deserialize, Serialize};

use crate::{Error, Index, IndexVec};

/// `IndexVec` allows to store h3index in compressed form.
///
/// The main purpose of this is to allow having seldom used data in memory without
/// it occupying too much space. This sacrifices a bit of speed when accessing the
/// data.
///
/// The order if the h3indexes in the block is not changed, so - for example - continuous paths of
/// h3 edges can be stored without them becoming shuffled.
///
/// The compression is done using the LZ4 algorithm. To improve the compression ratio
/// the bytes of all contained h3indexes are grouped by their position in the `u64` of the
/// h3index. For spatially close h3index this results in a quite good compression ratio as many
/// bytes are common over many h3indexes. As an example: a k-ring with `k=50` and 7651 cells
/// compresses from 61kb to around 7.6kb.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "use-serde",
    serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>",))
)]
pub struct IndexBlock<T> {
    phantom_data: PhantomData<T>,
    num_indexes: usize,
    compressed: bool,
    block_data: Vec<u8>,
}

impl<T> IndexBlock<T> {
    /// approximate size of the data in the block.
    ///
    /// Does not include any overheads from the inner `Vec`.
    pub fn size_of(&self) -> usize {
        size_of::<Self>() + (size_of::<u8>() * self.block_data.len())
    }

    pub const fn len(&self) -> usize {
        self.num_indexes
    }

    pub const fn is_empty(&self) -> bool {
        self.num_indexes == 0
    }

    pub const fn is_compressed(&self) -> bool {
        self.compressed
    }

    /// The size of the inner data when it would be stored in a simple `Vec`
    #[allow(dead_code)]
    pub const fn size_of_uncompressed(&self) -> usize {
        size_of::<Vec<T>>() + size_of::<T>() * self.len()
    }

    #[allow(dead_code)]
    pub const fn size_of_compressed(&self) -> usize {
        size_of::<Self>() + size_of::<u8>() * self.len()
    }
}

impl<'a, T> From<&[T]> for IndexBlock<T>
where
    T: Index,
{
    fn from(index_slice: &[T]) -> Self {
        let byte_offset = index_slice.len();
        let mut buf = vec![255u8; index_slice.len() * (size_of::<u64>() / size_of::<u8>())];

        for (pos, index) in index_slice.iter().enumerate() {
            let h3index = index.h3index() as u64;

            // keep the same bits of the h3indexes together to improve compression
            // when the h3indexes are closely together.
            buf[pos] = (h3index >> (7 * 8) & 255) as u8;
            buf[pos + (byte_offset)] = (h3index >> (6 * 8) & 255) as u8;
            buf[pos + (2 * byte_offset)] = (h3index >> (5 * 8) & 255) as u8;
            buf[pos + (3 * byte_offset)] = (h3index >> (4 * 8) & 255) as u8;
            buf[pos + (4 * byte_offset)] = (h3index >> (3 * 8) & 255) as u8;
            buf[pos + (5 * byte_offset)] = (h3index >> (2 * 8) & 255) as u8;
            buf[pos + (6 * byte_offset)] = (h3index >> 8 & 255) as u8;
            buf[pos + (7 * byte_offset)] = (h3index & 255) as u8;
        }

        let compressed = index_slice.len() >= 4;
        let block_data = if compressed { compress(&buf) } else { buf };

        Self {
            phantom_data: PhantomData::default(),
            num_indexes: index_slice.len(),
            compressed,
            block_data,
        }
    }
}

impl<T> From<Vec<T>> for IndexBlock<T>
where
    T: Index,
{
    fn from(vc: Vec<T>) -> Self {
        vc.as_slice().into()
    }
}

impl<T> From<IndexVec<T>> for IndexBlock<T>
where
    T: Index + Copy,
{
    fn from(ivc: IndexVec<T>) -> Self {
        ivc.iter().collect()
    }
}

impl<T, B> FromIterator<B> for IndexBlock<T>
where
    B: Borrow<T>,
    T: Index + Copy,
{
    fn from_iter<I: IntoIterator<Item = B>>(iter: I) -> Self {
        let indexes: Vec<T> = iter
            .into_iter()
            .map(|i| {
                let index: T = *i.borrow();
                index
            })
            .collect();
        indexes.as_slice().into()
    }
}

pub struct Decompressor {
    buf: Vec<u8>,
}

impl Decompressor {
    pub const fn new() -> Self {
        Self { buf: vec![] }
    }

    pub fn decompress_block<'a, 'b, T>(
        &'a mut self,
        block: &'b IndexBlock<T>,
    ) -> Result<DecompressedIter<'a, 'b, T>, Error>
    where
        T: Index,
    {
        let buf = if block.is_compressed() {
            let uncompressed_size = block.num_indexes * size_of::<u64>();
            self.buf.resize(uncompressed_size, 0xff);
            let bytes_uncompressed = decompress_into(block.block_data.as_slice(), &mut self.buf)
                .map_err(|e| Error::DecompressionError(e.to_string()))?;
            if bytes_uncompressed != uncompressed_size {
                return Err(Error::DecompressionError(format!(
                    "size missmatch. expected {} bytes, uncompressed to {} bytes",
                    uncompressed_size, bytes_uncompressed
                )));
            }
            Some(self.buf.as_slice())
        } else {
            None
        };
        Ok(DecompressedIter { buf, block, pos: 0 })
    }
}

impl Default for Decompressor {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DecompressedIter<'a, 'b, T> {
    buf: Option<&'a [u8]>,
    block: &'b IndexBlock<T>,
    pos: usize,
}

impl<'a, 'b, T> Iterator for DecompressedIter<'a, 'b, T>
where
    T: Index,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.block.num_indexes {
            return None;
        }
        let buf = match self.buf {
            Some(b) => b,
            None => self.block.block_data.as_slice(),
        };
        let h3index: u64 = (u64::from(buf[self.pos]) << (7 * 8))
            + (u64::from(buf[self.pos + self.block.num_indexes]) << (6 * 8))
            + (u64::from(buf[self.pos + (2 * self.block.num_indexes)]) << (5 * 8))
            + (u64::from(buf[self.pos + (3 * self.block.num_indexes)]) << (4 * 8))
            + (u64::from(buf[self.pos + (4 * self.block.num_indexes)]) << (3 * 8))
            + (u64::from(buf[self.pos + (5 * self.block.num_indexes)]) << (2 * 8))
            + (u64::from(buf[self.pos + (6 * self.block.num_indexes)]) << 8)
            + u64::from(buf[self.pos + (7 * self.block.num_indexes)]);
        self.pos += 1;
        Some(T::from_h3index(h3index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.block.num_indexes, None)
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;
    use std::iter::FromIterator;

    use crate::collections::compressed::Decompressor;
    use crate::H3Cell;

    use super::IndexBlock;

    fn make_kring(k: u32) -> Vec<H3Cell> {
        let idx = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        idx.k_ring(k).into()
    }

    fn kring_indexblock_roundtrip(cells: Vec<H3Cell>) -> IndexBlock<H3Cell> {
        let compressed_cells = IndexBlock::from(cells.as_slice());

        println!(
            "n:{}, size compressed: {:?}, size uncompressed: {}",
            cells.len(),
            compressed_cells.size_of_compressed(),
            compressed_cells.size_of_uncompressed()
        );
        /*
        assert!(
            compressed_ring.size_of_data_compressed()
                <= compressed_ring.size_of_data_uncompressed()
        );

         */
        assert_eq!(cells.len(), compressed_cells.len());

        let mut decompressor = Decompressor::default();
        let ring2: Vec<_> = decompressor
            .decompress_block(&compressed_cells)
            .unwrap()
            .collect();
        assert_eq!(cells, ring2);

        compressed_cells
    }

    #[test]
    fn test_indexblock_roundtrip_kring1() {
        let _civ = kring_indexblock_roundtrip(make_kring(1));
    }

    #[test]
    fn test_indexblock_roundtrip_kring8() {
        let _civ = kring_indexblock_roundtrip(make_kring(8));
    }

    #[test]
    fn test_indexblock_roundtrip_kring50() {
        let _civ = kring_indexblock_roundtrip(make_kring(50));
    }

    #[test]
    fn test_indexblock_roundtrip_2_cells() {
        let cells = make_kring(1).iter().take(2).copied().collect();
        let _civ = kring_indexblock_roundtrip(cells);
    }

    #[test]
    fn test_indexblock_from_iter() {
        let ib: IndexBlock<H3Cell> = IndexBlock::from_iter(make_kring(3).iter());
        assert!(!ib.is_empty());
        assert!(ib.is_compressed());
    }

    #[cfg(feature = "use-serde")]
    #[test]
    fn serde_roundtrip() {
        let ib = IndexBlock::from(make_kring(3).as_slice());
        let byte_data = bincode::serialize(&ib).unwrap();
        let ib_de = bincode::deserialize::<IndexBlock<H3Cell>>(&byte_data).unwrap();

        assert_eq!(ib_de.len(), ib.len());
        assert_eq!(ib, ib_de);
    }
}
