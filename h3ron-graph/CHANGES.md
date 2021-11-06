# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

After version 0.12 the version numbers from the individual crates are decoupled from each other as releases are now done
without github actions and without having to coordinate the release process with the `h3ronpy`
python extension.

## h3ron-graph Unreleased

## h3ron-graph 0.2.0 - 2021-11-06

### Changed

- Simplified `GetGapBridgedCellNodes` trait
- Converted `Path` to an enum and added variant to support paths where origin == destination.
- Improved `ShortestPath` to support paths where origin == destination. Also added an unittest.

## h3ron-graph 0.1.0 - 2021-11-01

### Added

- Added initial version of this crate.
