# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

After version 0.12 the version numbers from the individual crates are decoupled from each other as releases are now
done without github actions and without having to coordinate the release process with the `h3ronpy`
python extension.

## h3ron Unreleased

## h3ron 0.13.0 - 2021-11-01

### Added
- `IndexVec<T>` to interface between libh3 and rust.
- `H3EdgesBuilder`:Creates H3Edges from cells while only requiring a single memory allocation when the struct is created.
- `KRingBuilder` for repeated creation of small k-rings while avoiding allocations for each cell.
- `neighbors_within_distance_window_or_default` iterator including a few simplified wrapper functions.
- add `CompactedCellVec::shrink_to_fit`
- added specialized collections based on `hashbrown` with `ahash` hashing. Added
  `ThreadPartionedMap` behind `use-rayon` feature.
- `HasH3Resolution` trait
- `H3Edge::cell_indexes` and `H3Edge::cell_indexes_unchecked` to get the origin and destination cell of an edge in one call.
- `H3Edge::reversed` and `H3Edge::reversed_unchecked` to get the edge in the reverse direction.
- `ContainsIndex` trait for collections.
- `ToMultiLineString` trait for `&[H3Edge]` and `Vec<H3Edge>`.
- Implemented `Deref` for `H3Cell` and `H3Edge` [#23](https://github.com/nmandery/h3ron/pull/23).
- All types implementing `Index` can have directions [#23](https://github.com/nmandery/h3ron/pull/23).
- Add `H3Edge::boundary_linestring`.
- Add `H3Edge::cell_centroid_distance_m` and  `H3Edge::cell_centroid_distance_m_at_resolution`.

### Changed
- Changed many return values from `Vec` to `IndexVec` to reduce the number of allocations by doing less moving around of `H3Index` types.
- Clean up measurement functions. Create `ExactArea` and `ExactLength` traits and move the measurement functions from `H3Cell` and `H3Edge`
  to these traits. Remove `AreaUnits` and move average-area functions to `H3Cell`.
- Fixed overflow in `H3Direction` [#23](https://github.com/nmandery/h3ron/pull/23).
- Make serde support feature-gated behind `use-serde`.


## h3ron [0.12.0] - 2021-08-10
### Added
- `change_cell_resolution` iterator

### Changed
- dependency updates
- Using `repr(transparent)` for `H3Cell` and `H3Edge` types.
- Removing `H3Index` from most of the API:
  - Changing all functions from `H3Index` parameters and return values to `H3Cell`/`H3Edge`. In the names of the functions the term "index" has also been replaced. 
  - Replacing the `ToH3Indexes` trait with `ToH3Cells`
  - Changed `H3CompactedVec` to `CompactedCellVec`
- Make `CompactedCellVec::add_cells` take a generic iterator and remove `add_indexes_from_iter`.
- remove `FromIterator<H3Index> for CompactedCellVec`

## h3ron [0.11.0] - 2021-06-12
### Added
- `Debug` trait implementations for `H3Cell` and `H3Edge` to display the `H3Index` in hexadecimal.
  [#16](https://github.com/nmandery/h3ron/pull/16)
- `ToLineString` trait providing `to_linestring` and `to_linestring_unchecked` methods to convert
  `H3Edge` to a `geo-types` `LineString<f64>`.
  
### Changed
- Update dependencies: `geo-types` 0.6->0.7


## h3ron [0.10.0] - 2021-04-24
### Added

- Edge indexes (named `H3Cell`) with edge length methods, validation, getting an edge between two
  hexagons. [#10](https://github.com/nmandery/h3ron/pull/10), [#11](https://github.com/nmandery/h3ron/pull/11)
    - Edges can retrieve their origin and destination hexagons.
    - Get a hexagons edges.
- Implementation of the relative H3 direction system (see https://h3geo.org/docs/core-library/h3Indexing)
  . [#13](https://github.com/nmandery/h3ron/pull/13)
- Implementing `Add` and `Sub` traits for `CoordIj`. [#9](https://github.com/nmandery/h3ron/issues/9)

### Changed
- Changing `Index` to a trait, the former `Index` renamed to `H3Cell`. [#10](https://github.com/nmandery/h3ron/pull/10)
  , [#11](https://github.com/nmandery/h3ron/pull/11), [#15](https://github.com/nmandery/h3ron/pull/15)
- Error handling improvements. [#10](https://github.com/nmandery/h3ron/pull/10)
  , [#11](https://github.com/nmandery/h3ron/pull/11)
- Renaming `CoordIJ` to `CoordIj` and `Error::NoLocalIJCoordinates` to `Error::NoLocalIjCoordinates` to follow clippy
  suggestions.

### Removed

## h3ron [0.9.0] - 2021-04-11
### Added

- `TryFrom` implementation to convert `u64` to `Index`
- Improved documentation. [#8](https://github.com/nmandery/h3ron/issues/8)
  , [#6](https://github.com/nmandery/h3ron/issues/6)
- `ToH3`-trait to convert `geotypes` geometries to H3 `Vec` instances.

### Changed
- Extending unittest for `CoordIJ`
- Fixing new clippy warnings after the upgrade to rust 1.51
- Introducing more checks in the API when traversing parent indexes and creating indexes from
  coordinates. [#8](https://github.com/nmandery/h3ron/issues/8)
- Improved index validation and error handling.

### Removed
- removed `From` implementation to convert `u64` to `Index`

## Earlier versions

The changes done in earlier versions where not documented in this changelog and can only be reconstructed from the
commits in git.

[0.12.0]: https://github.com/nmandery/h3ron/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/nmandery/h3ron/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/nmandery/h3ron/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/nmandery/h3ron/compare/v0.8.1...v0.9.0
