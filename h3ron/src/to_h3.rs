use geo::{
    GeometryCollection, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Rect,
    Triangle,
};
use geo_types::{Coordinate, Geometry, Line, Polygon};

use crate::collections::indexvec::IndexVec;
use crate::error::check_valid_h3_resolution;
use crate::{line, polyfill, Error, H3Cell, Index};
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
        Ok(polyfill(self, h3_resolution))
    }
}

impl ToH3Cells for MultiPolygon<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        let mut outvec = IndexVec::new();
        for poly in self.0.iter() {
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
        for pt in self.0.iter() {
            outvec.push(H3Cell::from_coordinate(&pt.0, h3_resolution)?.h3index());
        }
        outvec.try_into()
    }
}

impl ToH3Cells for Coordinate<f64> {
    fn to_h3_cells(&self, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
        check_valid_h3_resolution(h3_resolution)?;
        vec![H3Cell::from_coordinate(self, h3_resolution)?.h3index()].try_into()
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
        for ls in self.0.iter() {
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
        for geom in self.0.iter() {
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
