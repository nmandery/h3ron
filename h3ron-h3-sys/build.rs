extern crate bindgen;
extern crate cmake;
extern crate regex;

use cmake::Config;
use regex::Regex;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=libh3");

    // build h3ron as a static library
    let dst_path = Config::new("libh3")
        .define("BUILD_BENCHMARKS", "OFF")
        .define("BUILD_FILTERS", "OFF")
        .define("BUILD_GENERATORS", "OFF")
        .define("BUILD_TESTING", "OFF")
        .define("ENABLE_COVERAGE", "OFF")
        .define("ENABLE_DOCS", "OFF")
        .define("ENABLE_FORMAT", "OFF")
        .define("ENABLE_LINTING", "OFF")
        .build();

    // link to the static library we just build
    println!("cargo:rustc-link-lib=static=h3");
    println!(
        "cargo:rustc-link-search=native={}",
        dst_path.join("lib").display()
    );

    let header_path = dst_path.join("include/h3/h3api.h");
    let mut builder = bindgen::Builder::default().header(
        dst_path
            .join("include/h3/h3api.h")
            .into_os_string()
            .into_string()
            .expect("Path could not be converted to string"),
    );

    // read the contents of the header to extract functions and types
    let header_contents = fs::read_to_string(header_path).expect("Unable to read h3 header");
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
