use std::borrow::Borrow;
use std::marker::PhantomData;
use std::mem::size_of;

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
/// The compression is done using run-length-encoding (RLE). To improve the compression ratio
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
    num_indexes: usize,
    block_data: Vec<u8>,
    phantom_data: PhantomData<T>,
}

impl<T> IndexBlock<T>
where
    T: Index,
{
    pub const fn len(&self) -> usize {
        self.num_indexes
    }

    pub const fn is_empty(&self) -> bool {
        self.num_indexes == 0
    }

    /// check if `index` is contained in this `IndexBlock`.
    ///
    /// This method avoids decompressing the whole block at once and instead
    /// only decompresses single bytes while traversing the contained indexes.
    pub fn contains(&self, index: &T) -> Result<bool, Error> {
        if self.num_indexes == 0 {
            return Ok(false);
        }
        let h3index_bytes = (index.h3index() as u64).to_le_bytes();
        let mut matching = vec![true; self.num_indexes];
        let mut byte_pos = 0_usize;
        let mut found = true;
        rle_decode_step_bytes(self.block_data.as_slice(), |byte, repetitions| {
            for _ in 0..repetitions {
                let h3index_byte_i = byte_pos / self.num_indexes;
                let h3index_i = byte_pos % self.num_indexes;

                matching[h3index_i] =
                    matching[h3index_i] && (byte == h3index_bytes[h3index_byte_i]);
                byte_pos += 1;

                // exit or early-exit in case no chance for a match is left
                if h3index_i == (self.num_indexes - 1) && !matching.iter().any(|v| *v) {
                    found = false;
                    break;
                }
            }
            found
        })?;
        Ok(found)
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

    /// returns an iterator over the decompressed decompressed contents of the `IndexBlock`.
    ///
    /// Useful for situations where only one or few decompressions are done. When
    /// many blocks are decompressed using the `Decompressor` is more efficient as the
    /// decompression buffer needs to be allocated only once.
    pub fn iter_uncompressed(&self) -> Result<OwningDecompressedIter<T>, Error> {
        let decompressor = Decompressor::default();
        decompressor.decompress_block_owning(self)
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
            let h3index_bytes = h3index.to_le_bytes();
            buf[pos] = h3index_bytes[0];
            buf[pos + byte_offset] = h3index_bytes[1];
            buf[pos + (2 * byte_offset)] = h3index_bytes[2];
            buf[pos + (3 * byte_offset)] = h3index_bytes[3];
            buf[pos + (4 * byte_offset)] = h3index_bytes[4];
            buf[pos + (5 * byte_offset)] = h3index_bytes[5];
            buf[pos + (6 * byte_offset)] = h3index_bytes[6];
            buf[pos + (7 * byte_offset)] = h3index_bytes[7];
        }

        let mut block_data = vec![];

        rle_encode(&buf, &mut block_data);
        block_data.shrink_to_fit();

        Self {
            num_indexes: index_slice.len(),
            block_data,
            phantom_data: PhantomData::default(),
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

    fn decompress_block_into_inner_buf<T>(&mut self, block: &IndexBlock<T>) -> Result<(), Error>
    where
        T: Index,
    {
        let uncompressed_size = block.num_indexes * size_of::<u64>();
        if self.buf.capacity() < uncompressed_size {
            self.buf
                .reserve(uncompressed_size.saturating_sub(self.buf.capacity()));
        }
        self.buf.clear();
        rle_decode(block.block_data.as_slice(), &mut self.buf)?;

        if self.buf.len() != uncompressed_size {
            Err(Error::DecompressionError(format!(
                "Expected to decompress to {} bytes, but got {} bytes",
                uncompressed_size,
                self.buf.len()
            )))
        } else {
            Ok(())
        }
    }
    pub fn decompress_block<'a, 'b, T>(
        &'a mut self,
        block: &'b IndexBlock<T>,
    ) -> Result<DecompressedIter<'a, 'b, T>, Error>
    where
        T: Index,
    {
        self.decompress_block_into_inner_buf(block)?;
        Ok(DecompressedIter {
            buf: self.buf.as_slice(),
            block,
            pos: 0,
        })
    }

    pub fn decompress_block_owning<T>(
        mut self,
        block: &IndexBlock<T>,
    ) -> Result<OwningDecompressedIter<T>, Error>
    where
        T: Index,
    {
        self.decompress_block_into_inner_buf(block)?;
        Ok(OwningDecompressedIter {
            buf: self.buf,
            block,
            pos: 0,
        })
    }
}

impl Default for Decompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn h3index_from_block_buf(buf: &[u8], pos: usize, num_indexes: usize) -> u64 {
    assert!(pos < num_indexes);
    assert!(buf.len() >= (num_indexes * size_of::<u64>() / size_of::<u8>()));
    u64::from_le_bytes([
        buf[pos],
        buf[pos + num_indexes],
        buf[pos + (2 * num_indexes)],
        buf[pos + (3 * num_indexes)],
        buf[pos + (4 * num_indexes)],
        buf[pos + (5 * num_indexes)],
        buf[pos + (6 * num_indexes)],
        buf[pos + (7 * num_indexes)],
    ])
}

pub struct DecompressedIter<'a, 'b, T> {
    buf: &'a [u8],
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
        let h3index: u64 = h3index_from_block_buf(self.buf, self.pos, self.block.num_indexes);
        self.pos += 1;
        Some(T::from_h3index(h3index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.block.num_indexes.saturating_sub(self.pos), None)
    }
}

/// a iterator owning the decompressed buffer.
///
/// Useful for situations where only one or few decompressions are done. When
/// many blocks are decompressed using the `Decompressor` is more efficient as the
/// decompression buffer needs to be allocated only once.
pub struct OwningDecompressedIter<'a, T> {
    buf: Vec<u8>,
    block: &'a IndexBlock<T>,
    pos: usize,
}

impl<'a, T> Iterator for OwningDecompressedIter<'a, T>
where
    T: Index,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.block.num_indexes {
            return None;
        }
        let h3index: u64 =
            h3index_from_block_buf(self.buf.as_slice(), self.pos, self.block.num_indexes);
        self.pos += 1;
        Some(T::from_h3index(h3index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.block.num_indexes.saturating_sub(self.pos), None)
    }
}

