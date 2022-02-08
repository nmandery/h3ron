use crate::{Error, FromH3Index, H3Direction};
use h3ron_h3_sys::H3Index;
use std::ffi::CString;

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
    /// TODO: move to trait, edge + cell
    fn resolution(&self) -> u8 {
        (unsafe { h3ron_h3_sys::getResolution(self.h3index()) }) as u8
    }

    /// Checks the validity of the index
    fn is_valid(&self) -> bool {
        self.validate().is_ok()
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

impl<IX> HasH3Resolution for IX
where
    IX: Index,
{
    fn h3_resolution(&self) -> u8 {
        self.resolution()
    }
}

/// parse an index from its string representation
pub(crate) fn index_from_str<IX: Index>(s: &str) -> Result<IX, Error> {
    let cs = CString::new(s).map_err(|_| Error::Failed)?;

    let mut h3index: H3Index = 0;
    Error::check_returncode(unsafe { h3ron_h3_sys::stringToH3(cs.as_ptr(), &mut h3index) })?;

    let index = IX::new(h3index);
    index.validate()?;
    Ok(index)
}
