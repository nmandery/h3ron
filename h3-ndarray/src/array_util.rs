use ndarray::{ArrayView2, Axis};
use geo_types::{Rect, Coordinate};

fn find_continuous_chunks_along_axis<T>(a: &ArrayView2<T>, axis: usize, nodata_value: &T) -> Vec<(usize, usize)> where T: Sized + PartialEq {
    let mut chunks = Vec::new();
    let mut current_chunk_start: Option<usize> = None;

    for (r0pos, r0) in a.axis_iter(Axis(axis)).enumerate() {
        if r0.iter().any(|v| v != nodata_value) {
            if current_chunk_start.is_none() {
                current_chunk_start = Some(r0pos);
            }
        } else if let Some(begin) = current_chunk_start {
            chunks.push((begin, r0pos - 1));
            current_chunk_start = None;
        }
    }
    if let Some(begin) = current_chunk_start {
        chunks.push((begin, a.shape()[axis] - 1));
    }
    chunks
}

/// find all boxes in the array where there are any values except the nodata_value
///
/// this implementation is far from perfect and often recognizes multiple smaller
/// clusters as one as its based on completely empty columns and rows, but it is probably
/// sufficient for the purpose to reduce the number of hexagons
/// to be generated when dealing with fragmented/sparse datasets.
pub fn find_boxes_containing_data<T>(a: &ArrayView2<T>, nodata_value: &T) -> Vec<Rect<usize>> where T: Sized + PartialEq {
    let mut boxes = Vec::new();

    for chunk_0raw_indexes in find_continuous_chunks_along_axis(a, 0, nodata_value) {
        let sv = a.slice(s![chunk_0raw_indexes.0..=chunk_0raw_indexes.1, ..]);
        for chunks_1raw_indexes in find_continuous_chunks_along_axis(&sv, 1, nodata_value) {
            let sv2 = sv.slice(s![0..=(chunk_0raw_indexes.1-chunk_0raw_indexes.0), chunks_1raw_indexes.0..=chunks_1raw_indexes.1]);

            // one more iteration along axis 0 to get the specific range for that axis 1 range
            for chunks_0_indexes in find_continuous_chunks_along_axis(&sv2, 0, nodata_value) {
                boxes.push(Rect::new(
                    Coordinate {
                        x: chunks_0_indexes.0 + chunk_0raw_indexes.0,
                        y: chunks_1raw_indexes.0,
                    },
                    Coordinate {
                        x: chunks_0_indexes.1 + chunk_0raw_indexes.0,
                        y: chunks_1raw_indexes.1,
                    },
                ))
            }
        }
    }
    boxes
}

#[cfg(test)]
mod tests {
    use crate::array_util::find_boxes_containing_data;

    #[test]
    fn test_find_boxes_containig_data() {
        let arr = array![
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
            [0, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            [0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1, 1],
        ];
        let mut arr_copy = arr.clone();

        let n_elements = arr_copy.shape()[0] * arr_copy.shape()[1];
        let mut n_elements_in_boxes = 0;

        for rect in find_boxes_containing_data(&arr.view(), &0) {
            n_elements_in_boxes += (rect.max().x - rect.min().x + 1) * (rect.max().y - rect.min().y + 1);

            //dbg!(rect);
            for x in rect.min().x..=rect.max().x {
                for y in rect.min().y..=rect.max().y {
                    arr_copy[(x, y)] = 0;
                }
            }
        }
        //dbg!(n_elements);
        //dbg!(n_elements_in_boxes);

        // there should be far less indexes to visit now
        assert!(n_elements_in_boxes < (n_elements / 2));

        // all elements should have been removed
        assert_eq!(arr_copy.sum(), 0);
  }
}
