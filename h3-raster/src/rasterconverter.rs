use std::borrow::Borrow;
use std::cell::RefCell;
use std::cmp::max;
use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::TryFrom;
use std::thread;

use crossbeam_channel::{bounded, Receiver, Sender};
use gdal::raster::Dataset;
use geo_types::Rect;

use h3::{AreaUnits, k_ring};
use h3::compact::CompactedIndexStack;
use h3_sys::H3Index;

use crate::geo::{area_rect, rect_contains, rect_from_coordinates};
use crate::input::{ClassifiedBand, ToValue, Value};
use crate::iter::ZipMultiIter;
use crate::tile::{generate_tiles, Tile};
use crate::convertedraster::{ConvertedRaster, Attributes, GroupedH3Indexes};
use gdal_geotransform::GeoTransformer;
use gdal::spatial_ref::SpatialRef;

pub struct ConversionProgress {
    pub tiles_total: usize,
    pub tiles_done: usize,
}

pub struct RasterConverter {
    dataset: Dataset,
    inputs: Vec<ClassifiedBand>,
    geotransformer: GeoTransformer,
    h3_resolution: u8,
}

struct ConversionSubset {
    pub tile: Tile,
    pub geotransformer: GeoTransformer,
    banddata: Vec<Vec<Option<Value>>>,
    h3_resolution: u8,
}


impl RasterConverter {
    pub fn new(dataset: Dataset, inputs: Vec<ClassifiedBand>, h3_resolution: u8) -> Result<Self, &'static str> {
        let required_max_band = inputs
            .iter()
            .map(|k| k.source_band)
            .fold(0, max);

        if required_max_band > dataset.count() as u8 {
            return Err("Dataset has not enough bands for input specification");
        }

        match SpatialRef::from_definition(&dataset.projection()) {
            Ok(srs) =>  if srs.to_wkt().unwrap().is_empty() {
                } else if srs.ne(SpatialRef::from_epsg(4326).unwrap().borrow()) {
                return Err("Dataset has to be EPSG:4326") },
            Err(_) => return Err("could not get Spatial Ref from dataset")
        };

        if h3_resolution > 15 {
            return Err("given h3_resolution exceeds the defined range");
        }

        let geotransform = match dataset.geo_transform() {
            Ok(gt) => gt,
            Err(_) => return Err("can not obtain geotransform from dataset")
        };

