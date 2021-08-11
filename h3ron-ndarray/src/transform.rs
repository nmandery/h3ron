use std::ops::Mul;

use geo_types::{Coordinate, Rect};

use crate::error::Error;

/// Affine Geotransform
///
/// Ported from [affine library](https://github.com/sgillies/affine/blob/master/affine/__init__.py) (used by rasterio).
///
/// `a`, `b`, `c`, `d`, `e` and `f` are typed as `f64` and are coefficients of an augmented affine
/// transformation matrix:
///
/// ```text
///   | x' |   | a  b  c | | x |
///   | y' | = | d  e  f | | y |
///   | 1  |   | 0  0  1 | | 1 |
/// ```
///
/// `a`, `b`, and `c` are the elements of the first row of the matrix. `d`, `e`, and `f` are the elements of the second row.
///
/// Other sources:
/// * [GDAL geotransform](https://gdal.org/tutorials/geotransforms_tut.html)
/// * [rasterio 1.0+ vs. GDAL](https://rasterio.readthedocs.io/en/latest/topics/migrating-to-v1.html#affine-affine-vs-gdal-style-geotransforms)
///
#[derive(Clone, PartialEq, Debug)]
pub struct Transform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Transform {
    #![allow(clippy::many_single_char_names)]
    pub fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// create from an f64 slice in the ordering used by rasterio
    pub fn from_rasterio(transform: &[f64; 6]) -> Self {
        Self::new(
            transform[0],
            transform[1],
            transform[2],
            transform[3],
            transform[4],
            transform[5],
        )
    }

    /// create from an f64 slice in the ordering used by gdal
    pub fn from_gdal(transform: &[f64; 6]) -> Self {
        Self::new(
            transform[1],
            transform[2],
            transform[0],
            transform[4],
            transform[5],
            transform[3],
        )
    }

    /// The determinant of the transform matrix
    pub fn determinant(&self) -> f64 {
        self.a * self.e - self.b * self.d
    }

    /// True if this transform is degenerate.
    ///
    /// Which means that it will collapse a shape to an effective area
    /// of zero. Degenerate transforms cannot be inverted.
    pub fn is_degenerate(&self) -> bool {
        self.determinant() == 0.0
    }

    pub fn invert(&self) -> Result<Self, Error> {
        if self.is_degenerate() {
            return Err(Error::TransformNotInvertible);
        }

        let idet = 1.0 / self.determinant();
        let ra = self.e * idet;
        let rb = -self.b * idet;
        let rd = -self.d * idet;
        let re = self.a * idet;
        Ok(Self::new(
            ra,
            rb,
            -self.c * ra - self.f * rb,
            rd,
            re,
            -self.c * rd - self.f * re,
        ))
    }
}

/// apply the transformation to a coordinate
impl Mul<&Coordinate<f64>> for &Transform {
    type Output = Coordinate<f64>;

    fn mul(self, rhs: &Coordinate<f64>) -> Self::Output {
        Coordinate {
            x: rhs.x as f64 * self.a + rhs.y as f64 * self.b + self.c,
            y: rhs.x as f64 * self.d + rhs.y as f64 * self.e + self.f,
        }
    }
}

/// apply the transformation to a rect
impl Mul<&Rect<f64>> for &Transform {
    type Output = Rect<f64>;

    fn mul(self, rhs: &Rect<f64>) -> Self::Output {
        Rect::new(self * &rhs.min(), self * &rhs.max())
    }
}

#[cfg(test)]
mod tests {
    /*
    $ gdalinfo data/r.tiff
    Driver: GTiff/GeoTIFF
    Files: data/r.tiff
    Size is 2000, 2000
    Coordinate System is:
    GEOGCRS["WGS 84",
        DATUM["World Geodetic System 1984",
            ELLIPSOID["WGS 84",6378137,298.257223563,
                LENGTHUNIT["metre",1]]],
        PRIMEM["Greenwich",0,
            ANGLEUNIT["degree",0.0174532925199433]],
        CS[ellipsoidal,2],
            AXIS["geodetic latitude (Lat)",north,
                ORDER[1],
                ANGLEUNIT["degree",0.0174532925199433]],
            AXIS["geodetic longitude (Lon)",east,
                ORDER[2],
                ANGLEUNIT["degree",0.0174532925199433]],
        ID["EPSG",4326]]
    Data axis to CRS axis mapping: 2,1
    Origin = (8.113770000000001,49.407919999999997)
    Pixel Size = (0.001196505000000,-0.001215135000000)
    Metadata:
      AREA_OR_POINT=Area
    Image Structure Metadata:
      COMPRESSION=LZW
      INTERLEAVE=BAND
    Corner Coordinates:
    Upper Left  (   8.1137700,  49.4079200) (  8d 6'49.57"E, 49d24'28.51"N)
    Lower Left  (   8.1137700,  46.9776500) (  8d 6'49.57"E, 46d58'39.54"N)
    Upper Right (  10.5067800,  49.4079200) ( 10d30'24.41"E, 49d24'28.51"N)
    Lower Right (  10.5067800,  46.9776500) ( 10d30'24.41"E, 46d58'39.54"N)
    Center      (   9.3102750,  48.1927850) (  9d18'36.99"E, 48d11'34.03"N)
    Band 1 Block=2000x4 Type=Byte, ColorInterp=Gray
      NoData Value=0
     */

    use geo_types::Coordinate;

    use crate::transform::Transform;

    fn r_tiff_test_helper(gt: &Transform) {
        // upper left pixel
        let px_ul = Coordinate { x: 0., y: 0. };

        let coord_ul = gt * &px_ul;
        assert_relative_eq!(coord_ul.x, 8.11377);
        assert_relative_eq!(coord_ul.y, 49.40792);

        let gt_inv = gt.invert().unwrap();
        let px_ul_back = &gt_inv * &coord_ul;
        assert_relative_eq!(px_ul_back.x, 0.0);
        assert_relative_eq!(px_ul_back.y, 0.0);
    }

    #[test]
    fn test_r_tiff_from_gdal() {
        /*
        Python 3.8.5 (default, Jul 28 2020, 12:59:40)
        [GCC 9.3.0] on linux
        >>> from osgeo import gdal
        >>> ds = gdal.Open("data/r.tiff")
        >>> ds.GetGeoTransform()
        (8.11377, 0.0011965049999999992, 0.0, 49.40792, 0.0, -0.001215135)
         */
        let gt = Transform::from_gdal(&[
            8.11377,
            0.0011965049999999992,
            0.0,
            49.40792,
            0.0,
            -0.001215135,
        ]);
        r_tiff_test_helper(&gt);
    }

    #[test]
    fn test_r_tiff_from_rasterio() {
        /*
        Python 3.8.5 (default, Jul 28 2020, 12:59:40)
        [GCC 9.3.0] on linux
         >>> import rasterio
        >>> ds = rasterio.open("data/r.tiff")
        >>> ds.transform
        Affine(0.0011965049999999992, 0.0, 8.11377,
               0.0, -0.001215135, 49.40792)
         */
        let gt = Transform::from_rasterio(&[
            0.0011965049999999992,
            0.0,
            8.11377,
            0.0,
            -0.001215135,
            49.40792,
        ]);
        r_tiff_test_helper(&gt);
    }
}
