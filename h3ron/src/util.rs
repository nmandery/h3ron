use h3ron_h3_sys::H3Index;

use crate::index::Index;

/// filter out 0 values ( = positions in the vec not used to store h3indexes)
macro_rules! remove_zero_indexes_from_vec {
    ($h3_indexes:expr) => {
        $h3_indexes.retain(|h3i: &H3Index| *h3i != 0);
    }
}

#[inline]
pub(crate) fn h3indexes_to_indexes(v: Vec<H3Index>) -> Vec<Index> {
    v.iter()
        .map(|h3index| Index::from(*h3index))
        .collect()
}

