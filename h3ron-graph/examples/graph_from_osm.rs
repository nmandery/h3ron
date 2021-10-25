// This example can also be used to build the benchmark data for `route_germany`.

use std::convert::TryFrom;
use std::fs::File;
use std::path::Path;

use clap::{App, Arg};
use h3ron::H3Edge;
use ordered_float::OrderedFloat;

use h3ron::io::serialize_into;
use h3ron_graph::formats::osm::osmpbfreader::Tags;
use h3ron_graph::formats::osm::{EdgeProperties, OsmPbfH3EdgeGraphBuilder, WayAnalyzer};
use h3ron_graph::graph::{GetStats, H3EdgeGraphBuilder, PreparedH3EdgeGraph};

struct MyWayAnalzer {}

impl WayAnalyzer<OrderedFloat<f64>> for MyWayAnalzer {
    type WayProperties = (OrderedFloat<f64>, bool);

    fn analyze_way_tags(&self, tags: &Tags) -> Option<Self::WayProperties> {
        // https://wiki.openstreetmap.org/wiki/Key:highway or https://wiki.openstreetmap.org/wiki/DE:Key:highway
        if let Some(highway_value) = tags.get("highway") {
            match highway_value.to_lowercase().as_str() {
                "motorway" | "motorway_link" | "trunk" | "trunk_link" | "primary"
                | "primary_link" => Some(3.0.into()),
                "secondary" | "secondary_link" => Some(4.0.into()),
                "tertiary" | "tertiary_link" => Some(5.0.into()),
                "unclassified" | "residential" | "living_street" | "service" => Some(8.0.into()),
                "road" => Some(9.0.into()),
                "pedestrian" => Some(50.0.into()), // fussgängerzone
                _ => None,
            }
            .map(|weight| {
                // oneway streets (https://wiki.openstreetmap.org/wiki/Key:oneway)
                // NOTE: reversed direction "oneway=-1" is not supported
                let is_bidirectional = tags
                    .get("oneway")
                    .map(|v| v.to_lowercase() != "yes")
                    .unwrap_or(true);
                (weight, is_bidirectional)
            })
        } else {
            None
        }
    }

    fn way_edge_properties(
        &self,
        _edge: H3Edge,
        way_properties: &Self::WayProperties,
    ) -> EdgeProperties<OrderedFloat<f64>> {
        // use the edge to make the WayProperties relative to the length of the edge (`cell_centroid_distance_m`)
        // or whatever else is desired
        EdgeProperties {
            is_bidirectional: way_properties.1,
            weight: way_properties.0,
        }
    }
}

fn main() {
    let app = App::new("graph_from_osm")
        .about("Build a routing graph from an OSM PBF file")
        .arg(
            Arg::with_name("h3_resolution")
                .short("r")
                .takes_value(true)
                .default_value("7"),
        )
        .arg(
            Arg::with_name("OUTPUT-GRAPH")
                .help("output file to write the graph to")
                .required(true),
        )
        .arg(
            Arg::with_name("OSM-PBF")
                .help("input OSM .pbf file")
                .required(true)
                .min_values(1),
        );

    let matches = app.get_matches();

    let h3_resolution: u8 = matches
        .value_of("h3_resolution")
        .unwrap()
        .parse()
        .expect("invalid h3 resolution");
    let graph_output = matches.value_of("OUTPUT-GRAPH").unwrap().to_string();

    let mut builder = OsmPbfH3EdgeGraphBuilder::new(h3_resolution, MyWayAnalzer {});
    for pbf_input in matches.values_of("OSM-PBF").unwrap() {
        builder
            .read_pbf(Path::new(&pbf_input))
            .expect("reading pbf failed");
    }
    let graph = builder.build_graph().expect("building graph failed");
    println!("Preparing graph");
    let prepared_graph = PreparedH3EdgeGraph::try_from(graph).expect("preparing the graph failed");

    let stats = prepared_graph.get_stats();
    println!(
        "Created a prepared graph ({} nodes, {} edges, {} long-edges)",
        stats.num_nodes,
        stats.num_edges,
        prepared_graph.num_long_edges()
    );
    let mut out_file = File::create(graph_output).expect("creating output file failed");
    serialize_into(&mut out_file, &prepared_graph, true).expect("writing graph failed");
}
