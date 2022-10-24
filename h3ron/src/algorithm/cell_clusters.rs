use crate::collections::HashMap;
use crate::iter::GridDiskBuilder;
use crate::{Error, H3Cell};
use ahash::RandomState;
use hashbrown::hash_map::Entry;
use indexmap::IndexMap;
use std::cmp::Ordering;

/// find clusters of neighboring cells
///
/// Requires the `indexmap` feature.
pub fn find_cell_clusters<CellsIter>(cells: CellsIter) -> Result<Vec<Vec<H3Cell>>, Error>
where
    CellsIter: Iterator<Item = H3Cell>,
{
    Ok(find_cell_clusters_eq_value_impl(cells)?
        .into_values()
        .map(|(cluster, _)| cluster)
        .collect())
}

/// find clusters of neighboring cells where the same value is associated with the cells.
///
/// Cells are assumed to be unique, otherwise the behaviour is undefined.
///
/// Requires the `indexmap` feature.
pub fn find_cell_clusters_eq_value<CellValueIter, CV, Value>(
    cell_value_iter: CellValueIter,
) -> Result<Vec<(Vec<H3Cell>, Value)>, Error>
where
    CV: CellAndValue<Value>,
    CellValueIter: Iterator<Item = CV>,
    Value: PartialEq,
{
    Ok(find_cell_clusters_eq_value_impl(cell_value_iter)?
        .into_values()
        .collect())
}

pub trait CellAndValue<Value> {
    fn cell(&self) -> H3Cell;
    fn value(self) -> Value;
}

impl CellAndValue<()> for H3Cell {
    fn cell(&self) -> H3Cell {
        *self
    }

    fn value(self) {}
}

impl<Value> CellAndValue<Value> for (H3Cell, Value) {
    fn cell(&self) -> H3Cell {
        self.0
    }

    fn value(self) -> Value {
        self.1
    }
}

fn find_cell_clusters_eq_value_impl<CellValueIter, CV, Value>(
    cell_value_iter: CellValueIter,
) -> Result<HashMap<usize, (Vec<H3Cell>, Value)>, Error>
where
    CV: CellAndValue<Value>,
    CellValueIter: Iterator<Item = CV>,
    Value: PartialEq,
{
    let items: IndexMap<_, _, RandomState> =
        cell_value_iter.map(|cv| (cv.cell(), cv.value())).collect();
    let mut cluster_ids: Vec<usize> = (0..items.len()).collect();

    let mut mutated = true;
    let mut disk_builder = GridDiskBuilder::create(1, 1)?;
    while mutated {
        mutated = false;
        for (pos, (cell, value)) in items.iter().enumerate() {
            let mut least_cluster_id = cluster_ids[pos];
            for (neighbor_cell, _) in &mut disk_builder.build_grid_disk(cell)? {
                if let Some((neighbor_pos, _, neighbor_value)) = items.get_full(&neighbor_cell) {
                    if neighbor_value == value {
                        match cluster_ids[neighbor_pos].cmp(&least_cluster_id) {
                            Ordering::Less => {
                                least_cluster_id = cluster_ids[neighbor_pos];
                                cluster_ids[pos] = least_cluster_id;
                                mutated = true;
                            }
                            Ordering::Equal => {}
                            Ordering::Greater => {
                                cluster_ids[neighbor_pos] = least_cluster_id;
                                mutated = true;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(cluster_ids.into_iter().zip(items.into_iter()).fold(
        HashMap::default(),
        |mut acc, (group, (cell, value))| {
            match acc.entry(group) {
                Entry::Vacant(entry) => {
                    entry.insert((vec![cell], value));
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().0.push(cell);
                }
            };
            acc
        },
    ))
}

#[cfg(test)]
mod tests {
    use crate::algorithm::find_cell_clusters;
    use crate::H3Cell;

    #[test]
    fn find_cell_clusters_simple() {
        let mut disk1: Vec<_> = H3Cell::from_coordinate((12.2, 14.5).into(), 6)
            .unwrap()
            .grid_disk(3)
            .unwrap()
            .iter()
            .collect();
        disk1.sort_unstable();
        let mut disk2: Vec<_> = H3Cell::from_coordinate((42.2, 45.5).into(), 6)
            .unwrap()
            .grid_disk(2)
            .unwrap()
            .iter()
            .collect();
        disk2.sort_unstable();

        let mut clusters =
            find_cell_clusters(disk1.iter().copied().chain(disk2.iter().copied())).unwrap();
        assert_eq!(clusters.len(), 2);
        let mut cluster1 = clusters.remove(0);
        cluster1.sort_unstable();
        let mut cluster2 = clusters.remove(0);
        cluster2.sort_unstable();
        assert!(cluster1 == disk1 || cluster1 == disk2);
        assert!(cluster2 == disk1 || cluster2 == disk2);
        assert_ne!(cluster1, cluster2);
    }
}
