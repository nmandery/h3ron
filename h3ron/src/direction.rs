use crate::{Error, Index, H3_MAX_RESOLUTION};

const H3_PER_DIGIT_OFFSET: u8 = 3;
const H3_DIGIT_MASK: u8 = 7;

/// H3 digit representing ijk+ axes direction.
/// See [documentation](https://h3geo.org/docs/core-library/h3Indexing/#introduction)
///
/// ```text
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
            v => return Err(Error::DirectionInvalid(v)),
        })
    }
}

impl H3Direction {
    /// Retrieves the H3 Direction of the `index` relative to its direct parent
    ///
    /// # Errors
    ///
    /// May fail if the direction is invalid. This can be caused by trying to retrieve a direction for:
    /// - an index of 0 resolution
    /// - an invalid index
    pub fn direction_to_parent<I: Index>(index: &I) -> Result<Self, Error> {
        Self::direction_to_parent_resolution(index, index.resolution().saturating_sub(1))
    }

    /// Retrieves the H3 Direction of the `index`
    ///
    /// # Errors
    ///
    /// May fail if the direction is invalid. This can be caused by trying to retrieve a direction for:
    /// - an index of 0 resolution
    /// - an invalid index
    pub fn direction<I: Index>(index: &I) -> Result<Self, Error> {
        Self::direction_to_parent_resolution(index, index.resolution())
    }

    /// Retrieves the H3 Direction of the `index` relative to its parent at `target_resolution`.
    ///
    /// The function may fail if `target_resolution` is higher than `index` resolution
    pub fn direction_to_parent_resolution<I: Index>(
        index: &I,
        target_resolution: u8,
    ) -> Result<Self, Error> {
        if target_resolution > index.resolution() {
            return Err(Error::ResMismatch);
        }
        direction(index.h3index(), offset(target_resolution))
    }

    /// iterate over all directions leading to the given `index` starting from
    /// resolution 0 to the resolution of the `index`.
    pub fn iter_directions_over_resolutions<I: Index>(index: &I) -> ResolutionDirectionIter {
        ResolutionDirectionIter {
            h3index: index.h3index(),
            stop_offset: offset(index.resolution()),
            current_offset: offset(1),
        }
    }
}

#[inline]
fn offset(target_resolution: u8) -> u64 {
    u64::from(H3_MAX_RESOLUTION.saturating_sub(target_resolution) * H3_PER_DIGIT_OFFSET)
}

#[inline]
fn direction(h3index: u64, offset: u64) -> Result<H3Direction, Error> {
    let dir = (h3index >> offset) & u64::from(H3_DIGIT_MASK);
    H3Direction::try_from(dir as u8)
}

pub struct ResolutionDirectionIter {
    h3index: u64,
    stop_offset: u64,
    current_offset: u64,
}

impl Iterator for ResolutionDirectionIter {
    type Item = Result<H3Direction, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_offset >= self.stop_offset {
            let dir_result = direction(self.h3index, self.current_offset);
            self.current_offset -= u64::from(H3_PER_DIGIT_OFFSET);
            Some(dir_result)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{H3Cell, H3DirectedEdge};

    use super::*;

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
        let direction = H3Direction::direction_to_parent(&cell).unwrap();
        assert_eq!(direction, H3Direction::JkAxesDigit);
        let direction = H3Direction::direction(&cell).unwrap();
        assert_eq!(direction, H3Direction::IjAxesDigit);
    }

    #[test]
    fn can_be_created_from_index_to_low_res() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        assert_eq!(cell.resolution(), 5);
        let direction = H3Direction::direction_to_parent_resolution(&cell, 1).unwrap();
        assert_eq!(direction, H3Direction::KAxesDigit);
    }

    #[should_panic(expected = "ResMismatch")]
    #[test]
    fn can_fail_from_wrong_resolution() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        assert_eq!(cell.resolution(), 5);
        H3Direction::direction_to_parent_resolution(&cell, 6).unwrap();
    }
    #[test]
    fn can_fail_with_res_0() {
        let cell = H3Cell::try_from(0x801ffffffffffff).unwrap();
        let cell_2 = H3Cell::try_from(0x805ffffffffffff).unwrap();
        assert_eq!(cell.resolution(), 0);
        assert_eq!(cell_2.resolution(), 0);
        assert!(H3Direction::direction(&cell).is_err());
        assert!(H3Direction::direction(&cell_2).is_err());
    }

    #[test]
    fn children_directions() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        let children = cell.get_children(cell.resolution() + 1).unwrap();
        for (i, child) in children.iter().enumerate() {
            let direction = H3Direction::direction(&child).unwrap();
            assert_eq!(direction as usize, i);
        }
    }

    #[test]
    fn children_edge_directions() {
        let cell = H3Cell::try_from(0x8518607bfffffff).unwrap();
        let children = cell.get_children(cell.resolution() + 1).unwrap();
        let center_child = children.first().unwrap();
        for (i, child) in children.iter().enumerate() {
            if child == center_child {
                continue;
            }
            let edge = child.directed_edge_to(center_child).unwrap();
            let direction = H3Direction::direction(&edge).unwrap();
            assert_eq!(direction as usize, i);
        }
    }

    #[test]
    fn iter_directions_over_resolutions_cell() {
        let cell = H3Cell::new(0x861ea54f7ffffff);
        //let cell = H3Cell::new(0x8518607bfffffff);
        let directions = H3Direction::iter_directions_over_resolutions(&cell)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(directions.len(), cell.resolution() as usize);
        assert_eq!(
            directions,
            vec![
                H3Direction::JAxesDigit,
                H3Direction::IAxesDigit,
                H3Direction::IkAxesDigit,
                H3Direction::JAxesDigit,
                H3Direction::JkAxesDigit,
                H3Direction::IjAxesDigit
            ]
        );
    }

    #[test]
    fn iter_directions_over_resolutions_edge() {
        let edge = H3DirectedEdge::new(0x149283080ddbffff);
        let directions = H3Direction::iter_directions_over_resolutions(&edge)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(directions.len(), edge.resolution() as usize);
        assert_eq!(
            directions,
            vec![
                H3Direction::CenterDigit,
                H3Direction::IjAxesDigit,
                H3Direction::CenterDigit,
                H3Direction::IAxesDigit,
                H3Direction::CenterDigit,
                H3Direction::KAxesDigit,
                H3Direction::IkAxesDigit,
                H3Direction::IjAxesDigit,
                H3Direction::IjAxesDigit,
            ]
        );
    }
}