        Ok(RasterConverter {
            dataset,
            inputs,
            geotransformer: GeoTransformer::try_from(geotransform)?,
            h3_resolution,
        })
    }

    fn extract_input_bands(&self, tile: &Tile) -> Result<Vec<Vec<Option<Value>>>, &'static str> {
        let mut input_data = vec![];
        let window = (tile.offset_origin.0 as isize, tile.offset_origin.1 as isize);

        for band_input in self.inputs.iter() {
            let band = self.dataset.rasterband(band_input.source_band as isize)
                .map_err(|e| {
                    eprintln!("Could not fetch band {}: {:?}", band_input.source_band, e);
                    "could not fetch band"
                })?;

            // block_size: https://gis.stackexchange.com/questions/292754/efficiently-read-large-tif-raster-to-a-numpy-array-with-gdal
            macro_rules! extract_band {
                ($datatype:path) => {{
                    // when the band type does not match $datatype, gdal will cast the values
                    let band_data_raw = band.read_as::<$datatype>(window, tile.size, tile.size);
                    match band_data_raw {
                        Err(e) => {
                                eprintln!("could not read from band as {}: {:?}", stringify!($datatype), e);
                                Err("Could not read band")
                        },
                        Ok(mut bd) => {
                            let result: Vec<_> = bd.data.drain(..)
                                .map(|v| band_input.classifier.classify(v.to_value()))
                                .collect();
                            Ok(result)
                        }
                    }
                }}
            }
            let band_data = match band_input.classifier.value_type() {
                Value::Uint8(_) => extract_band!(u8),
                Value::Uint16(_) => extract_band!(u16),
                Value::Uint32(_) => extract_band!(u32),
                Value::Int16(_) => extract_band!(i16),
                Value::Int32(_) => extract_band!(i32),
                Value::Float32(_) => extract_band!(f32),
                Value::Float64(_) => extract_band!(f64),
            }?;
            input_data.push(band_data);
        };
        Ok(input_data)
    }

    pub fn convert_tiles(&self, num_threads: u32, tiles: Vec<Tile>, progress_sender: Option<Sender<ConversionProgress>>) -> Result<ConvertedRaster, &'static str> {
        let geotransformer = self.geotransformer.clone();

        /*
        let required_bands = self.inputs
            .iter()
            .map(|k| k.source_band)
            .collect::<HashSet<u8>>();
        */

        let (send_subset, recv_subset): (Sender<ConversionSubset>, Receiver<ConversionSubset>) = bounded(1);
        let (send_result, recv_result) = bounded(1);

        let mut join_handles = Vec::new();
        for _ in 0..num_threads {
            let thread_recv_subset = recv_subset.clone();
            let thread_send_result = send_result.clone();
            let join_handle = thread::spawn(move || {
                for subset in thread_recv_subset.iter() {
                    let tile_bounds = rect_from_coordinates(
                        subset.geotransformer.pixel_to_coordinate((
                            subset.tile.offset_origin.0 + subset.tile.size.0,
                            subset.tile.offset_origin.1 + subset.tile.size.1,
                        )),
                        subset.geotransformer.pixel_to_coordinate(subset.tile.offset_origin),
                    );

                    // switch algorithms depending on the ratio of non-empty-pixels to the approximate number
                    // of h3 indexes fitting into the tile
                    //    n_h3_indexes < n_pixels -> fill tile with h3 indexes and check the pixels at the h3 indexes
                    //    n_h3_indexes > n_pixels -> find pixel clusters -> fill each cluster with h3 indexes individually
                    let n_h3indexes = max(
                        (area_rect(&tile_bounds) / h3::hex_area_at_resolution(subset.h3_resolution as i32, AreaUnits::M2)).ceil() as usize,
                        1,
                    );
                    let n_pixels = subset.tile.size.0 * subset.tile.size.1;

                    let result = if (n_h3indexes as f64 * 0.9) as usize > n_pixels {
                        convert_subset_by_filtering_and_region_growing(tile_bounds, subset)
                    } else {
                        convert_subset_by_checking_index_positions(tile_bounds, subset)
                    };
                    thread_send_result.send(result).unwrap();
                }
            });
            join_handles.push(join_handle);
        }
        std::mem::drop(recv_subset); // no need to receive anything on this thread;
        std::mem::drop(send_result);

        let mut tiles = VecDeque::from(tiles);
        let tiles_total = tiles.len();
        let tiles_done = RefCell::new(0_usize);

        let notify_progress = |sender: Option<Sender<ConversionProgress>>| {
            if let Some(progress_sender) = sender {
                progress_sender.send(ConversionProgress {
                    tiles_total,
                    tiles_done: *tiles_done.borrow(),
                }).unwrap();
            }
        };
        notify_progress(progress_sender.clone());

        let mut build_subset = || {
            Ok(match tiles.pop_front() {
                Some(tile) => {
                    let subset = ConversionSubset {
                        tile: tile.clone(),
                        geotransformer: geotransformer.clone(),
                        banddata: self.extract_input_bands(&tile)?,
                        h3_resolution: self.h3_resolution,
                    };
                    Option::Some(subset)
                }
                None => None
            })
        };

        let mut grouped_indexes = GroupedH3Indexes::new();
        let mut grouped_indexes_add = |mut gi_new: GroupedH3Indexes| {
            for (attributes, mut compacted_stack) in gi_new.drain() {
                grouped_indexes.entry(attributes)
                    .or_insert_with(CompactedIndexStack::new)
                    .append(&mut compacted_stack)
            }
            *tiles_done.borrow_mut() += 1;
            notify_progress(progress_sender.clone());
        };

        let mut next_subset = build_subset()?;
        while next_subset.is_some() {
            select! {
                send(send_subset, next_subset.unwrap()) -> res => {
                    res.unwrap();
                    next_subset = build_subset()?;
                }
                recv(recv_result) -> result => {
                    match result {
                        Ok(received_grouped_indexes) => grouped_indexes_add(received_grouped_indexes),
                        Err(_) => return Err("failed to receive grouped indexes")
                    }
                }
            }
        }

        std::mem::drop(send_subset); // There is nothing left to send, so let the consumers shut down on channel close

        for received_grouped_indexes in recv_result.iter() {
            grouped_indexes_add(received_grouped_indexes)
        }

        // wait for threads to finish
        for join_handle in join_handles {
            join_handle.join().unwrap();
        }
        Ok(ConvertedRaster {
            value_types: self.inputs.iter().map(|c| c.classifier.value_type().clone()).collect(),
            indexes: grouped_indexes,
        })
    }

    pub fn convert(&self, num_threads: u32, tile_size: (usize, usize)) -> Result<ConvertedRaster, &'static str> {
        self.convert_tiles(num_threads, generate_tiles(self.dataset.size(), tile_size), None)
    }
}

