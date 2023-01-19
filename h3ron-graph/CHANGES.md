# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

After version 0.12 the version numbers from the individual crates are decoupled from each other as releases are now done
without github actions and without having to coordinate the release process with the `h3ronpy`
python extension.

## h3ron-graph Unreleased

## h3ron-graph 0.6.0 - 2023-01-19
### Changed
* Removed `io_gdal` feature and GDAL dependency.

## h3ron-graph 0.5.0 - 2022-10-25
### Changed
* Upgrade `geo` from 0.22 to 0.23, `gdal` from 0.12 to 0.13.

## h3ron-graph 0.4.0 - 2022-08-23

This version includes the migration from H3 version 3.x to 4.x. This includes some renaming of functions and
structs to stay somewhat consistent [with the changes made in H3](https://github.com/uber/h3/releases/tag/v4.0.0-rc3)
as well as making most functions return `Result<T, Error>` as H3 now returns error codes in most functions of its API.

### Added

* `h3ron::io` module has been relocated to `h3ron_graph:io::serde_util` as it is fairly custom and only used there. Requires the `io_serde_util` feature.

### Changed

* Refactored `GetGapBridgedCellNodes` trait to `NearestGraphNodes`
* Make OSM parsing failable.
* Make path transformation functions failable.
* Rename `ShortestPathOptions::num_gap_cells_to_graph` to `max_distance_to_graph`.
* Modify `Path` to contain the intended origin and destination cells.
* Upgrade `geo` dependency to 0.21.
* Optimize `ShortestPath` to reduce overheads. This change also lead to the introduction of the `GetCellEdges` trait.
* According to the naming-convention used by crates like `arrow2` the feature gates have been renamed. `use-gdal` became `io_gdal` and `osm` became `io_osm`. The `osm` module has been moved into the `io` module.
* Reduced the stack-footprint of the `OwnedEDgeValue` struct. This allows holding larger graphs in memory.

## h3ron-graph 0.3.0 - 2022-01-23

### Added

* added `WithinWeightThreshold` and `WithinWeightThresholdMany` traits.
* add `Path::cells()`.
* Implemented `h3ron::to_geo::ToLineString` for `LongEdge`.
* Add `Path::length_m()`.

### Changed

* Upgraded gdal from 0.10 to 0.12
* Re-export algorithm traits from `algorithm` module.
- The replacement of `CompressedIndexVec` with `IndexBlock` in `h3ron` required making a few `LongEdge` failable.
- Switch to rust edition 2021

## h3ron-graph 0.2.0 - 2021-11-06

### Changed

- Simplified `GetGapBridgedCellNodes` trait
- Converted `Path` to an enum and added variant to support paths where origin == destination.
- Improved `ShortestPath` to support paths where origin == destination. Also added an unittest.

## h3ron-graph 0.1.0 - 2021-11-01

### Added

- Added initial version of this crate.
