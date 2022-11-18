use gdal::{
    vector::{Defn, Feature, FieldDefn, OGRFieldType, ToGdal},
    Dataset, DriverManager,
};

use h3ron::{Index, ToPolygon};
use h3ron_ndarray::{AxisOrder, H3Converter, ResolutionSearchMode::SmallerThanPixel, Transform};

fn main() {
    env_logger::init(); // run with the environment variable RUST_LOG set to "debug" for log output

    let filename = format!("{}/../data/r.tiff", env!("CARGO_MANIFEST_DIR"));
    let dataset = Dataset::open(&filename).unwrap();
    let transform = Transform::from_gdal(&dataset.geo_transform().unwrap());
    let band = dataset.rasterband(1).unwrap();
    let band_array = band
        .read_as_array::<u8>((0, 0), band.size(), band.size(), None)
        .unwrap();

    let view = band_array.view();
    let conv = H3Converter::new(&view, &Some(0_u8), &transform, AxisOrder::YX);

    let h3_resolution = conv.nearest_h3_resolution(SmallerThanPixel).unwrap();
    println!("selected H3 resolution: {}", h3_resolution);

    let results = conv.to_h3(h3_resolution, true).unwrap();
    results.iter().for_each(|(value, index_stack)| {
        println!("{} -> {}", value, index_stack.len());
    });

    // write to vector file
    let out_drv = DriverManager::get_driver_by_name("GPKG").unwrap();
    let mut out_dataset = out_drv
        .create_vector_only("h3ify_r_tiff_results.gpkg")
        .unwrap();
    let out_lyr = out_dataset.create_layer(Default::default()).unwrap();

    let h3index_field_defn = FieldDefn::new("h3index", OGRFieldType::OFTString).unwrap();
    h3index_field_defn.set_width(20);
    h3index_field_defn.add_to_layer(&out_lyr).unwrap();

    let h3res_field_defn = FieldDefn::new("h3res", OGRFieldType::OFTInteger).unwrap();
    h3res_field_defn.add_to_layer(&out_lyr).unwrap();

    let defn = Defn::from_layer(&out_lyr);

    results.iter().for_each(|(_value, index_stack)| {
        for cell in index_stack.iter_compacted_cells() {
            let mut ft = Feature::new(&defn).unwrap();
            ft.set_geometry(cell.to_polygon().unwrap().to_gdal().unwrap())
                .unwrap();
            ft.set_field_string("h3index", &cell.to_string()).unwrap();
            ft.set_field_integer("h3res", cell.resolution() as i32)
                .unwrap();
            ft.create(&out_lyr).unwrap();
        }
    });
}
