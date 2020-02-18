

/// filter out 0 values ( = positions in the vec not used to store h3indexes)
macro_rules! remove_zero_indexes_from_vec {
    ($h3_indexes:expr) => {
        $h3_indexes.retain(|h3i: &H3Index| *h3i != 0);
    }
}