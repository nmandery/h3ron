use geo_types::{
    Coord, Geometry, GeometryCollection, Line, LineString, MultiLineString, MultiPoint,
    MultiPolygon, Point, Polygon, Rect, Triangle,
};

use crate::collections::indexvec::IndexVec;
use crate::error::check_valid_h3_resolution;
use crate::{line, Error, H3Cell, Index};
use h3ron_h3_sys::{GeoLoop, GeoPolygon, LatLng};
use std::os::raw::c_int;

use std::convert::TryInto;

/// convert to indexes at the given resolution
///
/// The output vec may contain duplicate indexes in case of
/// overlapping input geometries.
pub trait ToH3Cells {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error>;
}

impl ToH3Cells for Polygon<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        check_valid_h3_resolution(h3_resolution)?;
        polygon_to_cells(self, h3_resolution)
    }
}

impl ToH3Cells for MultiPolygon<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        let mut outvec = IndexVec::new();
        for poly in &self.0 {
            let mut thisvec = poly.to_h3_cells(h3_resolution)?;
            outvec.append(&mut thisvec);
        }
        Ok(outvec)
    }
}

impl ToH3Cells for Point<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        self.0.to_h3_cells(h3_resolution)
    }
}

impl ToH3Cells for MultiPoint<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        let mut outvec = vec![];
        for pt in &self.0 {
            outvec.push(H3Cell::from_coordinate(pt.0, h3_resolution)?.h3index());
        }
        outvec.try_into()
    }
}

impl ToH3Cells for Coord<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        check_valid_h3_resolution(h3_resolution)?;
        vec![H3Cell::from_coordinate(*self, h3_resolution)?.h3index()].try_into()
    }
}

impl ToH3Cells for LineString<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        check_valid_h3_resolution(h3_resolution)?;
        line(self, h3_resolution)
    }
}

impl ToH3Cells for MultiLineString<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        let mut outvec = IndexVec::new();
        for ls in &self.0 {
            let mut thisvec = ls.to_h3_cells(h3_resolution)?;
            outvec.append(&mut thisvec);
        }
        Ok(outvec)
    }
}

impl ToH3Cells for Rect<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        self.to_polygon().to_h3_cells(h3_resolution)
    }
}

impl ToH3Cells for Triangle<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        self.to_polygon().to_h3_cells(h3_resolution)
    }
}

impl ToH3Cells for Line<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        LineString::from(vec![self.start, self.end]).to_h3_cells(h3_resolution)
    }
}

impl ToH3Cells for GeometryCollection<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        let mut outvec = IndexVec::new();
        for geom in &self.0 {
            let mut thisvec = geom.to_h3_cells(h3_resolution)?;
            outvec.append(&mut thisvec);
        }
        Ok(outvec)
    }
}

impl ToH3Cells for Geometry<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        match self {
            Geometry::Point(pt) => pt.to_h3_cells(h3_resolution),
            Geometry::Line(l) => l.to_h3_cells(h3_resolution),
            Geometry::LineString(ls) => ls.to_h3_cells(h3_resolution),
            Geometry::Polygon(poly) => poly.to_h3_cells(h3_resolution),
            Geometry::MultiPoint(mp) => mp.to_h3_cells(h3_resolution),
            Geometry::MultiLineString(mls) => mls.to_h3_cells(h3_resolution),
            Geometry::MultiPolygon(mpoly) => mpoly.to_h3_cells(h3_resolution),
            Geometry::GeometryCollection(gc) => gc.to_h3_cells(h3_resolution),
            Geometry::Rect(r) => r.to_h3_cells(h3_resolution),
            Geometry::Triangle(tr) => tr.to_h3_cells(h3_resolution),
        }
    }
}

fn to_geoloop(ring: &mut Vec<LatLng>) -> GeoLoop {
    GeoLoop {
        numVerts: ring.len() as c_int,
        verts: ring.as_mut_ptr(),
    }
}

fn with_geopolygon<F, O>(poly: &Polygon<f64>, inner_fn: F) -> O
where
    F: Fn(&GeoPolygon) -> O,
{
    let mut exterior: Vec<LatLng> = linestring_to_latlng_vec(poly.exterior());
    let mut interiors: Vec<Vec<LatLng>> = poly
        .interiors()
        .iter()
        .map(linestring_to_latlng_vec)
        .collect();

    let mut holes: Vec<GeoLoop> = interiors.iter_mut().map(to_geoloop).collect();

    let geo_polygon = GeoPolygon {
        geoloop: to_geoloop(&mut exterior),
        numHoles: holes.len() as c_int,
        holes: holes.as_mut_ptr(),
    };
    inner_fn(&geo_polygon)
}

#[inline]
fn linestring_to_latlng_vec(ls: &LineString<f64>) -> Vec<LatLng> {
    ls.points().map(LatLng::from).collect()
}

fn max_polygon_to_cells_size_internal(gp: &GeoPolygon, h3_resolution: u8) -> Result<usize, Error> {
    let mut cells_size: i64 = 0;
    Error::check_returncode(unsafe {
        h3ron_h3_sys::maxPolygonToCellsSize(gp, c_int::from(h3_resolution), 0, &mut cells_size)
    })?;
    Ok(cells_size as usize)
}

pub fn max_polygon_to_cells_size(poly: &Polygon<f64>, h3_resolution: u8) -> Result<usize, Error> {
    with_geopolygon(poly, |gp| {
        max_polygon_to_cells_size_internal(gp, h3_resolution)
    })
}

pub fn polygon_to_cells(poly: &Polygon<f64>, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
    with_geopolygon(poly, |gp| {
        match max_polygon_to_cells_size_internal(gp, h3_resolution) {
            Ok(cells_size) => {
                // pre-allocate for the expected number of hexagons
                let mut index_vec = IndexVec::with_length(cells_size);

                Error::check_returncode(unsafe {
                    h3ron_h3_sys::polygonToCells(
                        gp,
                        c_int::from(h3_resolution),
                        0,
                        index_vec.as_mut_ptr(),
                    )
                })
                .map(|_| index_vec)
            }
            Err(e) => Err(e),
        }
    })
}
