use crate::{Error, FromIndexIterator, IndexChunked};
use h3ron::algorithm::{find_cell_clusters, find_cell_clusters_eq_value};
use h3ron::H3Cell;
use polars_core::frame::DataFrame;
use polars_core::prelude::{NamedFrom, Series};

pub trait H3CellClusters {
    /// find clusters of neighboring cells
    ///
    /// Returns a new dataframe with two columns:
    /// * `cluster_num`: artificial id (u32) for the cluster.
    /// * `cells`: list of cells in the cluster
    ///
    fn h3_cell_clusters(&self) -> Result<DataFrame, Error>;

    /// find clusters of neighboring cells where the same value is associated with the cells.
    ///
    /// The `self` and `values` should have the same length. Any excess in either of them
    /// will be ignored.
    ///
    /// Cells are assumed to be unique, otherwise the behaviour is undefined.
    ///
    /// Returns a new dataframe with three columns:
    /// * `cluster_num`: artificial id (u32) for the cluster.
    /// * `cells`: list of cells in the cluster
    /// * The value of the series given as the `values` parameter using the name of that series.
    ///
    fn h3_cell_clusters_eq_value(&self, values: &Series) -> Result<DataFrame, Error>;
}

impl<'a> H3CellClusters for IndexChunked<'a, H3Cell> {
    fn h3_cell_clusters(&self) -> Result<DataFrame, Error> {
        let clusters = find_cell_clusters(self.iter_indexes_nonvalidated().flatten())?;
        let capacity = clusters.len();

        let (cluster_num, cells) = clusters.into_iter().enumerate().fold(
            (Vec::with_capacity(capacity), Vec::with_capacity(capacity)),
            |mut acc, (cluster_num, cells)| {
                acc.0.push(cluster_num as u32);
                acc.1.push(Series::from_index_iter(cells));
                acc
            },
        );
        DataFrame::new(vec![
            Series::new("cluster_num", cluster_num),
            Series::new("cells", cells),
        ])
        .map_err(Error::from)
    }

    fn h3_cell_clusters_eq_value(&self, values: &Series) -> Result<DataFrame, Error> {
        self.chunked_array.rechunk(); // avoid panic in values.iter
        let clusters = find_cell_clusters_eq_value(
            self.iter_indexes_nonvalidated()
                .zip(values.iter())
                .flat_map(|(cell, value)| cell.map(|cell| (cell, value))),
        )?;

        let capacity = clusters.len();

        let (cluster_num, cluster_value, cells) = clusters.into_iter().enumerate().fold(
            (
                Vec::with_capacity(capacity),
                Vec::with_capacity(capacity),
                Vec::with_capacity(capacity),
            ),
            |mut acc, (cluster_num, (cells, value))| {
                acc.0.push(cluster_num as u32);
                acc.1.push(value);
                acc.2.push(Series::from_index_iter(cells));
                acc
            },
        );
        DataFrame::new(vec![
            Series::new("cluster_num", cluster_num),
            Series::from_any_values(values.name(), &cluster_value, true)?,
            Series::new("cells", cells),
        ])
        .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithm::H3CellClusters;
    use crate::{AsH3CellChunked, FromIndexIterator};
    use h3ron::H3Cell;
    use polars_core::prelude::{NamedFrom, Series, UInt64Chunked};
    use std::iter::repeat;

    #[test]
    fn find_cell_clusters_simple() {
        let mut cells: Vec<_> = H3Cell::from_coordinate((12.2, 14.5).into(), 6)
            .unwrap()
            .grid_disk(3)
            .unwrap()
            .iter()
            .collect();
        let mut values: Vec<_> = repeat(1u32).take(cells.len()).collect();

        cells.extend(
            H3Cell::from_coordinate((42.2, 45.5).into(), 6)
                .unwrap()
                .grid_disk(2)
                .unwrap()
                .iter(),
        );
        values.extend(repeat(5u32).take(cells.len() - values.len()));

        let cells = UInt64Chunked::from_index_iter::<_, H3Cell>(cells.iter());
        let values = Series::new("value", values);
        assert_eq!(cells.len(), values.len());

        let clusters = cells.h3cell().h3_cell_clusters_eq_value(&values).unwrap();
        assert_eq!(clusters.shape().0, 2);
        //dbg!(clusters);
    }
}
