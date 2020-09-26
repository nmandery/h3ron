use std::cmp::max;
use std::collections::HashMap;
use std::convert::TryFrom;

use crossbeam::channel::{bounded, Receiver, Sender};
use gdal::raster::{Buffer, Dataset, RasterBand};
use gdal::raster::types::GdalType;
use gdal::spatial_ref::SpatialRef;
use geo::algorithm::centroid::Centroid;
use geo_types::{Coordinate, Polygon, Rect};

use h3::{max_k_ring_size, polyfill};
use h3::index::Index;
use h3::stack::H3IndexStack;
use h3_util::progress::ProgressPosition;

use crate::convertedraster::{ConvertedRaster, GroupedH3Indexes};
use crate::error::Error;
use crate::geo::{area_linearring, area_rect, rect_from_coordinates};
use crate::geotransform::GeoTransformer;
use crate::input::{ClassifiedBand, Classifier, ToValue, Value};
use crate::tile::{Dimensions, generate_tiles, Tile};

pub struct ConversionProgress {
    pub tiles_total: usize,
    pub tiles_done: usize,
}

impl ProgressPosition for ConversionProgress {
    fn position(&self) -> u64 { self.tiles_done as u64 }
}

pub struct RasterConverter {
    dataset: Dataset,
    inputs: Vec<ClassifiedBand>,
    geotransformer: GeoTransformer,
    h3_resolution: u8,
}


fn position_to_pixel(tile: &Tile, pos: usize) -> Result<Coordinate<usize>, Error> {
    let pixel = Coordinate {
        x: pos % tile.size.width,
        y: pos / tile.size.width,
    };
    if pixel.x > tile.size.width || pixel.y > tile.size.height {
        Err(Error::OutOfBounds)
    } else {
        Ok(pixel)
    }
}

fn pixel_to_position(tile: &Tile, tile_pixel: &Coordinate<usize>) -> Result<usize, Error> {
    Ok((tile_pixel.y * tile.size.width) + tile_pixel.x)
}


struct SparseCoordinateMap<T> {
    //pub inner: AHashMap<usize, T>,
    pub inner: HashMap<usize, T>,
    pub tile: Tile,
    pub geotransformer: GeoTransformer,
}

impl<T> SparseCoordinateMap<T> {
    pub fn new(tile: Tile, geotransformer: GeoTransformer) -> Self {
        Self {
            inner: Default::default(),
            tile,
            geotransformer,
        }
    }

    pub fn bounds(&self) -> Rect<f64> {
        rect_from_coordinates(
            self.geotransformer.pixel_to_coordinate(
                &self.tile.get_global_coordinate(&Coordinate { x: self.tile.size.width, y: self.tile.size.height })),
            self.geotransformer.pixel_to_coordinate(&self.tile.offset_origin),
        )
    }

    pub fn value_at_coordinate(&self, c: &Coordinate<f64>) -> Result<Option<(usize, &T)>, Error> {
        let pos = self.coordinate_to_position(c)?;
        let value = if let Some(v) = self.inner.get(&pos) {
            Some((pos, v))
        } else {
            None
        };
        Ok(value)
    }

    #[allow(dead_code)]
    pub fn value_at_position(&self, pos: &usize) -> Option<&T> {
        self.inner.get(pos)
    }

