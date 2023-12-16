# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

After version 0.12 the version numbers from the individual crates are decoupled from each other as releases are now
done without github actions and without having to coordinate the release process with the `h3ronpy`
python extension.


## h3ron-h3-sys Unreleased

## h3ron-h3-sys 0.17.0 - 2023-12-16
### Changed
* Upgrade `bindgen` from 0.63 to 0.69 and rebuild the prebuild bindings.

## h3ron-h3-sys 0.16.0 - 2023-01-19
### Changed
* Upgrade `bindgen` from 0.61 to 0.63 and rebuild the prebuild bindings.
* Upgrade H3 from version 4.0.1 to 4.1.0

## h3ron-h3-sys 0.15.2 - 2022-10-25
### Changed
* Upgrade `bindgen` from 0.60 to 0.61.

## h3ron-h3-sys 0.15.1 - 2022-09-16

* Include pre-build bindings to drop the `bindgen` dependency as the default. Enabling the `bindgen`-feature
  allows creating the bindings during build.
* Update H3 to [v4.0.1](https://github.com/uber/h3/releases/tag/v4.0.1).

## h3ron-h3-sys 0.15.0 - 2022-08-24

* Drop the `cmake` build time dependency by building `libh3` with the `cc` crate. [#47](https://github.com/nmandery/h3ron/pull/47).

## h3ron-h3-sys 0.14.0 - 2022-08-23

* Upgrade to h3 v4.0.0

### Migration to H3 v4.0.0

* Added `geo-types` feature.

## h3ron-h3-sys 0.13.0 - 2022-01-23
### Changed
- Switch to rust edition 2021

## h3ron-h3-sys [0.12.0] - 2021-08-10
### Changed
- Silence `deref_nullptr` warnings in cbindgen generated bindings. #19
- Updated libh3 to v3.7.2
- dependency updates

## h3ron-h3-sys [0.11.0] - 2021-06-12
no changes

## h3ron-h3-sys [0.10.0] - 2021-04-24
no changes

## h3ron-h3-sys [0.9.0] - 2021-04-11 
no changes

## Earlier versions

The changes done in earlier versions where not documented in this changelog and can only be reconstructed from the
commits in git.

[0.12.0]: https://github.com/nmandery/h3ron/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/nmandery/h3ron/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/nmandery/h3ron/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/nmandery/h3ron/compare/v0.8.1...v0.9.0
