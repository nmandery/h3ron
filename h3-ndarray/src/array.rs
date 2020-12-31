use std::collections::HashMap;
use std::hash::Hash;

use geo_types::{Coordinate, Rect};
use ndarray::{
    ArrayView2,
    Axis,
    parallel::prelude::*,
};

use h3::index::Index;
use h3::polyfill;
use h3::stack::H3IndexStack;
use log::debug;

use crate::error::Error;
use crate::transform::Transform;

//use rayon::prelude::*;

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
fn find_boxes_containing_data<T>(a: &ArrayView2<T>, nodata_value: &T) -> Vec<Rect<usize>> where T: Sized + PartialEq {
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

pub struct H3Converter<'a, T> where T: Sized + PartialEq + Sync + Eq + Hash {
    arr: &'a ArrayView2<'a, T>,
    nodata_value: &'a T,
    transform: &'a Transform,
}

impl<'a, T> H3Converter<'a, T> where T: Sized + PartialEq + Sync + Eq + Hash {
    pub fn new(arr: &'a ArrayView2<'a, T>, nodata_value: &'a T, transform: &'a Transform) -> Self {
        Self {
            arr,
            nodata_value,
            transform,
        }
    }

    fn rects_with_data(&self, rect_size: usize) -> Vec<Rect<f64>> {
        self.arr.axis_chunks_iter(Axis(0), rect_size)
            .into_par_iter() // requires T to be Sync
            .enumerate()
            .map(|(axis0_chunk_i, axis0_chunk)| {
                let mut rects = Vec::new();
                for chunk0_rect in find_boxes_containing_data(&axis0_chunk, self.nodata_value) {
                    let offset_0 = (axis0_chunk_i * rect_size) + chunk0_rect.min().x;
                    let chunk_rect_view = axis0_chunk.slice(s![chunk0_rect.min().x..chunk0_rect.max().x, chunk0_rect.min().y..chunk0_rect.max().y ]);
                    chunk_rect_view.axis_chunks_iter(Axis(1), rect_size)
                        .enumerate()
                        .for_each(|(c_i, c)| {
                            let offset_1 = (c_i * rect_size) + chunk0_rect.min().y;

                            // the window in array coordinates
                            let window = Rect::new(
                                Coordinate {
                                    x: offset_0 as f64,
                                    y: offset_1 as f64,
                                },
                                // add one to the max coordinate to include the whole last pixel
                                Coordinate {
                                    x: (offset_0 + c.shape()[0] + 1) as f64,
                                    y: (offset_1 + c.shape()[1] + 1) as f64,
                                },
                            );
                            rects.push(window)
                        })
                }
                rects
            }).flatten().collect()
    }

    pub fn to_h3(&self, h3_resolution: u8, compact: bool) -> Result<HashMap<&'a T, H3IndexStack>, Error> {
        let inverse_transform = self.transform.invert()?;

        let rects = self.rects_with_data(250);
        let n_rects = rects.len();
        debug!("to_h3: found {} rects containing non-nodata values", n_rects);

        let mut chunk_h3_maps = rects
            .into_par_iter()
            .enumerate()
            .map(|(array_window_i, array_window)| {
                debug!("to_h3: rect {}/{} with size {} x {}", array_window_i, n_rects, array_window.width(), array_window.height());

                // the window in geographical coordinates
                let window_box = self.transform * &array_window;
                //dbg!(window_box);

                let mut chunk_h3_map = HashMap::<&T, H3IndexStack>::new();
                let h3indexes = polyfill(&window_box.to_polygon(), h3_resolution);
                //println!("num h3indexes: {}", h3indexes.len());
                for h3index in h3indexes {
                    // find the array element for the coordinate of the h3 index
                    let arr_coord = {
                        let transformed = &inverse_transform * &Index::from(h3index).coordinate();
                        /*
                        Coordinate {
                            x: transformed.x.floor() as usize,
                            y: transformed.y.floor() as usize,
                        }

                         */
                        [transformed.x.floor() as usize, transformed.y.floor() as usize]
                    };
                    if let Some(value) = self.arr.get(arr_coord) {
                        if value != self.nodata_value {
                            chunk_h3_map.entry(value)
                                .or_insert_with(|| H3IndexStack::new())
                                .add_indexes(&[h3index], false);
                        }
                    }
                }

                // do an early compacting to free a bit of memory
                if compact {
                    chunk_h3_map.iter_mut()
                        .for_each(|(_value, index_stack)| {
                            index_stack.compact();
                        });
                }

                chunk_h3_map
            })
            .collect::<Vec<_>>();

        // combine the results from all chunks
        let mut h3_map = HashMap::new();
        for mut chunk_h3_map in chunk_h3_maps.drain(..) {
            for (value, mut index_stack) in chunk_h3_map.drain() {
                h3_map.entry(value)
                    .or_insert_with(|| H3IndexStack::new())
                    .append(&mut index_stack, false);
            }
        }

        h3_map.iter_mut()
            .for_each(|(_, index_stack)| {
                if compact {
                    index_stack.compact()
                } else {
                    index_stack.dedup()
                };
            });

        Ok(h3_map)
    }
}

#[cfg(test)]
mod tests {
    use crate::array::find_boxes_containing_data;

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