    /// return a random value, or None when the Map is empty
    pub fn random_value(&self) -> Option<(usize, &T)> {
        if let Some((pos, value)) = self.inner.iter().next() {
            Some((*pos, value))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn remove(&mut self, position: &usize) -> Option<T> {
        self.inner.remove(position)
    }

    /// convert the key of the inner map to the pixel coordinates within
    /// the tile
    fn position_to_pixel(&self, pos: usize) -> Result<Coordinate<usize>, Error> {
        position_to_pixel(&self.tile, pos)
    }

    /// convert the pixel coordinates from the tile to the key of the inner map
    fn pixel_to_position(&self, tile_pixel: &Coordinate<usize>) -> Result<usize, Error> {
        pixel_to_position(&self.tile, tile_pixel)
    }

    pub fn position_to_coordinate(&self, pos: usize) -> Result<Coordinate<f64>, Error> {
        let pixel = self.position_to_pixel(pos)?;
        let c = self.geotransformer.pixel_to_coordinate(
            &self.tile.get_global_coordinate(&pixel)
        );
        Ok(c)
    }

    pub fn coordinate_to_position(&self, c: &Coordinate<f64>) -> Result<usize, Error> {
        let pixel = {
            let pixel_in_dataset = self.geotransformer.coordinate_to_pixel(c);
            // make the pixel relative to the tile, not the dataset
            self.tile.to_tile_relative_pixel(&pixel_in_dataset)?
        };
        self.pixel_to_position(&pixel)
    }
}

type ValueSparseCoordinateMap = SparseCoordinateMap<Vec<Option<Value>>>;


fn classifiy_band_and_append<T: Copy + GdalType + ToValue>(scm: &mut ValueSparseCoordinateMap, num_values: usize, band_i: usize, band: &RasterBand, tile: &Tile, classifier: &Box<dyn Classifier>) -> Result<(), Error> {
    let window = (tile.offset_origin.x as isize, tile.offset_origin.y as isize);
    let tile_size = (tile.size.width, tile.size.height);
    let bd: Buffer<T> = band.read_as(window, tile_size, tile_size)?;
    for (i, raw_value) in bd.data.iter().enumerate() {
        let classified_value = classifier.classify(raw_value.to_value());
        if classified_value.is_some() {
            let entry = scm.inner.entry(i)
                .or_insert_with(|| vec![None; num_values]);
            entry[band_i] = classified_value;
        }
    }
    Ok(())
}

impl RasterConverter {
    pub fn new(dataset: Dataset, inputs: Vec<ClassifiedBand>, h3_resolution: u8) -> Result<Self, Error> {
        let required_max_band = inputs
            .iter()
            .map(|k| k.source_band)
            .fold(0, max);

        if required_max_band > dataset.count() as u8 {
            return Err(Error::BandOutOfRange);
        }

        // input projection has to be WGS84. Checking if possible, otherwise
        // it is assumed that the SRS is correct
        let proj_str = dataset.projection();
        if !proj_str.is_empty() {
            if let (Ok(sr), Ok(sr4326)) = (SpatialRef::from_definition(&proj_str), SpatialRef::from_epsg(4326)) {
                if sr != sr4326 {
                    return Err(Error::InvalidSRS);
                }
            }
        }
        if h3_resolution > 15 {
            return Err(Error::H3ResolutionOutOfRange);
        }
        let geotransform = dataset.geo_transform()
            .map_err(|_| Error::NoGeotransformFound)?;

        Ok(RasterConverter {
            dataset,
            inputs,
            geotransformer: GeoTransformer::try_from(geotransform)?,
            h3_resolution,
        })
    }


    fn extract_input_bands(&self, tile: &Tile, geotransformer: &GeoTransformer) -> Result<ValueSparseCoordinateMap, Error> {
        let mut scm = ValueSparseCoordinateMap::new(tile.clone(), geotransformer.clone());
        for (band_i, band_input) in self.inputs.iter().enumerate() {
            let band = self.dataset.rasterband(band_input.source_band as isize)?;
            match band_input.classifier.value_type() {
                Value::Uint8(_) => classifiy_band_and_append::<u8>(&mut scm, self.inputs.len(), band_i, &band, tile, &band_input.classifier)?,
                Value::Uint16(_) => classifiy_band_and_append::<u16>(&mut scm, self.inputs.len(), band_i, &band, tile, &band_input.classifier)?,
                Value::Uint32(_) => classifiy_band_and_append::<u32>(&mut scm, self.inputs.len(), band_i, &band, tile, &band_input.classifier)?,
                Value::Int16(_) => classifiy_band_and_append::<i16>(&mut scm, self.inputs.len(), band_i, &band, tile, &band_input.classifier)?,
                Value::Int32(_) => classifiy_band_and_append::<i32>(&mut scm, self.inputs.len(), band_i, &band, tile, &band_input.classifier)?,
                Value::Float32(_) => classifiy_band_and_append::<f32>(&mut scm, self.inputs.len(), band_i, &band, tile, &band_input.classifier)?,
                Value::Float64(_) => classifiy_band_and_append::<f64>(&mut scm, self.inputs.len(), band_i, &band, tile, &band_input.classifier)?,
            };
        };
        Ok(scm)
    }

    pub fn convert_tiles(&self, num_threads: u32, tiles: Vec<Tile>, progress_sender: Option<Sender<ConversionProgress>>, compact: bool) -> Result<ConvertedRaster, Error> {
        let tiles_total = tiles.len();
        crossbeam::scope(|scope| {
            let (send_scm, recv_scm): (Sender<ValueSparseCoordinateMap>, Receiver<ValueSparseCoordinateMap>) = bounded(num_threads as usize);
            let (send_result, recv_result) = bounded(num_threads as usize);
            let (send_final_result, recv_final_result) = bounded(1);

            for _ in 0..num_threads {
                let thread_recv_scm = recv_scm.clone();
                let thread_send_result = send_result.clone();
                let thread_h3_resolution = self.h3_resolution;
                scope.spawn(move |_| {
                    for scm in thread_recv_scm.iter() {
                        if !scm.is_empty() {
                            let tile_bounds = scm.bounds();
                            let centroid_index = Index::from_coordinate(&tile_bounds.centroid().0, thread_h3_resolution);

                            // switch algorithms depending on the expected workload of the tile
                            // of h3 indexes fitting into the tile
                            let n_h3indexes_per_tile = max(
                                (area_rect(&tile_bounds) / area_linearring(&centroid_index.polygon().exterior())).ceil() as usize,
                                1,
                            );
                            let n_h3indexes_per_pixel = n_h3indexes_per_tile as f64 / (scm.tile.size.width * scm.tile.size.height) as f64;
                            let ring_max_distance = (6.0 * n_h3indexes_per_pixel.sqrt()).ceil() as u32;

                            // assumes a somewhat clustered spatial distribution of the pixels within the tile
                            let expected_indexes_to_visit_for_ring_growing = (
                                scm.inner.len() as f64
                                    * max_k_ring_size(ring_max_distance) as f64
                                    * (1.0 - 0.7) // expect a 70% coverage of pixels within a kring
                            ).ceil() as usize + (scm.inner.len() as f64 * n_h3indexes_per_pixel).ceil() as usize;
                            /*
                            log::debug!(
                                "n_h3indexes_per_tile: {} , expected_indexes_to_visit_for_ring_growing: {}",
                                n_h3indexes_per_tile,
                                expected_indexes_to_visit_for_ring_growing
                            );
                             */

                            let mut grouped_indexes = if n_h3indexes_per_tile > expected_indexes_to_visit_for_ring_growing {
                                //log::debug!("convert_by_filtering_and_region_growing(ring_max_distance={})\n", ring_max_distance);
                                convert_by_growing_rings(ring_max_distance, thread_h3_resolution, scm)
                            } else {
                                //log::debug!("convert_by_checking_index_positions\n");
                                convert_by_checking_index_positions(tile_bounds, thread_h3_resolution, scm, compact)
                            };
                            if compact {
                                for stack in grouped_indexes.values_mut() {
                                    stack.compact();
                                }
                            }
                            thread_send_result.send(grouped_indexes).expect("sending result failed");
                        }
                    }
                });
            }
            std::mem::drop(recv_scm); // no need to receive anything on this thread;
            std::mem::drop(send_result);

            scope.spawn(move |_| {
                let mut grouped_indexes = GroupedH3Indexes::new();
                let mut tiles_done = 0;
                for mut gi in recv_result.iter() {
                    for (attributes, mut compacted_stack) in gi.drain() {
                        grouped_indexes.entry(attributes)
                            .or_insert_with(H3IndexStack::new)
                            .append(&mut compacted_stack, false)
                    }

                    if let Some(ps) = &progress_sender {
                        tiles_done += 1;
                        ps.send(ConversionProgress {
                            tiles_total,
                            tiles_done,
                        }).unwrap();
                    }
                }
                // do the compacting just once instead of at each append to
                // trade an increased memory usage for a better processing time
                if compact {
                    for (_, ci) in grouped_indexes.iter_mut() {
                        ci.compact();
                    }
                }
                send_final_result.send(grouped_indexes).unwrap()
            });

            for tile in tiles.iter() {
                let scm = self.extract_input_bands(tile, &self.geotransformer).unwrap();
                send_scm.send(scm).unwrap();
            }
            std::mem::drop(send_scm); // no need to receive anything on this thread;

            ConvertedRaster {
                value_types: self.inputs.iter().map(|c| c.classifier.value_type().clone()).collect(),
                indexes: recv_final_result.recv().unwrap(),
            }
        }).map_err(|e| {
            log::error!("conversion failed: {:?}", e);
            Error::ConversionFailed
        })
    }

    pub fn convert(&self, num_threads: u32, tile_size: &Dimensions, compact: bool) -> Result<ConvertedRaster, Error> {
        let tiles = generate_tiles(
            &self.dataset.size().into(),
            tile_size,
        );
        self.convert_tiles(num_threads, tiles, None, compact)
    }
}


/// convert by pre-filtering the raster values reducing them to just the raster pixel which have
/// an actual value. After that the clusters of pixels are determinated using k_rings.
fn convert_by_growing_rings(ring_max_distance: u32, h3_resolution: u8, mut scm: ValueSparseCoordinateMap) -> GroupedH3Indexes {
    let mut grouped_indexes = GroupedH3Indexes::new();
    while let Some((position, _attributes)) = scm.random_value() {
        //let mut max_distance_per_position: AHashMap<usize, u32> = Default::default();
        let mut max_distance_per_position: HashMap<usize, u32> = Default::default();
        max_distance_per_position.insert(position, 0_u32);

        if let Ok(coordinate) = scm.position_to_coordinate(position) {
            let index = Index::from_coordinate(&coordinate, h3_resolution);

            // grow a ring around the index to grow a "bubble" in which to check for other indexes
            let other_indexes = match index.hex_range_distances(0, ring_max_distance) {
                Ok(other_indexes) => {
                    other_indexes
                }
                Err(_) => {
                    // hex_ring is the cheapest to calculate, but may fail in pentagons and some other cases
                    // so we use a fallback
                    index.k_ring_distances(0, ring_max_distance)
                }
            };

            for (distance, other_index) in other_indexes {
                let other_coordinate = other_index.coordinate();
                if let Ok(Some((other_position, other_attributes))) = scm.value_at_coordinate(&other_coordinate) {

                    // add to grouped_indexes
                    let stack = match grouped_indexes.get_mut(other_attributes) {
                        Some(stack) => stack,
                        None => {
                            grouped_indexes.insert(other_attributes.clone(), H3IndexStack::new());
                            grouped_indexes.get_mut(other_attributes).unwrap()
                        }
                    };
                    stack.indexes_by_resolution.entry(h3_resolution)
                        .and_modify(|v| { v.push(other_index.h3index()) })
                        .or_insert_with(|| {
                            let mut v = vec![];
                            v.push(other_index.h3index());
                            v
                        });

                    // schedule for a later removal
                    max_distance_per_position.entry(other_position)
                        .and_modify(|v| {
                            if distance > *v {
                                *v = distance
                            }
                        })
                        .or_insert(distance);
                }
            }
        }
        for (pos, pos_max_distance) in max_distance_per_position.drain() {
            // allow overlapps to be sure to not remove pixels, which may have only
            // been partially covered.
            if pos_max_distance < ring_max_distance {
                scm.remove(&pos);
            }
        }
    }

    for stack in grouped_indexes.values_mut() {
        // there will be duplicated h3indexes caused by overlapping rings
        stack.dedup();
    }
    grouped_indexes
}

/// convert using a simple approach by just checking the pixel values at the center points of the h3
/// indexes
fn convert_by_checking_index_positions(tile_bounds: Rect<f64>, h3_resolution: u8, scm: ValueSparseCoordinateMap, compact: bool) -> GroupedH3Indexes {
    let mut grouped_indexes = GroupedH3Indexes::new();

    for h3index in polyfill(&Polygon::from(tile_bounds), h3_resolution).drain(..) {
        let index = Index::from(h3index);
        if let Ok(Some((_position, attributes))) = scm.value_at_coordinate(&index.coordinate()) {
            // add when the attributes are not all None
            if attributes.iter().any(|a| a.is_some()) {
                let stack = match grouped_indexes.get_mut(attributes) {
                    Some(stack) => stack,
                    None => {
                        grouped_indexes.insert(attributes.clone(), H3IndexStack::new());
                        grouped_indexes.get_mut(attributes).unwrap()
                    }
                };
                let target_vec = stack.indexes_by_resolution.entry(h3_resolution).or_insert_with(Vec::new);
                target_vec.push(index.h3index());

                // attempt to save a bit of space by compacting
                if compact && target_vec.len() % 10_000 == 0 {
                    stack.compact();
                }
            }
        }
    }

    grouped_indexes
}


#[cfg(test)]
mod tests {
    use geo_types::Coordinate;

    use crate::rasterconverter::{pixel_to_position, position_to_pixel};
    use crate::tile::Tile;

    #[test]
    fn test_converting_pixels_to_positions() {
        let tile = Tile {
            offset_origin: Coordinate { x: 0_usize, y: 0_usize },
            size: (200_usize, 300_usize).into(),
        };

        let pos = pixel_to_position(&tile, &Coordinate { x: 60_usize, y: 1_usize }).unwrap();
        assert_eq!(pos, 200 + 60);
        let pixel = position_to_pixel(&tile, pos).unwrap();
        assert_eq!(pixel, Coordinate { x: 60_usize, y: 1_usize });
    }
}

