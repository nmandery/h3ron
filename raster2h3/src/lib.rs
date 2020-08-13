use std::cmp::max;
use std::path::Path;

use argh::FromArgs;
use crossbeam::channel::{bounded, Receiver, Sender};
use gdal::raster::Dataset;
use regex::Regex;

use h3_raster::convertedraster::ConvertedRaster;
use h3_raster::input::{ClassifiedBand, NoData, Value};
use h3_raster::rasterconverter::{ConversionProgress, RasterConverter};
use h3_raster::tile::{generate_tiles, tile_size_from_rasterband};
use h3_util::progress::{Progress, ApplyProgress};

fn parse_u32(arg: &str, name: &str) -> Result<u32, String> {
    match arg.parse::<u32>() {
        Ok(v) => Ok(v),
        Err(_) => Err(format!("{} must be a positive integer", name)),
    }
}

fn parse_h3_resolution(res_str: &str) -> Result<u8, String> {
    let resolution = parse_u32(res_str, "h3 resolution")?;
    if resolution > 15 {
        return Err("h3 resolution must be in range 0 - 15".to_string());
    }
    Ok(resolution as u8)
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

fn parse_n_tile_threads(arg: &str) -> Result<u32, String> {
    let n_tile_threads = parse_u32(arg, "number of tiling threads")?;
    if n_tile_threads < 1 {
        return Err("number of tiling threads must be >= 1".to_string());
    }
    Ok(n_tile_threads)
}

fn default_n_tile_threads() -> u32 {
    max(1, num_cpus::get() - 1) as u32
}

fn parse_tile_size(arg: &str) -> Result<Option<u32>, String> {
    if arg.is_empty() {
        Ok(None)
    } else {
        let tile_size = parse_u32(arg, "tile size")?;
        Ok(Some(tile_size))
    }
}

fn default_tile_size() -> Option<u32> { None }

#[derive(FromArgs, PartialEq, Debug)]
/// Convert raster dataset to compacted H3 hexagons.
pub struct TopLevelArguments {
    /// input raster dataset
    #[argh(option, short = 'i')]
    input_raster: String,

    /// h3 resolution
    #[argh(option, short = 'r', from_str_fn(parse_h3_resolution))]
    h3_resolution: u8,

    /// number of threads to use to analyze tiles
    #[argh(option, short = 'n', from_str_fn(parse_n_tile_threads), default = "default_n_tile_threads()")]
    n_tile_threads: u32,

    /// tile size in pixels
    #[argh(option, short = 't', from_str_fn(parse_tile_size), default = "default_tile_size()")]
    tile_size: Option<u32>,

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
    let mut band_based_tile_size = (1000_usize, 1000_usize);
    for (band_num, no_data_string) in top_level_args.bands.iter() {
        let raster_band = match dataset.rasterband(*band_num as isize) {
            Ok(rb) => rb,
            Err(e) => {
                log::error!("can not access raster band {}: {}", band_num, e);
                return Err("can not access raster band");
            }
        };
        band_based_tile_size = tile_size_from_rasterband(&raster_band);
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
                return Err("unsupported band type");
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

    let tiles = if let Some(tile_size) = top_level_args.tile_size {
        generate_tiles(dataset.size(), (tile_size as usize, tile_size as usize))
    } else {
        let tiles = generate_tiles(dataset.size(), band_based_tile_size);
        log::info!("using a tile size derived of {} x {} pixels derived from the block size -> {} tiles",
        band_based_tile_size.0, band_based_tile_size.1, tiles.len());
        tiles
    };

    log::info!("starting conversion using {} tiling threads",
             top_level_args.n_tile_threads
    );

    let converter = RasterConverter::new(dataset, inputs, top_level_args.h3_resolution)
        .map_err(|e| {
            log::error!("{:?}", e);
            "creating rasterconverter failed"
        })?;

    let (progress_send, progress_recv): (Sender<ConversionProgress>, Receiver<ConversionProgress>) = bounded(2);

    let mut progress = Progress::new(tiles.len() as u64, progress_recv, "tiles finished");
    let converted = progress.apply(|| {
        converter.convert_tiles(
            top_level_args.n_tile_threads,
            tiles,
            Some(progress_send),
            true,
        )
    }).map_err(|e| {
        log::error!("{:?}", e);
        "converting tiles failed"
    })?;

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
    let mut progress = Progress::new(converted_raster.count_h3indexes() as u64, progress_recv, "features written");

    let write_result = progress.apply(|| {
        converted_raster.write_to_ogr_dataset(
            &mut ogr_dataset,
            &to_ogr_args.output_layer_name,
            false, // TODO: expose as switch,
            Some(progress_send),
        )
    });

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

    let mut progress = Progress::new(converted_raster.count_h3indexes() as u64, progress_recv, "h3indexes written");
    progress.apply(|| {
        let write_result: Result<(), &'static str> = match converted_raster.write_to_sqlite(Path::new(&to_sqlite_args.output_db), &to_sqlite_args.output_table_name, false, Some(progress_send)) {
            Ok(()) => Ok(()),
            Err(e) => {
                log::error!("error writing to sqlite db {}: {}", &to_sqlite_args.output_db, e);
                Err("error writing to sqlite db")
            }
        };
        write_result
    })
}

