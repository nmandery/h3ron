use std::cmp::max;
use std::path::Path;
use std::thread;

use argh::FromArgs;
use crossbeam_channel::{bounded, Receiver, Sender};
use gdal::raster::Dataset;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;

use h3_raster::input::{ClassifiedBand, NoData, Value};
use h3_raster::rasterconverter::{ConversionProgress, RasterConverter};
use h3_raster::tile::{generate_tiles, tile_size_from_rasterband};
use h3_raster::convertedraster::ConvertedRaster;

fn parse_h3_resolution(res_str: &str) -> Result<u8, String> {
    let resolution: u8 = match res_str.parse() {
        Ok(v) => v,
        Err(_) => return Err("h3 resolution must be a positive integer".to_string()),
    };
    if resolution > 15 {
        return Err("h3 resolution must be in range 0 - 15".to_string());
    }
    Ok(resolution)
}

type BandArguments = Vec<(u8, String)>;

fn parse_bands(arg: &str) -> Result<BandArguments, String> {
    let r = Regex::new(r"(([1-9][0-9]*)\s*(:\s*([0-9]+\.?[0-9]?))?)").unwrap();
    let mut results = BandArguments::new();

    for cap in r.captures_iter(arg) {
        let band = match &cap[2].parse::<u8>() {
            Ok(band) => *band,
            Err(e) => {
                log::error!("unable to parse band number: {} - {}", &cap[2], e);
                return Err("invalid band number".to_string());
            }
        };
        results.push((band, (&cap[4]).parse().unwrap()));
    }
    Ok(results)
}

#[test]
fn test_parse_bands() {
    let b1 = parse_bands("4:0").unwrap();
    assert_eq!(b1.len(), 1);
    assert_eq!(b1.get(0).unwrap().0, 4);
    assert_eq!(b1.get(0).unwrap().1, "0".to_string());

    let b1 = parse_bands("3:67, 2:1.5").unwrap();
    assert_eq!(b1.len(), 2);
    assert_eq!(b1.get(0).unwrap().0, 3);
    assert_eq!(b1.get(0).unwrap().1, "67".to_string());

    assert_eq!(b1.get(1).unwrap().0, 2);
    assert_eq!(b1.get(1).unwrap().1, "1.5".to_string());
}

#[derive(FromArgs, PartialEq, Debug)]
/// Convert raster dataset to compacted H3 hexagons.
pub struct TopLevelArguments {
    /// input raster dataset
    #[argh(option, short = 'i')]
    input_raster: String,

    /// h3 resolution
    #[argh(option, short = 'r', from_str_fn(parse_h3_resolution))]
    h3_resolution: u8,

    /// bands to extract. Must be in the form of "1:0" for band 1 with the no data
    /// value 0. Multiple bands are possible when the values are separated by a comma.
    #[argh(option, short = 'b', from_str_fn(parse_bands))]
    bands: BandArguments,

    #[argh(subcommand)]
    pub(crate) subcommand: Subcommands,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum Subcommands {
    ToOgr(ToOgrArguments),
    ToSqlite(ToSqliteArguments),
}

fn default_output_format() -> String {
    // just using shapefiles as the format is supported in most builds of gdal
    "ESRI Shapefile".to_string()
}

fn default_output_layer_name() -> String {
    "raster_to_h3".to_string()
}

fn default_output_table_name() -> String {
    "raster_to_h3".to_string()
}

#[derive(FromArgs, PartialEq, Debug)]
/// convert to an OGR dataset
#[argh(subcommand, name = "to-ogr")]
pub struct ToOgrArguments {
    #[argh(option, short = 'o')]
    /// output vector dataset
    output_dataset: String,

    #[argh(option, short = 'f', default = "default_output_format()")]
    /// the OGR output format
    output_format: String,

    #[argh(option, short = 'l', default = "default_output_layer_name()")]
    /// name of the output layer
    output_layer_name: String,

}

#[derive(FromArgs, PartialEq, Debug)]
/// convert to an OGR dataset
#[argh(subcommand, name = "to-sqlite")]
pub struct ToSqliteArguments {
    #[argh(option, short = 'o')]
    /// output sqlite database
    output_db: String,

    #[argh(option, short = 't', default = "default_output_table_name()")]
    /// name of the output database table
    output_table_name: String,

}


fn convert_raster(top_level_args: &TopLevelArguments) -> Result<ConvertedRaster, &'static str> {
    let dataset = match Dataset::open(Path::new(&top_level_args.input_raster)) {
        Ok(ds) => ds,
        Err(e) => {
            log::error!("unable to open gdal raster {}: {:?}", &top_level_args.input_raster, e);
            return Err("unable to open gdal raster");
        }
    };

