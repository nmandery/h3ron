use crate::index::Index;
use crate::{Error, FromH3Index};
use h3ron_h3_sys::H3Index;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::ffi::CString;
use std::str::FromStr;

/// a single H3 index
#[derive(PartialOrd, PartialEq, Clone, Debug, Serialize, Deserialize, Hash, Eq, Ord, Copy)]
pub struct EdgeIndex(H3Index);

/// convert to index including validation
impl TryFrom<u64> for EdgeIndex {
    type Error = Error;

    fn try_from(h3index: H3Index) -> Result<Self, Self::Error> {
        let index = Self::new(h3index);
        index.validate()?;
        Ok(index)
    }
}

impl EdgeIndex {}

impl FromH3Index for EdgeIndex {
    fn from_h3index(h3index: H3Index) -> Self {
        EdgeIndex::new(h3index)
    }
}

impl Index for EdgeIndex {
    fn h3index(&self) -> H3Index {
        self.0
    }

    fn new(h3index: H3Index) -> Self {
        Self(h3index)
    }
}

impl ToString for EdgeIndex {
    fn to_string(&self) -> String {
        format!("{:x}", self.0)
    }
}

impl FromStr for EdgeIndex {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let h3index: H3Index = CString::new(s)
            .map(|cs| unsafe { h3ron_h3_sys::stringToH3(cs.as_ptr()) })
            .map_err(|_| Error::InvalidInput)?;
        Self::try_from(h3index)
    }
}
