use geo::{
    GeometryCollection, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Rect,
    Triangle,
};
use geo_types::{Coordinate, Geometry, Line, Polygon};

use crate::error::check_valid_h3_resolution;
use crate::{line, polyfill, Error, HexagonIndex, Index};

/// convert to indexes at the given resolution
///
/// The output vec may contain duplicate indexes in case of
/// overlapping input geometries.
pub trait ToH3Indexes {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error>;
}

impl ToH3Indexes for Polygon<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        check_valid_h3_resolution(h3_resolution)?;
        let mut indexes = polyfill(&self, h3_resolution);
        Ok(indexes.drain(..).map(HexagonIndex::new).collect())
    }
}

impl ToH3Indexes for MultiPolygon<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        let mut outvec = vec![];
        for poly in self.0.iter() {
            let mut thisvec = poly.to_h3_indexes(h3_resolution)?;
            outvec.append(&mut thisvec);
        }
        Ok(outvec)
    }
}

impl ToH3Indexes for Point<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        check_valid_h3_resolution(h3_resolution)?;
        Ok(vec![HexagonIndex::from_coordinate(&self.0, h3_resolution)?])
    }
}

impl ToH3Indexes for MultiPoint<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        let mut outvec = vec![];
        for pt in self.0.iter() {
            outvec.push(HexagonIndex::from_coordinate(&pt.0, h3_resolution)?);
        }
        Ok(outvec)
    }
}

impl ToH3Indexes for Coordinate<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        check_valid_h3_resolution(h3_resolution)?;
        Ok(vec![HexagonIndex::from_coordinate(&self, h3_resolution)?])
    }
}

impl ToH3Indexes for LineString<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        check_valid_h3_resolution(h3_resolution)?;
        let mut indexes = line(&self, h3_resolution)?;
        Ok(indexes.drain(..).map(HexagonIndex::new).collect())
    }
}

impl ToH3Indexes for MultiLineString<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        let mut outvec = vec![];
        for ls in self.0.iter() {
            let mut thisvec = ls.to_h3_indexes(h3_resolution)?;
            outvec.append(&mut thisvec);
        }
        Ok(outvec)
    }
}

impl ToH3Indexes for Rect<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        self.to_polygon().to_h3_indexes(h3_resolution)
    }
}

impl ToH3Indexes for Triangle<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        self.to_polygon().to_h3_indexes(h3_resolution)
    }
}

impl ToH3Indexes for Line<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        LineString::from(vec![self.start, self.end]).to_h3_indexes(h3_resolution)
    }
}

impl ToH3Indexes for GeometryCollection<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        let mut outvec = vec![];
        for geom in self.0.iter() {
            let mut thisvec = geom.to_h3_indexes(h3_resolution)?;
            outvec.append(&mut thisvec);
        }
        Ok(outvec)
    }
}

impl ToH3Indexes for Geometry<f64> {
    fn to_h3_indexes(&self, h3_resolution: u8) -> Result<Vec<HexagonIndex>, Error> {
        match self {
            Geometry::Point(pt) => pt.to_h3_indexes(h3_resolution),
            Geometry::Line(l) => l.to_h3_indexes(h3_resolution),
            Geometry::LineString(ls) => ls.to_h3_indexes(h3_resolution),
            Geometry::Polygon(poly) => poly.to_h3_indexes(h3_resolution),
            Geometry::MultiPoint(mp) => mp.to_h3_indexes(h3_resolution),
            Geometry::MultiLineString(mls) => mls.to_h3_indexes(h3_resolution),
            Geometry::MultiPolygon(mpoly) => mpoly.to_h3_indexes(h3_resolution),
            Geometry::GeometryCollection(gc) => gc.to_h3_indexes(h3_resolution),
            Geometry::Rect(r) => r.to_h3_indexes(h3_resolution),
            Geometry::Triangle(tr) => tr.to_h3_indexes(h3_resolution),
        }
    }
}
