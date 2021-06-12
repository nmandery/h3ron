# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### h3ron crate

#### Added

- `Debug` trait implementations for `H3Cell` and `H3Edge` to display the `H3Index` in hexadecimal.
  [#16](https://github.com/nmandery/h3ron/pull/16)
- `ToLineString` trait providing `to_linestring` and `to_linestring_unchecked` methods to convert
  `H3Edge` to a `geo-types` `LineString<f64>`.
  
#### Changed

- Update dependencies: `geo-types` 0.6->0.7

### h3ron-ndarray crate

#### Changed

- Update dependencies: `geo-types` 0.6->0.7, `ndarray` 0.14->0.15, `gdal` 0.7->0.8

### h3ronpy

#### Added

- Support for transforming `numpy.float32` and `numpy.float64` raster arrays to H3 dataframes by warping the array values in `OrderedFloat<T>`.

#### Changed

- Fix `ValueError` when converting empty dataframes. [#17](https://github.com/nmandery/h3ron/issues/17)
- Deprecate `h3ronpy.util.h3index_column_to_geodataframe` in favor of `h3ronpy.util.dataframe_to_geodataframe`.
- Update dependencies: `geo-types` 0.6->0.7, `ndarray` 0.14->0.15


## [0.10.0] - 2021-04-24

### h3ron crate

#### Added

- Edge indexes (named `H3Cell`) with edge length methods, validation, getting an edge between two
  hexagons. [#10](https://github.com/nmandery/h3ron/pull/10), [#11](https://github.com/nmandery/h3ron/pull/11)
    - Edges can retrieve their origin and destination hexagons.
    - Get a hexagons edges.
- Implementation of the relative H3 direction system (see https://h3geo.org/docs/core-library/h3Indexing)
  . [#13](https://github.com/nmandery/h3ron/pull/13)
- Implementing `Add` and `Sub` traits for `CoordIj`. [#9](https://github.com/nmandery/h3ron/issues/9)

#### Changed

- Changing `Index` to a trait, the former `Index` renamed to `H3Cell`. [#10](https://github.com/nmandery/h3ron/pull/10)
  , [#11](https://github.com/nmandery/h3ron/pull/11), [#15](https://github.com/nmandery/h3ron/pull/15)
- Error handling improvements. [#10](https://github.com/nmandery/h3ron/pull/10)
  , [#11](https://github.com/nmandery/h3ron/pull/11)
- Renaming `CoordIJ` to `CoordIj` and `Error::NoLocalIJCoordinates` to `Error::NoLocalIjCoordinates` to follow clippy
  suggestions.

#### Removed

### h3ron-ndarray crate

### h3ron-h3-sys crate

### h3ronpy

#### Added

- Unittests for `raster_to_dataframe` and `geodataframe_to_h3` using `pytest`

#### Changed

#### Removed

### other

- CI improvements. [#7](https://github.com/nmandery/h3ron/issues/7)

## [0.9.0] - 2021-04-11

### h3ron crate

#### Added

- `TryFrom` implementation to convert `u64` to `Index`
- Improved documentation. [#8](https://github.com/nmandery/h3ron/issues/8)
  , [#6](https://github.com/nmandery/h3ron/issues/6)
- `ToH3`-trait to convert `geotypes` geometries to H3 `Vec` instances.

#### Changed

- Extending unittest for `CoordIJ`
- Fixing new clippy warnings after the upgrade to rust 1.51
- Introducing more checks in the API when traversing parent indexes and creating indexes from
  coordinates. [#8](https://github.com/nmandery/h3ron/issues/8)
- Improved index validation and error handling.

#### Removed

- removed `From` implementation to convert `u64` to `Index`

### h3ron-ndarray crate

#### Changed

- Fixing new clippy warnings after the upgrade to rust 1.51

### h3ron-h3-sys crate

### h3ronpy

#### Added

- Integration with geopandas `GeoDataFrame` to convert the contained geometries to H3.
- Update of `maturin` to 0.10.2

#### Changed

- Simplified API of raster integration.

## Earlier versions

The changes done in earlier versions where not documented in this changelog and can only be reconstructed from the
commits in git.

[Unreleased]: https://github.com/nmandery/h3ron/compare/v0.10.0...HEAD

[0.10.0]: https://github.com/nmandery/h3ron/compare/v0.10.0...v0.9.0
[0.9.0]: https://github.com/nmandery/h3ron/compare/v0.8.1...v0.9.0