#[inline]
fn pixel_to_array_position(tile_pixel: (usize, usize), tile_size: (usize, usize)) -> usize {
    (tile_pixel.1 * tile_size.0) + tile_pixel.0
}

#[inline]
fn array_position_to_pixel(array_pos: usize, tile_size: (usize, usize)) -> (usize, usize) {
    (array_pos / tile_size.0, array_pos % tile_size.0)
}

/// convert by pre-filtering the raster values reducing them to just the raster pixel which have
/// an actual value. After that the clusters of pixels are determinated using region growing.
///
/// On each of these pixel clusters a region growing of h3 indexes is performed until the complete
/// cluster is covered.
fn convert_subset_by_filtering_and_region_growing(tile_bounds: Rect<f64>, subset: ConversionSubset) -> GroupedH3Indexes {
    // zip the bands and hash by their location in the tile
    let mut attributes_by_pos: HashMap<_, _> = ZipMultiIter::new(&subset.banddata)
        .filter(|(_pos, attributes)| {
            // at least one value must not be None
            attributes.iter().any(|v| v.is_some())
        })
        .collect();

    let mut grouped_indexes = GroupedH3Indexes::new();
    let mut indexes_to_add = HashMap::new();

    while !attributes_by_pos.is_empty() {
        let (array_pos, _attributes) = attributes_by_pos.iter().next().unwrap();

        let cluster = grow_region_starting_with_index(&attributes_by_pos, *array_pos, subset.tile.size);

        let mut indexes_to_check = VecDeque::new();
        let mut indexes_scheduled: HashSet<H3Index> = HashSet::new();

        // find the first h3 index located inside the cluster
        for cluster_pos in cluster.iter() {
            let pixel_in_tile = array_position_to_pixel(*cluster_pos, subset.tile.size);

            // find the nearest h3 index for this pixel
            let index = h3::coordinate_to_h3index(
                &subset.geotransformer.pixel_to_coordinate((
                    subset.tile.offset_origin.0 + pixel_in_tile.1,
                    subset.tile.offset_origin.1 + pixel_in_tile.0
                )),
                subset.h3_resolution as i32,
            );

            let coordinate = h3::coordinate_from_h3index(index);
            if !rect_contains(&tile_bounds, &coordinate) {
                continue;
            }

            // reverse-check if the h3 index is located in the cluster, or outside of it
            let index_pos = pixel_to_array_position(subset.tile.to_tile_relative_pixel(
                subset.geotransformer.coordinate_to_pixel(coordinate)
            ), subset.tile.size);

            if cluster.contains(&index_pos) {
                indexes_to_check.push_back(index);
                indexes_scheduled.insert(index);
                break;
            }
        }

        let mut indexes_visited: HashSet<H3Index> = HashSet::new();

        // start h3 region growing from the first index of the cluster
        while let Some(this_index) = indexes_to_check.pop_front() {
            indexes_visited.insert(this_index);

            let this_coordinate = h3::coordinate_from_h3index(this_index);
            if !rect_contains(&tile_bounds, &this_coordinate) {
                continue;
            }
            let this_index_pos = pixel_to_array_position(subset.tile.to_tile_relative_pixel(
                subset.geotransformer.coordinate_to_pixel(this_coordinate)
            ), subset.tile.size);

            if !cluster.contains(&this_index_pos) {
                continue;
            }

            if let Some(attributes) = attributes_by_pos.get(&this_index_pos) {
                indexes_to_add.entry(attributes.clone()).or_insert_with(Vec::new).push(this_index);
                for neighbor in k_ring(this_index, 1).iter() {
                    if !(indexes_visited.contains(neighbor) || indexes_scheduled.contains(neighbor)) {
                        indexes_to_check.push_back(*neighbor);
                        indexes_scheduled.insert(*neighbor);
                    }
                }
            }
        }

        // remove the positions which were visited in this iteration
        cluster.iter()
            .for_each(|pos| { let _ = attributes_by_pos.remove(pos); });
    };

    // copy the collected into the grouped indexes to perform compacting
    for (attributes_ref, mut h3indexes) in indexes_to_add.drain() {
        let attributes = attributes_ref.iter()
            .map(|a| {
                match a {
                    Some(v) => Some(v.clone()),
                    None => None
                }
            }).collect();
        grouped_indexes.entry(attributes)
            .or_insert_with(CompactedIndexStack::new)
            .append_to_resolution(subset.h3_resolution, h3indexes.as_mut());
    }


    grouped_indexes
}

