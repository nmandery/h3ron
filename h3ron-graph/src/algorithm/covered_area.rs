use std::borrow::Borrow;

use geo::algorithm::simplify::Simplify;
use geo_types::{MultiPolygon, Polygon};

use h3ron::collections::H3CellSet;
use h3ron::iter::change_cell_resolution;
use h3ron::{H3Cell, ToLinkedPolygons};

use crate::error::Error;

/// calculates a [`MultiPolygon`] of the area covered by a graph
pub trait CoveredArea {
    /// calculates a [`MultiPolygon`] of the area covered by a graph
    ///
    /// As the resulting geometry will be quite complex, it is recommended
    /// to reduce the h3 resolution using `reduce_resolution_by`. A value of 3
    /// will make the calculation based on resolution 7 for a graph of resolution 10.
    /// Reducing the resolution leads to a overestimation of the area.
    ///
    /// A slight simplification will be applied to the output geometry and
    /// eventual holes will be removed.
    fn covered_area(&self, reduce_resolution_by: u8) -> Result<MultiPolygon<f64>, Error>;
}

/// calculates a [`MultiPolygon`] of the area covered by a [`H3Cell`] iterator.
pub fn cells_covered_area<I>(
    cell_iter: I,
    cell_iter_resolution: u8,
    reduce_resolution_by: u8,
) -> Result<MultiPolygon<f64>, Error>
where
    I: IntoIterator,
    I::Item: Borrow<H3Cell>,
{
    let t_res = cell_iter_resolution.saturating_sub(reduce_resolution_by);
    let mut cells: H3CellSet = change_cell_resolution(cell_iter.into_iter(), t_res).collect();
    let cell_vec: Vec<_> = cells.drain().collect();
    let mp = MultiPolygon::from(
        cell_vec
            // remove the number of vertices by smoothing
            .to_linked_polygons(true)
            .drain(..)
            // reduce the number of vertices again and discard all holes
            .map(|p| Polygon::new(p.exterior().simplify(&0.000001), vec![]))
            .collect::<Vec<_>>(),
    );
    Ok(mp)
}
