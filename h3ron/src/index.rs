use crate::collections::indexvec::IndexVec;
use crate::{Error, FromH3Index, H3Direction};
use h3ron_h3_sys::H3Index;
use std::os::raw::c_int;

/// Trait to handle types having a H3 Index like cells and edges
pub trait Index: Sized + PartialEq + FromH3Index {
    /// Get the u64 H3 Index address
    fn h3index(&self) -> H3Index;

    /// create an index from the given u64.
    ///
    /// No validation is performed.
    fn new(h3index: H3Index) -> Self;

    /// Checks the validity of the index
    fn validate(&self) -> Result<(), Error>;

    /// Gets the index resolution (0-15)
    fn resolution(&self) -> u8 {
        (unsafe { h3ron_h3_sys::h3GetResolution(self.h3index()) }) as u8
    }

    /// Checks the validity of the index
    fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Checks if `self` is a parent of `other`
    fn is_parent_of(&self, other: &Self) -> bool {
        *self == other.get_parent_unchecked(self.resolution())
    }

    /// Checks if `other` is a parent of `self`
    fn is_child_of(&self, other: &Self) -> bool {
        other.is_parent_of(self)
    }

    /// Checks if `self` is a parent of `other`
    fn contains(&self, other: &Self) -> bool {
        self.is_parent_of(other)
    }

    /// Retrieves the parent index at `parent_resolution`.
    ///
    /// # Returns
    ///
    /// This method may fail if the `parent_resolution` is higher than current `self` resolution.
    ///
    /// If you don't want it to fail use `get_parent_unchecked`
    fn get_parent(&self, parent_resolution: u8) -> Result<Self, Error> {
        let res = self.get_parent_unchecked(parent_resolution);
        res.validate()?;
        Ok(res)
    }

    /// Retrieves the parent index at `parent_resolution`.
    ///
    /// # Returns
    ///
    /// This method may return an invalid `Index` if the `parent_resolution`is higher than current
    /// `self` resolution.
    ///
    /// Use `get_parent` for validity check.
    fn get_parent_unchecked(&self, parent_resolution: u8) -> Self {
        Self::new(unsafe { h3ron_h3_sys::h3ToParent(self.h3index(), parent_resolution as c_int) })
    }

    /// Retrieves all children of `self` at resolution `child_resolution`
    fn get_children(&self, child_resolution: u8) -> IndexVec<Self> {
        let mut index_vec = IndexVec::with_length(unsafe {
            h3ron_h3_sys::maxH3ToChildrenSize(self.h3index(), child_resolution as c_int)
        } as usize);
        unsafe {
            h3ron_h3_sys::h3ToChildren(
                self.h3index(),
                child_resolution as c_int,
                index_vec.as_mut_ptr(),
            );
        }
        index_vec
    }

    /// Retrieves the direction of the current index
    fn direction(&self) -> H3Direction {
        H3Direction::direction(self)
    }

    /// Retrieves the direction of the current index relative to a parent at `target_resolution`
    fn direction_to_parent_resolution(&self, target_resolution: u8) -> Result<H3Direction, Error> {
        H3Direction::direction_to_parent_resolution(self, target_resolution)
    }
}

/// trait to be implemented by all structs being based
/// on H3 data with a given resolution
pub trait HasH3Resolution {
    /// Gets the index resolution (0-15)
    fn h3_resolution(&self) -> u8;
}
