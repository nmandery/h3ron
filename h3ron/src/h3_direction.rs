use crate::{Error, Index, H3_MAX_RESOLUTION};
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

impl H3Direction {
    /// Retrieves the H3 Direction of the `index` relative to its direct parent
    pub fn direction_to_parent<I: Index>(index: &I) -> Self {
        Self::direction_to_parent_resolution(index, index.resolution().saturating_sub(1)).unwrap()
    }

    /// Retrieves the H3 Direction of the `index`
    pub fn direction<I: Index>(index: &I) -> Self {
        Self::direction_to_parent_resolution(index, index.resolution()).unwrap()
    }

    /// Retrieves the H3 Direction of the `index` relative to its parent at `target_resolution`.
    ///
    /// The function may fail if `target_resolution` is higher than `index` resolution
    pub fn direction_to_parent_resolution<I: Index>(
        index: &I,
        target_resolution: u8,
    ) -> Result<Self, Error> {
        if target_resolution > index.resolution() {
            return Err(Error::MixedResolutions(
                index.resolution(),
                target_resolution,
            ));
        }
        let ptr = index.h3index() as *const u64;
        let ptr = ptr as u64;
        let offset =
            (H3_MAX_RESOLUTION.saturating_sub(target_resolution) * H3_PER_DIGIT_OFFSET) as u64;
        let mask = H3_DIGIT_MASK as u64;
        let dir = (ptr >> offset) & mask;
        Self::try_from(dir as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::H3Cell;
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
        let direction = H3Direction::direction_to_parent_resolution(&cell, 4).unwrap();
        assert_eq!(direction, H3Direction::JkAxesDigit);
        let direction = H3Direction::direction_to_parent(&cell);
        assert_eq!(direction, H3Direction::JkAxesDigit);
        let direction = H3Direction::direction(&cell);
        assert_eq!(direction, H3Direction::IjAxesDigit);
    }

    #[test]
    fn can_be_created_from_index_to_low_res() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        assert_eq!(cell.resolution(), 5);
        let direction = H3Direction::direction_to_parent_resolution(&cell, 1).unwrap();
        assert_eq!(direction, H3Direction::KAxesDigit);
    }

    #[should_panic(expected = "MixedResolutions")]
    #[test]
    fn can_fail_from_wrong_resolution() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        assert_eq!(cell.resolution(), 5);
        H3Direction::direction_to_parent_resolution(&cell, 6).unwrap();
    }

    #[test]
    fn works_with_res_0() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        let cell = cell.get_parent(0).unwrap();
        assert_eq!(cell.resolution(), 0);
        let direction = H3Direction::direction_to_parent(&cell);
        assert_eq!(direction, H3Direction::IAxesDigit);
    }

    #[test]
    fn children_directions() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        let children = cell.get_children(cell.resolution() + 1);
        for (i, child) in children.iter().enumerate() {
            let direction = H3Direction::direction(&child);
            assert_eq!(direction as usize, i);
        }
    }

    #[test]
    fn children_edge_directions() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        let children = cell.get_children(cell.resolution() + 1);
        let center_child = children.first().unwrap();
        for (i, child) in children.iter().enumerate() {
            if child == center_child {
                continue;
            }
            let edge = child.unidirectional_edge_to(&center_child).unwrap();
            let direction = H3Direction::direction(&edge);
            assert_eq!(direction as usize, i);
        }
    }
}
