use crate::collections::indexvec::IndexVec;
use crate::{H3Cell, H3Edge, Index};

/// Creates H3Edges from cells while only requiring a single memory allocation
/// when the struct is created.
pub struct H3EdgesBuilder {
    index_vec: IndexVec<H3Edge>,
}

impl Default for H3EdgesBuilder {
    fn default() -> Self {
        Self {
            index_vec: IndexVec::with_length(6),
        }
    }
}

impl H3EdgesBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Create an iterator for iterating over all [`H3Edge`] leading away from the given [`H3Cell`].
    pub fn from_origin_cell(
        &mut self,
        cell: &H3Cell,
    ) -> crate::collections::indexvec::UncheckedIter<'_, H3Edge> {
        unsafe {
            h3ron_h3_sys::getH3UnidirectionalEdgesFromHexagon(
                cell.h3index(),
                self.index_vec.as_mut_ptr(),
            )
        };
        self.index_vec.iter()
    }
}
