use crate::spatial_index::SpatialIndex;
use geo_types::{Coordinate, Rect};
use polars_core::datatypes::BooleanChunked;
use rstar::RTree;

pub struct RTreeIndex {}