/// convert using a simple approach by just checking the pixel values at the center points of the h3
/// indexes
fn convert_subset_by_checking_index_positions(tile_bounds: Rect<f64>, subset: ConversionSubset) -> GroupedH3Indexes {
    let mut indexes_to_check = VecDeque::new();
    indexes_to_check.push_back(
        h3::coordinate_to_h3index(
            subset.geotransformer.pixel_to_coordinate(subset.tile.center_pixel()).borrow(),
            subset.h3_resolution as i32,
        )
    );

    let mut grouped_indexes = GroupedH3Indexes::new();

    // IMPROVEMENT: rewrite to use https://doc.rust-lang.org/std/collections/struct.BTreeSet.html#method.pop_first
    // once this leaves nightly
    let mut indexes_scheduled: HashSet<H3Index> = HashSet::new();
    let mut indexes_visited: HashSet<H3Index> = HashSet::new();
    let mut indexes_to_add: HashMap<Attributes, Vec<H3Index>> = HashMap::new();
    while let Some(this_h3index) = indexes_to_check.pop_front() {
        indexes_visited.insert(this_h3index);
        indexes_scheduled.remove(&this_h3index);

        let coordinate = h3::coordinate_from_h3index(this_h3index);
        if !rect_contains(&tile_bounds, &coordinate) {
            continue;
        }
        let array_pos = pixel_to_array_position(
            subset.tile.to_tile_relative_pixel(
                subset.geotransformer.coordinate_to_pixel(coordinate)
            ),
            subset.tile.size);

        let attributes: Vec<_> = subset.banddata.iter().map(|bd| {
            match bd.get(array_pos) {
                Some(v) => v.clone(),
                None => {
                    log::warn!("could not read value from band at index {}", array_pos);
                    None
                }
            }
        }).collect();

        // add when the attributes are not all None
        if attributes.iter().any(|a| a.is_some()) {
            let target_vec = indexes_to_add.entry(attributes.clone()).or_insert_with(Vec::new);
            target_vec.push(this_h3index);

            // attempt to save a bit of space by compacting what we got
            if target_vec.len() > 20_000 {
                grouped_indexes.entry(attributes)
                    .or_insert_with(CompactedIndexStack::new)
                    .append_to_resolution(subset.h3_resolution, target_vec);
            }
        }

        // check the neighbors
        // IMPROVEMENT: re-use the vector used within kring
        for neighbor in k_ring(this_h3index, 1).iter() {
            if !(indexes_visited.contains(neighbor) || indexes_scheduled.contains(neighbor)) {
                indexes_to_check.push_back(*neighbor);
                indexes_scheduled.insert(*neighbor);
            }
        }
    }

    for (attributes, mut h3indexes) in indexes_to_add.drain() {
        grouped_indexes.entry(attributes)
            .or_insert_with(CompactedIndexStack::new)
            .append_to_resolution(subset.h3_resolution, h3indexes.as_mut());
    }

    grouped_indexes
}

