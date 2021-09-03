fetch-data:
    mkdir -p data
    wget --unlink https://download.geofabrik.de/europe/germany-latest.osm.pbf -O data/germany-latest.osm.pbf

generate-testdata:
     cargo run --release --all-features -p h3ron-graph --example graph_from_osm -- -r 7 data/graph-germany_r7_f64.bincode.lz data/germany-latest.osm.pbf

clippy:
    cargo clippy --all-targets --all-features

test:
    cargo test --workspace --all-targets --all-features
