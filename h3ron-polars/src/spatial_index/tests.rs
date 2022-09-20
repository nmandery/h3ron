#[allow(unused_macros)]
macro_rules! impl_std_tests {
    ($mk_index:expr) => {
        use crate::from::NamedFromIndexes;
        use crate::spatial_index::{SpatialIndex, SpatialIndexGeomOp};
        use crate::AsH3CellChunked;
        use geo_types::{coord, polygon, Rect};
        use h3ron::{Index};
        use polars::prelude::{TakeRandom, UInt64Chunked, NamedFrom};

        fn build_cell_ca() -> UInt64Chunked {
            UInt64Chunked::new_from_indexes(
                "",
                vec![
                    H3Cell::from_coordinate((45.5, 45.5).into(), 7).unwrap(),
                    H3Cell::from_coordinate((-60.5, -60.5).into(), 7).unwrap(),
                    H3Cell::from_coordinate((120.5, 70.5).into(), 7).unwrap(),
                    H3Cell::new(55), // invalid
                ],
            )
        }

        #[test]
        fn cell_create_empty_index() {
            let values: Vec<u64> = vec![];
            let ca = UInt64Chunked::new("", values);
            let _ = $mk_index(&ca.h3cell());
        }

        #[test]
        fn cell_envelopes_within_distance() {
            let ca = build_cell_ca();
            let idx = $mk_index(&ca.h3cell());
            let mask = idx.envelopes_within_distance((-60.0, -60.0).into(), 2.0);

            assert_eq!(mask.len(), 4);
            assert_eq!(mask.get(0), Some(false));
            assert_eq!(mask.get(1), Some(true));
            assert_eq!(mask.get(2), Some(false));
            assert_eq!(mask.get(3), None);
        }

        #[test]
        fn cell_geometries_intersect() {
            let ca = build_cell_ca();
            let idx = $mk_index(&ca.h3cell());
            let mask = idx.geometries_intersect(&Rect::new((40.0, 40.0), (50.0, 50.0)));

            assert_eq!(mask.len(), 4);
            assert_eq!(mask.get(0), Some(true));
            assert_eq!(mask.get(1), Some(false));
            assert_eq!(mask.get(2), Some(false));
            assert_eq!(mask.get(3), None);
        }

        #[test]
        fn cell_geometries_intersect_polygon() {
            let ca = build_cell_ca();
            let idx = $mk_index(&ca.h3cell());
            let mask = idx.geometries_intersect_polygon(&polygon!(exterior: [
                    coord! {x: 40.0, y: 40.0},
                    coord! {x: 40.0, y: 50.0},
                    coord! {x: 49.0, y: 50.0},
                    coord! {x: 49.0, y: 40.0},
                    coord! {x: 40.0, y: 40.0},
                ], interiors: []));

            assert_eq!(mask.len(), 4);
            assert_eq!(mask.get(0), Some(true));
            assert_eq!(mask.get(1), Some(false));
            assert_eq!(mask.get(2), Some(false));
            assert_eq!(mask.get(3), None);
        }
    }
}

// make the macro available to other modules
#[allow(unused_imports)]
pub(crate) use impl_std_tests;
