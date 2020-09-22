use std::convert::TryFrom;

use gdal::raster::dataset::GeoTransform;
use geo_types::{Coordinate, Rect};

use crate::geo::rect_from_coordinates;
use crate::error::Error;

#[derive(Clone)]
pub struct GeoTransformer {
    geotransform: GeoTransform,
    inv_geotransform: GeoTransform,
}

impl GeoTransformer {
    /// Convert a coordinate to the pixel coordinate in the dataset.
    ///
    /// Will return pixel coordinates outside of the bounds of the dataset when
    /// the coordinates are outside of the envelope of the raster.
    pub fn coordinate_to_pixel(&self, coordinate: &Coordinate<f64>) -> Coordinate<usize> {
        // ported from https://github.com/OSGeo/gdal/blob/master/gdal/apps/gdallocationinfo.cpp#L282
        Coordinate {
            x: (self.inv_geotransform[0] + (self.inv_geotransform[1] * coordinate.x)
                + (self.inv_geotransform[2] * coordinate.y)).floor() as usize,
            y: (self.inv_geotransform[3] + (self.inv_geotransform[4] * coordinate.x)
                + (self.inv_geotransform[5] * coordinate.y)).floor() as usize,
        }
    }

    /// Convert a pixel coordinate to the geo-coordinate
    pub fn pixel_to_coordinate(&self, pixel: &Coordinate<usize>) -> Coordinate<f64> {
        // ported form https://github.com/OSGeo/gdal/blob/18bfbd32302f611bde0832f61ca0747d4c4421dd/gdal/apps/gdalinfo_lib.cpp#L1443
        Coordinate {
            x: self.geotransform[0] + (self.geotransform[1] * pixel.x as f64)
                + (self.geotransform[2] * pixel.y as f64),
            y: self.geotransform[3] + (self.geotransform[4] * pixel.x as f64)
                + (self.geotransform[5] * pixel.y as f64),
        }
    }

    /// generate to boundingbox from the size of a gdal dataset
    #[allow(dead_code)]
    pub fn bounds_from_size(&self, size: (usize, usize)) -> Rect<f64> {
        let c1 = self.pixel_to_coordinate(&Coordinate { x: 0, y: 0 });
        let c2 = self.pixel_to_coordinate(&Coordinate { x: size.0, y: size.1 });
        rect_from_coordinates(c1, c2)
    }
}

impl TryFrom<GeoTransform> for GeoTransformer {
    type Error = Error;

    fn try_from(geotransform: GeoTransform) -> Result<Self, Self::Error> {
        let mut inv_geotransform = GeoTransform::default();
        let mut gt = geotransform;
        let res = unsafe { gdal_sys::GDALInvGeoTransform(gt.as_mut_ptr(), inv_geotransform.as_mut_ptr()) };
        if res == 0 {
            Err(Error::GeotransformFailed)
        } else {
            Ok(GeoTransformer { geotransform: gt, inv_geotransform })
        }
    }
}