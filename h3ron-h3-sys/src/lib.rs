#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::upper_case_acronyms)]

//! low-level bindings to H3
//!
//! This crate includes the C sources for libh3, so compiling it requires a c-compiler and the `cmake`
//! build tool.

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
