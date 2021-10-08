use std::convert::TryInto;
use std::fs::File;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use geo_types::Coordinate;
use ordered_float::OrderedFloat;

use h3ron::io::deserialize_from;
use h3ron::{H3Cell, HasH3Resolution};
use h3ron_graph::algorithm::shortest_path::{DefaultShortestPathOptions, ShortestPath};
use h3ron_graph::graph::prepared::PreparedH3EdgeGraph;

//use std::io::Write;

fn load_bench_graph() -> PreparedH3EdgeGraph<OrderedFloat<f64>> {
    let graph: PreparedH3EdgeGraph<OrderedFloat<f64>> = deserialize_from(
        File::open(format!(
            "{}/../data/graph-germany_r7_f64.bincode.lz",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    graph
}

fn route_across_germany(routing_graph: &PreparedH3EdgeGraph<OrderedFloat<f64>>) {
    let origin_cell = H3Cell::from_coordinate(
        &Coordinate::from((9.834909439086914, 47.68708804564653)), // Wangen im Allgäu
        routing_graph.h3_resolution(),
    )
    .unwrap();

    let destination_cells = vec![
        H3Cell::from_coordinate(
            &Coordinate::from((7.20600128173828, 53.3689915114596)), // Emden
            routing_graph.h3_resolution(),
        )
        .unwrap(),
        H3Cell::from_coordinate(
            &Coordinate::from((13.092269897460938, 54.3153216473314)), // Stralsund
            routing_graph.h3_resolution(),
        )
        .unwrap(),
    ];

    let paths = routing_graph
        .shortest_path(
            origin_cell,
            destination_cells,
            &DefaultShortestPathOptions::default(),
        )
        .unwrap();
    assert_eq!(paths.len(), 2);
    let mut features = Vec::with_capacity(paths.len());
    for path in paths.iter() {
        // the following would fail if a non-consecutive path is generated
        let linestring = geo_types::Geometry::from(path.to_linestring().unwrap());
        let feature = geojson::Feature {
            bbox: None,
            geometry: (&linestring).try_into().ok(),
            id: None,
            properties: None,
            foreign_members: None,
        };
        features.push(feature);
    }
    /*
    let fc = geojson::FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };
    let mut filename = std::env::temp_dir();
    filename.push("route_germany.geojson");
    let mut out_file = File::create(filename).unwrap();
    write!(&mut out_file, "{}", fc.to_string()).unwrap();

     */
}

fn criterion_benchmark(c: &mut Criterion) {
    let routing_graph = load_bench_graph();

    let mut group = c.benchmark_group("route_many_to_many");
    // group.sample_size(10);
    group.bench_function("route_across germany", |b| {
        b.iter(|| route_across_germany(black_box(&routing_graph)))
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
