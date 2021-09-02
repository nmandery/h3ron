//! Support for OpenStreetMap data formats

use std::ops::Add;
use std::path::Path;

use geo_types::{Coordinate, LineString};
pub use osmpbfreader;
use osmpbfreader::{OsmPbfReader, Tags};

use h3ron::collections::HashMap;

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

/// Builds [`H3EdgeGraph`] instances from .osm.pbf files.
pub struct OsmPbfH3EdgeGraphBuilder<
    T: PartialOrd + PartialEq + Add + Copy + Sync + Send,
    F: Fn(&Tags) -> Option<EdgeProperties<T>>,
> {
    h3_resolution: u8,
    edge_properties_fn: F,
    graph: H3EdgeGraph<T>,
}

impl<T, F> OsmPbfH3EdgeGraphBuilder<T, F>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Sync,
    F: Fn(&Tags) -> Option<EdgeProperties<T>>,
{
    pub fn new(h3_resolution: u8, edge_properties_fn: F) -> Self {
        Self {
            h3_resolution,
            edge_properties_fn,
            graph: H3EdgeGraph::new(h3_resolution),
        }
    }

    pub fn read_pbf(&mut self, pbf_path: &Path) -> Result<(), Error> {
        let pbf_file = std::fs::File::open(pbf_path)?;
        let mut pbf = OsmPbfReader::new(pbf_file);
        let mut nodeid_coordinates: HashMap<_, _> = Default::default();
        for obj_result in pbf.par_iter() {
            let obj = obj_result?;
            match obj {
                osmpbfreader::OsmObj::Node(node) => {
                    let coordinate = Coordinate {
                        x: node.lon(),
                        y: node.lat(),
                    };
                    nodeid_coordinates.insert(node.id, coordinate);
                }
                osmpbfreader::OsmObj::Way(way) => {
                    if let Some(edge_props) = (self.edge_properties_fn)(&way.tags) {
                        let coordinates: Vec<_> = way
                            .nodes
                            .iter()
                            .filter_map(|node_id| nodeid_coordinates.get(node_id).cloned())
                            .collect();
                        if coordinates.len() >= 2 {
                            let mut h3indexes =
                                h3ron::line(&LineString::from(coordinates), self.h3_resolution)?;
                            h3indexes.dedup();

                            for window in h3indexes.windows(2) {
                                if edge_props.is_bidirectional {
                                    self.graph.add_edge_using_cells_bidirectional(
                                        window[0],
                                        window[1],
                                        edge_props.weight,
                                    )?;
                                } else {
                                    self.graph.add_edge_using_cells(
                                        window[0],
                                        window[1],
                                        edge_props.weight,
                                    )?;
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

impl<T, F> H3EdgeGraphBuilder<T> for OsmPbfH3EdgeGraphBuilder<T, F>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Sync,
    F: Fn(&Tags) -> Option<EdgeProperties<T>>,
{
    fn build_graph(self) -> std::result::Result<H3EdgeGraph<T>, Error> {
        Ok(self.graph)
    }
}
