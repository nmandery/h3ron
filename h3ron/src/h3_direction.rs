use crate::{Error, H3Cell, Index, H3_MAX_RESOLUTION};
use std::convert::TryFrom;

const H3_PER_DIGIT_OFFSET: u8 = 3;
const H3_DIGIT_MASK: u8 = 7;

#[svgbobdoc::transform]
/// H3 digit representing ijk+ axes direction.
/// See [documentation](https://h3geo.org/docs/core-library/h3Indexing/#introduction)
///
/// ```svgbob
///            j Axis
///            ___
///           /   \
///       +--+  2  +--+
///      / 3  \___/  6 \
///      \    /   \    /
///       +--+  0  +--+
///      /    \___/    \
///      \ 1  /   \  4 /
///       +--+  5  +--+   i Axis
/// k Axis    \___/
/// ```
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub enum H3Direction {
    /// H3 digit in center
    CenterDigit = 0,

    /// H3 digit in k-axes direction
    KAxesDigit = 1,

    /// H3 digit in j-axes direction
    JAxesDigit = 2,

    /// H3 digit in j==k direction
    JkAxesDigit = 3, // Self::JAxesDigit as isize | Self::KAxesDigit as isize,

    /// H3 digit in i-axes direction
    IAxesDigit = 4,

    /// H3 digit in i==k direction
    IkAxesDigit = 5, //Self::IAxesDigit as isize | Self::KAxesDigit as isize,

    /// H3 digit in i==j direction
    IjAxesDigit = 6, // Self::IAxesDigit as isize | Self::JAxesDigit as isize,
}

impl TryFrom<u8> for H3Direction {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::CenterDigit,
            1 => Self::KAxesDigit,
            2 => Self::JAxesDigit,
            3 => Self::JkAxesDigit,
            4 => Self::IAxesDigit,
            5 => Self::IkAxesDigit,
            6 => Self::IjAxesDigit,
            v => return Err(Error::InvalidH3Direction(v)),
        })
    }
}

impl H3Cell {
    /// Retrieves the H3 Direction of the `self` cell relative to its direct parent
    pub fn direction_to_parent(&self) -> H3Direction {
        self.direction_to_parent_resolution(self.resolution().saturating_sub(1))
            .unwrap()
    }

    /// Retrieves the H3 Direction of the `self`cell relative to its parent at `resolution`.
    ///
    /// The function may fail if `resolution` is higher than `self` resolution
    pub fn direction_to_parent_resolution(&self, resolution: u8) -> Result<H3Direction, Error> {
        if resolution > self.resolution() {
            return Err(Error::MixedResolutions(self.resolution(), resolution));
        }
        let ptr = self.h3index() as *const u64;
        let ptr = ptr as u64;
        let offset = ((H3_MAX_RESOLUTION - resolution) * H3_PER_DIGIT_OFFSET) as u64;
        let mask = H3_DIGIT_MASK as u64;
        let dir = (ptr >> offset) & mask;
        H3Direction::try_from(dir as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn can_be_created() {
        assert_eq!(H3Direction::try_from(0).unwrap(), H3Direction::CenterDigit);
        assert_eq!(H3Direction::try_from(1).unwrap(), H3Direction::KAxesDigit);
        assert_eq!(H3Direction::try_from(2).unwrap(), H3Direction::JAxesDigit);
        assert_eq!(H3Direction::try_from(3).unwrap(), H3Direction::JkAxesDigit);
        assert_eq!(H3Direction::try_from(4).unwrap(), H3Direction::IAxesDigit);
        assert_eq!(H3Direction::try_from(5).unwrap(), H3Direction::IkAxesDigit);
        assert_eq!(H3Direction::try_from(6).unwrap(), H3Direction::IjAxesDigit);
    }

    #[test]
    fn can_be_compared() {
        assert!(H3Direction::CenterDigit < H3Direction::KAxesDigit);
        assert!(H3Direction::IjAxesDigit > H3Direction::KAxesDigit);
    }

    #[test]
    fn can_be_created_from_index() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        assert_eq!(cell.resolution(), 5);
        let direction = cell.direction_to_parent_resolution(4).unwrap();
        assert_eq!(direction, H3Direction::JkAxesDigit);
        let direction = cell.direction_to_parent();
        assert_eq!(direction, H3Direction::JkAxesDigit);
    }

    #[test]
    fn can_be_created_from_index_to_low_res() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        assert_eq!(cell.resolution(), 5);
        let direction = cell.direction_to_parent_resolution(1).unwrap();
        assert_eq!(direction, H3Direction::KAxesDigit);
    }

    #[should_panic(expected = "MixedResolutions")]
    #[test]
    fn can_fail_from_wrong_resolution() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        assert_eq!(cell.resolution(), 5);
        cell.direction_to_parent_resolution(6).unwrap();
    }

    #[test]
    fn works_with_res_0() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        let cell = cell.get_parent(0).unwrap();
        assert_eq!(cell.resolution(), 0);
        let direction = cell.direction_to_parent();
        assert_eq!(direction, H3Direction::IAxesDigit);
    }
}
