# h3ron

[h3](https://h3geo.org) library for [rust](https://rustlang.org)

This repository consists of multiple crates:

* [h3ron-h3-sys](./h3ron-h3-sys): bindgen-generated bindings for statically linked libh3.
* [h3ron](./h3ron): high level rust API including collections for selected parts of H3.
* [h3ron-ndarray](./h3ron-ndarray): Integration with the [ndarray](https://github.com/rust-ndarray/ndarray) crate to generate H3 data from raster data (using [gdal](https://github.com/georust/gdal), ...)

## Why this name?

Well, coming up with a good name for a project while avoiding naming conflicts is hard. On the other hand are animal-based names always pretty easy to remember.

How to pronounce it? I got no idea - probably like the [heron bird family](https://en.wikipedia.org/wiki/Heron).

## License

[MIT](./LICENSE-MIT)