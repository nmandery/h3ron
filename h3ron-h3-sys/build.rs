extern crate bindgen;
extern crate regex;

use glob::glob;
use regex::Regex;
use std::env;
use std::fs::{create_dir_all, read_to_string, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn h3_version() -> (String, String, String) {
    let version_contents = read_to_string("libh3/VERSION").unwrap();
    let cap = Regex::new("^(?P<mj>[0-9]+)\\.(?P<mn>[0-9]+)\\.(?P<pt>[0-9]+)")
        .unwrap()
        .captures(&version_contents)
        .expect("version number not found");
    (
        cap["mj"].to_string(),
        cap["mn"].to_string(),
        cap["pt"].to_string(),
    )
}

fn configure_header() -> (PathBuf, PathBuf) {
    let mut include_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    include_dir.push("include");
    create_dir_all(&include_dir).unwrap();

    let (version_major, version_minor, version_patch) = h3_version();
    let header_contents = read_to_string("libh3/src/h3lib/include/h3api.h.in")
        .unwrap()
        .replace("@H3_VERSION_MAJOR@", &version_major)
        .replace("@H3_VERSION_MINOR@", &version_minor)
        .replace("@H3_VERSION_PATCH@", &version_patch);
    let mut h3api_header = include_dir.clone();
    h3api_header.push("h3api.h");
    write!(
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&h3api_header)
            .unwrap(),
        "{}",
        header_contents
    )
    .unwrap();

    (include_dir, h3api_header)
}

fn main() {
    println!("cargo:rerun-if-changed=libh3");
    let (configured_includes, h3api_header) = configure_header();

    cc::Build::new()
        .include("libh3/src/h3lib/include")
        .include(configured_includes)
        .files(glob("libh3/src/h3lib/lib/*.c").unwrap().map(|p| p.unwrap()))
        .compile("h3");

    let mut builder = bindgen::Builder::default().header(
        h3api_header
            .as_path()
            .as_os_str()
            .to_owned()
            .into_string()
            .expect("Path could not be converted to string"),
    );

    // read the contents of the header to extract functions and types
    let header_contents = read_to_string(h3api_header).expect("Unable to read h3 header");
    for cap in Regex::new(r"H3_EXPORT\(\s*(?P<func>[a-zA-Z0-9_]+)\s*\)")
        .unwrap()
        .captures_iter(&header_contents)
    {
        builder = builder.allowlist_function(&cap["func"]);
    }
    for cap in Regex::new(r"struct\s+\{[^\}]*}\s*(?P<type>[a-zA-Z0-9_]+)")
        .unwrap()
        .captures_iter(&header_contents)
    {
        builder = builder.allowlist_type(&cap["type"]);
    }
    // Finish the builder and generate the bindings.
    let bindings = builder
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
