//! Low-level bindings to H3
//!
//! This crate includes the C sources for libh3, so compiling it requires a C toolchain.
//!
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::upper_case_acronyms)]
// https://github.com/nmandery/h3ron/issues/19
// https://github.com/rust-lang/rust-bindgen/issues/1651
#![allow(deref_nullptr)]

#[cfg(feature = "geo-types")]
pub mod geo;

#[cfg(feature = "bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(not(feature = "bindgen"))]
include!("prebuild_bindings.rs");