/// perform region growing to find all indexes connected indexes
///
/// diagonal neighbors will be treated as being part of the cluster
fn grow_region_starting_with_index<T>(index_hashmap: &HashMap<usize, T>, start_index: usize, tile_size: (usize, usize)) -> HashSet<usize> {
    let mut indexes_of_cluster = HashSet::new();
    let mut indexes_to_check = VecDeque::new();
    indexes_to_check.push_back(start_index);

    while let Some(next_index) = indexes_to_check.pop_back() {
        if !index_hashmap.contains_key(&next_index) {
            continue;
        }
        if indexes_of_cluster.contains(&next_index) {
            continue;
        }
        indexes_of_cluster.insert(next_index);
        let pos = array_position_to_pixel(next_index, tile_size);

        for i in -1..=1 {
            if (pos.0 == 0 && i == -1) || (pos.0 == tile_size.0 && i == 1) {
                continue; // stay inside the tile bounds
            }
            for j in -1..=1 {
                if (pos.1 == 0 && j == -1) || (pos.1 == tile_size.1 && j == 1) {
                    continue; // stay inside the tile bounds
                }
                let next_pos = ((pos.1 as isize + j) as usize, (pos.0 as isize + i) as usize);
                let map_key = pixel_to_array_position(next_pos, tile_size);
                if !indexes_of_cluster.contains(&map_key) {
                    indexes_to_check.push_back(map_key);
                }
            }
        }
    }
    indexes_of_cluster
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    use gdal::raster::Dataset;
    use gdal::vector::Driver;

    use crate::input::{ClassifiedBand, NoData, Value};
    use crate::rasterconverter::{grow_region_starting_with_index, pixel_to_array_position, RasterConverter};
    use std::env::temp_dir;

    #[test]
    fn test_convert() {
        let path = Path::new("data/land_shallow_topo_1024.tif");
        let dataset = Dataset::open(path).unwrap();

        let inputs = vec![
            ClassifiedBand {
                classifier: Box::new(NoData::new(Value::Uint8(0))),
                source_band: 2,
            },
        ];
        let converter = RasterConverter::new(dataset, inputs, 3).unwrap();

        let converted = converter.convert(2, (300, 300)).unwrap();
        /*
        for (attr, h3indexes) in converted.indexes.iter() {
            println!("a: {:?} -> {}", attr, h3indexes.len());
        }
        */

        // write to file
        let mut outfile = temp_dir();
        outfile.push("h3-from-raster.shp");
        println!("writing to {:?}", outfile);
        let _ = fs::remove_file(outfile.clone());
        let drv = Driver::get("ESRI Shapefile").unwrap();
        let mut ds = drv.create(&outfile).unwrap();

        converted.write_to_ogr_dataset(&mut ds, "l1", false, None).unwrap();
        drop(ds); // close

        let mut ds2 = gdal::vector::Dataset::open(&outfile).unwrap();
        let layer = ds2.layer(0).unwrap();
        assert!(layer.features().next().is_some());
    }

    #[test]
    fn test_grow_region_starting_with_index() {
        let indata: Vec<usize> = vec![
            0, 0, 0, 0, 0, 0, 0, 1, 0, 0,
            0, 0, 0, 0, 0, 0, 1, 1, 1, 0,
            0, 1, 1, 1, 1, 1, 0, 1, 0, 0,
            1, 1, 0, 0, 0, 0, 0, 0, 0, 1 // last one should not be found
        ];
        let inmap: HashMap<_, _> = indata.iter().enumerate().filter(|(_, v)| { **v != 0_usize }).collect();
        let tile_size = (10, 4);
        let start_index = pixel_to_array_position((7, 0), tile_size);
        let positions = grow_region_starting_with_index(&inmap, start_index, tile_size);
        assert_eq!(positions.len(), 12);
        positions.iter().for_each(|p| {
            assert_eq!(inmap.get(p), Some(&&1_usize))
        })
    }
}

