use std::cmp::min;

use gdal::raster::RasterBand;
use geo_types::{Coordinate, Rect};

use crate::geo::rect_from_coordinates;
use crate::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Dimensions {
    pub width: usize,
    pub height: usize,
}

impl From<(usize, usize)> for Dimensions {
    fn from(t: (usize, usize)) -> Self {
        Dimensions {
            width: t.0,
            height: t.1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Tile {
    /// offset to origin (top-left corner)
    pub offset_origin: Coordinate<usize>,

    /// size of the tile
    pub size: Dimensions,
}


impl Tile {
    /// global center pixel of the tile
    pub fn center_pixel(&self) -> Coordinate<usize> {
        Coordinate {
            x: self.offset_origin.x + (self.size.width as f64 / 2.0).round() as usize,
            y: self.offset_origin.y + (self.size.height as f64 / 2.0).round() as usize,
        }
    }


    pub fn to_tile_relative_pixel(&self, pixel: &Coordinate<usize>) -> Result<Coordinate<usize>,Error> {
        if pixel.x < self.offset_origin.x || pixel.y < self.offset_origin.y {
            Err(Error::OutOfBounds)
        } else {
            Ok(Coordinate {
                x: pixel.x - self.offset_origin.x,
                y: pixel.y - self.offset_origin.y,
            })
        }
    }

    /// convert a pixel coordinate from the tile to a global pixel coordinate
    /// in the un-tiled dataset
    /// (x, y)
    pub fn get_global_coordinate(&self, c: &Coordinate<usize>) -> Coordinate<usize> {
        Coordinate {
            x: self.offset_origin.x + c.x,
            y: self.offset_origin.y + c.y,
        }
    }

    pub fn bounds(&self) -> Rect<usize> {
        rect_from_coordinates(
            self.offset_origin,
            Coordinate {
                x: self.offset_origin.x + self.size.width,
                y: self.offset_origin.y + self.size.height,
            },
        )
    }
}


pub fn generate_tiles(full_size: &Dimensions, tile_size: &Dimensions) -> Vec<Tile> {
    let mut tiles = vec![];
    for tx in 0..(full_size.width as f64 / tile_size.width as f64).ceil() as usize {
        for ty in 0..(full_size.height as f64 / tile_size.height as f64).ceil() as usize {
            tiles.push(Tile {
                offset_origin: Coordinate {
                    x: tx * tile_size.width,
                    y: ty * tile_size.height,
                },
                size: Dimensions {
                    width: min(full_size.width - (tx * tile_size.width), tile_size.width),
                    height: min(full_size.height - (ty * tile_size.height), tile_size.height),
                },
            })
        }
    }
    tiles
}

/// derive the tile size from one of the bands blocksize, to make reading
/// data from gdal more efficient without using gdals block api.
///
/// attempts to make the tiles as square as possible while keeping an upper
/// limit on the number of pixels. The generated tiles align with the blocks of the
/// raster band for more efficient IO.
///
/// see https://gis.stackexchange.com/questions/292754/efficiently-read-large-tif-raster-to-a-numpy-array-with-gdal
pub fn tile_size_from_rasterband(rasterband: &RasterBand, min_num_tiles: usize) -> Dimensions {
    let block_size = rasterband.block_size();
    let band_size = rasterband.size();
    let threshold = min(
        (band_size.0 * band_size.1) / min_num_tiles,
        4_000_000
    );
    let mut tile_size = block_size;
    loop {
        let new_tile_size = (
            if tile_size.0 > tile_size.1 { tile_size.0 } else { tile_size.0 + block_size.0 },
            if tile_size.1 > tile_size.0 { tile_size.1 } else { tile_size.1 + block_size.1 },
        );
        if ((new_tile_size.0 * new_tile_size.1) >= threshold)
            || (new_tile_size.0 > band_size.0)
            || (new_tile_size.1 > band_size.1) {
            break;
        }
        tile_size = new_tile_size
    }
    tile_size.into()
}


#[cfg(test)]
mod tests {
    use geo_types::Coordinate;

    use crate::tile::Dimensions;

    use super::generate_tiles;

    #[test]
    fn test_tiles_equal_size() {
        let size = Dimensions {
            width: 1000,
            height: 1200,
        };
        let tile_size = Dimensions {
            width: 200,
            height: 200,
        };
        let tiles = generate_tiles(&size, &tile_size);

        assert_eq!(tiles.len(), 30);
        assert_eq!(tiles.last().unwrap().offset_origin, Coordinate { x: 800, y: 1000 });
        assert_eq!(tiles.last().unwrap().size, tile_size);
    }

    #[test]
    fn test_tiles_not_equal_size() {
        let size = Dimensions {
            width: 990,
            height: 1180,
        };
        let tile_size = Dimensions {
            width: 200,
            height: 200,
        };
        let tiles = generate_tiles(&size, &tile_size);

        assert_eq!(tiles.len(), 30);
        assert_eq!(tiles.last().unwrap().offset_origin, Coordinate { x: 800, y: 1000 });
        assert_eq!(tiles.first().unwrap().size, tile_size);
        assert_eq!(tiles.last().unwrap().size, Dimensions { width: 190, height: 180 });
    }
}
