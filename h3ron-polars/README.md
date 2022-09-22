# h3ron-polars

[![Latest Version](https://img.shields.io/crates/v/h3ron-polars.svg)](https://crates.io/crates/h3ron-polars) [![Documentation](https://docs.rs/h3ron-polars/badge.svg)](https://docs.rs/h3ron-polars)

Integration of the [h3](https://h3geo.org) geospatial indexing system with [polars](https://docs.rs/polars) dataframes by providing extension traits 
to `UInt64Chunked` and `DataFrame`.

This integration does not aim to be feature-complete, it was created by moving functionalities implemented in other 
projects to this common crate for better usability. In case of missing features, please submit a PR.

Some features so far:

* Convert from `UInt64Chunked` to H3 cells and edges and vice versa.
* Algorithms on `UInt64Chunked` for building grid-disks, changing the cell resolution, deriving the bounding box and some more.
* Spatial-indexing of H3-cells and edges using the [kdbush](https://docs.rs/kdbush) spatial index. The spatial index 
  returns a `BooleanChunked` array suitable to be used with polars filters.
* Algorithms on `DataFrame` for [compacting/uncompacting](https://h3geo.org/docs/highlights/indexing) the contained data 
  by grouping the rows based on the remaining columns and applying compaction/uncompaction to the cell column.
