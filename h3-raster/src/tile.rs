use std::cmp::min;

use gdal::raster::RasterBand;

#[derive(Debug, Clone)]
pub struct Tile {
    /// offset to origin (top-left corner)
    /// (x, y)
    pub offset_origin: (usize, usize),

    /// size of the tile (width, height)
    pub size: (usize, usize),
}


impl Tile {
    /// global center pixel of the tile
    pub fn center_pixel(&self) -> (usize, usize) {
        (
            self.offset_origin.0 + (self.size.0 as f64 / 2.0).floor() as usize,
            self.offset_origin.1 + (self.size.1 as f64 / 2.0).floor() as usize
        )
    }

    pub fn to_tile_relative_pixel(&self, pixel: (usize, usize)) -> (usize, usize) {
        (
            pixel.0 - self.offset_origin.0,
            pixel.1 - self.offset_origin.1,
        )
    }

    /// convert a pixel coordinate from the tile to a global pixel coordinate
    /// in the un-tiled dataset
    /// (x, y)
    pub fn get_global_coordinate(&self, c: (usize, usize)) -> (usize, usize) {
        (self.offset_origin.0 + c.0, self.offset_origin.1 + c.1)
    }
}


pub fn generate_tiles(full_size: (usize, usize), tile_size: (usize, usize)) -> Vec<Tile> {
    let mut tiles = vec![];
    for tx in 0..(full_size.0 as f64 / tile_size.0 as f64).ceil() as usize {
        for ty in 0..(full_size.1 as f64 / tile_size.1 as f64).ceil() as usize {
            tiles.push(Tile {
                offset_origin: (
                    tx * tile_size.0,
                    ty * tile_size.1,
                ),
                size: (
                    min(full_size.0 - (tx * tile_size.0), tile_size.0),
                    min(full_size.1 - (ty * tile_size.1), tile_size.1),
                ),
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
pub fn tile_size_from_rasterband(rasterband: &RasterBand) -> (usize, usize) {
    let block_size = rasterband.block_size();
    let band_size = rasterband.size();
    let mut tile_size = block_size;
    loop {
        let new_tile_size = (
            if tile_size.0 > tile_size.1 { tile_size.0 } else { tile_size.0 + block_size.0 },
            if tile_size.1 > tile_size.0 { tile_size.1 } else { tile_size.1 + block_size.1 },
        );
        if ((new_tile_size.0 * new_tile_size.1) > 4_000_000)
            || (new_tile_size.0 > band_size.0)
            || (new_tile_size.1 > band_size.1) {
            break;
        }
        tile_size = new_tile_size
    }
    tile_size
}


#[cfg(test)]
mod tests {
    use super::generate_tiles;

    #[test]
    fn test_tiles_equal_size() {
        let size = (1000, 1200);
        let tile_size = (200, 200);
        let tiles = generate_tiles(size, tile_size);

        // println!("XXX: {}", tiles.len());
        // println!("XXX: {:?}", tiles);

        assert_eq!(tiles.len(), 30);
        assert_eq!(tiles.last().unwrap().offset_origin, (800, 1000));
        assert_eq!(tiles.last().unwrap().size, tile_size);
    }

    #[test]
    fn test_tiles_not_equal_size() {
        let size = (990, 1180);
        let tile_size = (200, 200);
        let tiles = generate_tiles(size, tile_size);

        assert_eq!(tiles.len(), 30);
        assert_eq!(tiles.last().unwrap().offset_origin, (800, 1000));
        assert_eq!(tiles.first().unwrap().size, tile_size);
        assert_eq!(tiles.last().unwrap().size, (190, 180));
    }

}
