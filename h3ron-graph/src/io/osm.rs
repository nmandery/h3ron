//! Support for OpenStreetMap data formats

use std::io::BufReader;
use std::ops::Add;
use std::path::Path;

use geo_types::{Coord, LineString};
pub use osmpbfreader;
use osmpbfreader::{OsmPbfReader, Tags};

use h3ron::collections::HashMap;
use h3ron::iter::continuous_cells_to_edges;
use h3ron::H3DirectedEdge;

use crate::error::Error;
use crate::graph::{H3EdgeGraph, H3EdgeGraphBuilder};

/// hide errors in the io error to avoid having osmpbfreader in the public api.
impl From<osmpbfreader::Error> for Error {
    fn from(g_err: osmpbfreader::Error) -> Self {
        Self::IOError(std::io::Error::new(std::io::ErrorKind::Other, g_err))
    }
}

pub struct EdgeProperties<T> {
    pub is_bidirectional: bool,
    pub weight: T,
}

pub trait WayAnalyzer<T> {
    type WayProperties;

    /// analyze the tags of an Way and return `Some` when this way should be used
    fn analyze_way_tags(&self, tags: &Tags) -> Result<Option<Self::WayProperties>, Error>;

    /// return the weight for a single `H3Edge`
    fn way_edge_properties(
        &self,
        edge: H3DirectedEdge,
        way_properties: &Self::WayProperties,
    ) -> Result<EdgeProperties<T>, Error>;
}

/// Builds [`H3EdgeGraph`] instances from .osm.pbf files.
pub struct OsmPbfH3EdgeGraphBuilder<
    T: PartialOrd + PartialEq + Add + Copy + Sync + Send,
    WA: WayAnalyzer<T>,
> {
    h3_resolution: u8,
    way_analyzer: WA,
    graph: H3EdgeGraph<T>,
}

impl<T, WA> OsmPbfH3EdgeGraphBuilder<T, WA>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Sync,
    WA: WayAnalyzer<T>,
{
    pub fn new(h3_resolution: u8, way_analyzer: WA) -> Self {
        Self {
            h3_resolution,
            way_analyzer,
            graph: H3EdgeGraph::new(h3_resolution),
        }
    }

    pub fn read_pbf(&mut self, pbf_path: &Path) -> Result<(), Error> {
        let pbf_file = BufReader::new(std::fs::File::open(pbf_path)?);
        let mut pbf = OsmPbfReader::new(pbf_file);
        let mut nodeid_coordinates: HashMap<_, _> = Default::default();
        for obj_result in pbf.iter() {
            let obj = obj_result?;
            match obj {
                osmpbfreader::OsmObj::Node(node) => {
                    let coordinate = Coord {
                        x: node.lon(),
                        y: node.lat(),
                    };
                    nodeid_coordinates.insert(node.id, coordinate);
                }
                osmpbfreader::OsmObj::Way(way) => {
                    if let Some(way_props) = self.way_analyzer.analyze_way_tags(&way.tags)? {
                        let coordinates: Vec<_> = way
                            .nodes
                            .iter()
                            .filter_map(|node_id| nodeid_coordinates.get(node_id).copied())
                            .collect();
                        if coordinates.len() >= 2 {
                            let h3indexes: Vec<_> =
                                h3ron::line(&LineString::from(coordinates), self.h3_resolution)?
                                    .into();

                            for edge_result in continuous_cells_to_edges(h3indexes) {
                                let edge = edge_result?;
                                let edge_props =
                                    self.way_analyzer.way_edge_properties(edge, &way_props)?;

                                self.graph.add_edge(edge, edge_props.weight)?;
                                if edge_props.is_bidirectional {
                                    self.graph.add_edge(edge.reversed()?, edge_props.weight)?;
                                }
                            }
                        }
                    }
                }
                osmpbfreader::OsmObj::Relation(_) => {}
            }
        }
        Ok(())
    }
}

impl<T, WA> H3EdgeGraphBuilder<T> for OsmPbfH3EdgeGraphBuilder<T, WA>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Sync,
    WA: WayAnalyzer<T>,
{
    fn build_graph(self) -> std::result::Result<H3EdgeGraph<T>, Error> {
        Ok(self.graph)
    }
}