    let mut inputs = vec![];
    let mut tile_size = (1000_usize, 1000_usize);
    for (band_num, no_data_string) in top_level_args.bands.iter() {
        let raster_band = match dataset.rasterband(*band_num as isize) {
            Ok(rb) => rb,
            Err(e) => {
                log::error!("can not access raster band {}: {}", band_num, e);
                return Err("can not access raster band");
            }
        };
        tile_size = tile_size_from_rasterband(&raster_band);
        macro_rules! no_data_classifier {
            ($value_type:path) => {{
                let value = $value_type(match no_data_string.parse() {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("can not parse value \"{}\" as {}: {}", no_data_string, stringify!($value_type), e);
                        return Err(concat!("can not access raster band as ", stringify!($value_type)));
                    }
                });
                Box::new(NoData { no_data_value: value })
            }}
        }
        let band_type = raster_band.band_type();
        let classifier = match band_type {
            gdal_sys::GDALDataType::GDT_Byte => no_data_classifier!(Value::Uint8),
            gdal_sys::GDALDataType::GDT_UInt16 => no_data_classifier!(Value::Uint16),
            gdal_sys::GDALDataType::GDT_UInt32 => no_data_classifier!(Value::Uint32),
            gdal_sys::GDALDataType::GDT_CInt16 => no_data_classifier!(Value::Int16),
            gdal_sys::GDALDataType::GDT_CInt32 => no_data_classifier!(Value::Int32),
            gdal_sys::GDALDataType::GDT_Int16 => no_data_classifier!(Value::Int16),
            gdal_sys::GDALDataType::GDT_Int32 => no_data_classifier!(Value::Int32),
            gdal_sys::GDALDataType::GDT_Float32 => no_data_classifier!(Value::Float32),
            gdal_sys::GDALDataType::GDT_Float64 => no_data_classifier!(Value::Float64),
            _ => {
                log::error!("unsupported gdal band type: {}", band_type);
                return Err("unsupported band type")
            }
        };
        inputs.push(ClassifiedBand {
            source_band: *band_num,
            classifier,
        })
    };
    if inputs.is_empty() {
        return Err("no bands given");
    }

    log::info!("dataset size is {} x {} pixels", dataset.size().0, dataset.size().1 );

    let tiles = generate_tiles(dataset.size(), tile_size);
    log::info!("using a tile size derived of {} x {} pixels derived from the block size -> {} tiles",
        tile_size.0, tile_size.1, tiles.len()
    );

    let num_converter_threads = max(1, (num_cpus::get() - 1) as u8);
    log::info!("starting conversion using {} tiling threads",
             num_converter_threads
    );

    let converter = RasterConverter::new(dataset, inputs, top_level_args.h3_resolution)?;

    let (progress_send, progress_recv): (Sender<ConversionProgress>, Receiver<ConversionProgress>) = bounded(2);

    let pb = ProgressBar::new(tiles.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:50.green/blue}] {pos}/{len} ({eta})")
        .progress_chars("#>-"));

    let child = thread::spawn(move || {
        for progress_update in progress_recv.iter() {
            pb.set_position(progress_update.tiles_done as u64);
        }
        pb.set_message("done");
        pb.abandon();
    });

    let converted = converter.convert_tiles(
        num_converter_threads,
        tiles,
        Some(progress_send),
    )?;

    child.join().unwrap();
    Ok(converted)
}

pub fn convert_to_ogr(top_level_args: &TopLevelArguments, to_ogr_args: &ToOgrArguments) -> Result<(), &'static str> {
    let converted_raster = convert_raster(top_level_args)?;

    log::info!("writing to OGR datasource");
    let ogr_driver = match gdal::vector::Driver::get(&to_ogr_args.output_format) {
        Ok(drv) => drv,
        Err(e) => {
            log::error!("ogr error opening driver {}: {}", &to_ogr_args.output_format, e);
            return Err("ogr error opening driver");
        }
    };
    let mut ogr_dataset = match ogr_driver.create(Path::new(&to_ogr_args.output_dataset)) {
        Ok(ds) => ds,
        Err(e) => {
            log::error!("ogr error creating {}: {}", &to_ogr_args.output_dataset, e);
            return Err("unable to create output dataset");
        }
    };

    let (progress_send, progress_recv): (Sender<usize>, Receiver<usize>) = bounded(2);

    let pb = ProgressBar::new(converted_raster.count_h3indexes() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:50.green/blue}] {pos}/{len} ({eta})")
        .progress_chars("#>-"));

    let child = thread::spawn(move || {
        for progress_update in progress_recv.iter() {
            pb.set_position(progress_update as u64);
        }
        pb.set_message("done");
        pb.abandon();
    });

    let write_result = converted_raster.write_to_ogr_dataset(
        &mut ogr_dataset,
        &to_ogr_args.output_layer_name,
        false, // TODO: expose as switch,
        Some(progress_send),
    );
    child.join().unwrap();

    match write_result {
        Ok(()) => Ok(()),
        Err(e) => {
            log::error!("ogr error: {}", e);
            Err("unable to write to ogr dataset")
        }
    }
}


pub fn convert_to_sqlite(top_level_args: &TopLevelArguments, to_sqlite_args: &ToSqliteArguments) -> Result<(), &'static str> {
    let converted_raster = convert_raster(top_level_args)?;

    let (progress_send, progress_recv): (Sender<usize>, Receiver<usize>) = bounded(2);

    let pb = ProgressBar::new(converted_raster.count_h3indexes() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:50.green/blue}] {pos}/{len} ({eta})")
        .progress_chars("#>-"));

    let child = thread::spawn(move || {
        for progress_update in progress_recv.iter() {
            pb.set_position(progress_update as u64);
        }
        pb.set_message("done");
        pb.abandon();
    });

    log::info!("writing to Sqlite database");
    let write_result: Result<(), &'static str> = match converted_raster.write_to_sqlite(Path::new(&to_sqlite_args.output_db), &to_sqlite_args.output_table_name, Some(progress_send)) {
        Ok(()) => Ok(()),
        Err(e) => {
            log::error!("error writing to sqlite db {}: {}", &to_sqlite_args.output_db, e);
            return Err("error writing to sqlite db");
        }
    };
    child.join().unwrap();

    match write_result {
        Ok(()) => Ok(()),
        Err(e) => {
            log::error!("sqlite error: {}", e);
            Err("unable to write to sqlite database")
        }
    }
}

