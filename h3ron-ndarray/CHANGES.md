# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

After version 0.12 the version numbers from the individual crates are decoupled from each other as releases are now
done without github actions and without having to coordinate the release process with the `h3ronpy`
python extension.

## h3ron-ndarray Unreleased

## h3ron-ndarray 0.13.0 - 2021-11-01
### Changed
- Documentation improvements
- Switch to `thiserror` crate for the error implementation.

## h3ron-ndarray [0.12.0] - 2021-08-10
### Changed
- dependency updates
- Return `CompactedCellVec`s from `raster::H3Converter::to_h3()`.

## h3ron-ndarray [0.11.0] - 2021-06-12
### Changed
- Update dependencies: `geo-types` 0.6->0.7, `ndarray` 0.14->0.15, `gdal` 0.7->0.8

## h3ron-ndarray [0.10.0] - 2021-04-24
no changes

## h3ron-ndarray [0.9.0] - 2021-04-11
### Changed
- Fixing new clippy warnings after the upgrade to rust 1.51

## Earlier versions

The changes done in earlier versions where not documented in this changelog and can only be reconstructed from the
commits in git.

[0.12.0]: https://github.com/nmandery/h3ron/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/nmandery/h3ron/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/nmandery/h3ron/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/nmandery/h3ron/compare/v0.8.1...v0.9.0