/// traverse through run-length-encoded bytes and pass each found byte to `step_fn`.
///
/// `step_fn` takes two arguments: the byte and the number of repetitions.
/// This function continues to step through the given bytes as long as `step_fn` returns true
/// or the end of the given byte slice has been reached.
fn rle_decode_step_bytes<SF>(bytes: &[u8], mut step_fn: SF) -> Result<(), Error>
where
    SF: FnMut(u8, u8) -> bool,
{
    if bytes.len() % 2 != 0 {
        return Err(Error::DecompressionError(
            "invalid (odd) input length".to_string(),
        ));
    }
    for (i, byte) in bytes.iter().enumerate() {
        if i % 2 != 0 {
            continue;
        }
        let repetitions = bytes[i + 1];
        if !step_fn(*byte, repetitions) {
            break;
        }
    }
    Ok(())
}

/// decode run-length-encoded bytes
fn rle_decode(bytes: &[u8], out: &mut Vec<u8>) -> Result<(), Error> {
    rle_decode_step_bytes(bytes, |byte, repetitions| {
        for _ in 0..repetitions {
            out.push(byte)
        }
        true
    })
}

/// run-length-encode bytes
fn rle_encode(bytes: &[u8], out: &mut Vec<u8>) {
    if let Some(byte) = bytes.first() {
        out.push(*byte);
    } else {
        return;
    }

    let mut occurrences = 1;
    for byte in bytes.iter().skip(1) {
        if byte == out.last().unwrap() && occurrences < 255 {
            occurrences += 1;
        } else {
            out.extend(&[occurrences, *byte]);
            occurrences = 1;
        }
    }
    out.push(occurrences);
}

#[cfg(test)]
mod tests {
    use crate::collections::compressed::Decompressor;
    use crate::H3Cell;

    use super::IndexBlock;

    fn make_grid_disk(k: u32) -> Vec<H3Cell> {
        let idx = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        idx.grid_disk(k).unwrap().into()
    }

    fn grid_disk_indexblock_roundtrip(cells: Vec<H3Cell>) -> IndexBlock<H3Cell> {
        let compressed_cells = IndexBlock::from(cells.as_slice());

        println!(
            "n:{}, size compressed: {:?}, size uncompressed: {}",
            cells.len(),
            compressed_cells.size_of_compressed(),
            compressed_cells.size_of_uncompressed()
        );
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
    fn test_indexblock_roundtrip_grid_disk1() {
        let civ = grid_disk_indexblock_roundtrip(make_grid_disk(1));
        assert!(civ.size_of_compressed() < civ.size_of_uncompressed());
    }

    #[test]
    fn test_indexblock_roundtrip_grid_disk8() {
        let civ = grid_disk_indexblock_roundtrip(make_grid_disk(8));
        assert!(civ.size_of_compressed() < civ.size_of_uncompressed());
        // dbg!((civ.size_of_compressed() , civ.size_of_uncompressed()));
    }

    #[test]
    fn test_indexblock_roundtrip_grid_disk50() {
        let civ = grid_disk_indexblock_roundtrip(make_grid_disk(50));
        assert!(civ.size_of_compressed() < civ.size_of_uncompressed());
        // dbg!((civ.size_of_compressed() , civ.size_of_uncompressed()));
    }

    #[test]
    fn test_indexblock_roundtrip_2_cells() {
        let cells = make_grid_disk(1).iter().take(2).copied().collect();
        let _civ = grid_disk_indexblock_roundtrip(cells);
    }

    #[test]
    fn test_indexblock_from_iter() {
        let ib: IndexBlock<H3Cell> = IndexBlock::from_iter(make_grid_disk(3).iter());
        assert!(!ib.is_empty());
    }

    #[test]
    fn test_indexblock_iter() {
        let ring = make_grid_disk(5);
        assert!(ring.len() > 10);
        let ib: IndexBlock<H3Cell> = IndexBlock::from(ring.as_slice());
        assert_eq!(ring.len(), ib.iter_uncompressed().unwrap().count());
    }

    #[cfg(feature = "use-serde")]
    #[test]
    fn serde_roundtrip() {
        let ib = IndexBlock::from(make_grid_disk(3).as_slice());
        let byte_data = bincode::serialize(&ib).unwrap();
        let ib_de = bincode::deserialize::<IndexBlock<H3Cell>>(&byte_data).unwrap();

        assert_eq!(ib_de.len(), ib.len());
        assert_eq!(ib, ib_de);
    }

    #[test]
    fn test_indexblock_contains() {
        let cell = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        let disk: Vec<_> = cell.grid_disk(2).unwrap().into();
        let ring: Vec<_> = cell.grid_ring_unsafe(3).unwrap().into();

        let ib = IndexBlock::from(disk.as_slice());
        assert_eq!(ib.len(), disk.len());

        for disk_cell in disk.iter() {
            assert!(ib.contains(disk_cell).unwrap());
        }
        for ring_cell in ring.iter() {
            assert!(!disk.iter().any(|dcell| dcell == ring_cell));
            assert!(!ib.contains(ring_cell).unwrap());
        }
    }
}
