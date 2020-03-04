use std::collections::HashMap;
use std::path::Path;

use crossbeam_channel::Sender;
use gdal::spatial_ref::SpatialRef;
use gdal::vector::{Defn, Feature, FieldDefn, ToGdal};
use rusqlite::{Connection, ToSql};
use rusqlite::types::ToSqlOutput;

use h3::{get_resolution, h3_to_string};
use h3::compact::CompactedIndexStack;

use crate::geo::polygon_has_dateline_wrap;
use crate::input::Value;

pub type Attributes = Vec<Option<Value>>;
pub type GroupedH3Indexes = HashMap<Attributes, CompactedIndexStack>;

pub struct ConvertedRaster {
    pub value_types: Vec<Value>,
    pub indexes: GroupedH3Indexes,
}

impl ToSql for Value {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match self {
            Value::Int16(v) => v.to_sql(),
            Value::Uint8(v) => v.to_sql(),
            Value::Uint16(v) => v.to_sql(),
            Value::Int32(v) => v.to_sql(),
            Value::Uint32(v) => v.to_sql(),
            Value::Float32(v) => Ok(ToSqlOutput::Owned((v.0 as f64).into())),
            Value::Float64(v) => v.0.to_sql(),
        }
    }
}

impl ConvertedRaster {
    pub fn write_to_sqlite(&self, db_file: &Path, table_name: &str, send_progress: Option<Sender<usize>>) -> rusqlite::Result<()> {
        let do_send_progress = |counter| {
            if let Some(sp) = &send_progress {
                sp.send(counter).unwrap();
            }
        };

        let mut conn = Connection::open(db_file)?;

        // create the table
        let mut columns = vec![
            ("h3index".to_string(), "TEXT".to_string()), // sqlite has no uint64
            ("h3res".to_string(), "INTEGER".to_string())
        ];
        for (i, value_type) in self.value_types.iter().enumerate() {
            let field_type = match value_type {
                Value::Int16(_) | Value::Uint8(_) | Value::Uint16(_) | Value::Uint32(_) | Value::Int32(_) => "INTEGER".to_string(),
                Value::Float32(_) | Value::Float64(_) => "REAL".to_string(),
            };
            columns.push((format!("value_{}", i), field_type));
        }
        let table_ddl = format!("create table if not exists {} ({})",
                                table_name,
                                columns.iter().map(|(c, t)| format!("{} {}", c, t)).collect::<Vec<String>>().join(", ")
        );
        conn.execute(&table_ddl, params![])?;

        let tx = conn.transaction()?;
        {
            let mut insert_stmt = tx.prepare(&format!(
                "insert into {} ({}) values ({});",
                table_name,
                columns.iter().map(|(c, _t)| c.to_string()).collect::<Vec<String>>().join(", "),
                (0..columns.len()).map(|v| format!("?{}", v + 1)).collect::<Vec<String>>().join(", ")
            ))?;

            let mut num_written_features: usize = 0;
            do_send_progress(num_written_features);
            for (attr, compacted_stack) in self.indexes.iter() {
                for h3index in compacted_stack.indexes_by_resolution.values().flatten() {
                    let index_str = h3_to_string(*h3index);
                    let resolution = get_resolution(*h3index);
                    let mut sql_params: Vec<&dyn ToSql> = vec![
                        &index_str, &resolution
                    ];
                    for val_opt in attr.iter() {
                        sql_params.push(val_opt);
                    }
                    insert_stmt.execute(sql_params)?;

                    num_written_features += 1;
                    if (num_written_features % 5000) == 0 {
                        do_send_progress(num_written_features);
                    }
                }
            }
            do_send_progress(num_written_features);
        }
        tx.commit()?;
        Ok(())
    }

    /// creates a new layer in an OGR dataset
    pub fn write_to_ogr_dataset(&self, dataset: &mut gdal::vector::Dataset, layer_name: &str, include_dateline_wrap: bool, send_progress: Option<Sender<usize>>) -> gdal::errors::Result<()> {
        let do_send_progress = |counter| {
            if let Some(sp) = &send_progress {
                sp.send(counter).unwrap();
            }
        };

        let srs = SpatialRef::from_epsg(4326)?;
        let layer = dataset.create_layer_ext(
            layer_name,
            Some(&srs),
            gdal_sys::OGRwkbGeometryType::wkbPolygon,
        )?;

        let index_field_name = "h3index";
        FieldDefn::new(index_field_name, gdal_sys::OGRFieldType::OFTString)?.add_to_layer(layer)?;

        let res_field_name = "h3res";
        FieldDefn::new(res_field_name, gdal_sys::OGRFieldType::OFTInteger)?.add_to_layer(layer)?;

        for (i, value_type) in self.value_types.iter().enumerate() {
            let ogr_field_type = match value_type {
                Value::Int16(_) => gdal_sys::OGRFieldType::OFTInteger,
                Value::Uint8(_) => gdal_sys::OGRFieldType::OFTInteger,
                Value::Uint16(_) => gdal_sys::OGRFieldType::OFTInteger,
                Value::Int32(_) => gdal_sys::OGRFieldType::OFTInteger,
                Value::Uint32(_) => gdal_sys::OGRFieldType::OFTInteger,
                Value::Float32(_) => gdal_sys::OGRFieldType::OFTReal,
                Value::Float64(_) => gdal_sys::OGRFieldType::OFTReal,
            };
            let field_defn = FieldDefn::new(format!("value_{}", i).as_ref(), ogr_field_type)?;
            field_defn.add_to_layer(layer)?;
        }

        let defn = Defn::from_layer(layer);

        let mut num_written_features: usize = 0;
        do_send_progress(num_written_features);
        for (attr, compacted_stack) in self.indexes.iter() {
            for h3index in compacted_stack.indexes_by_resolution.values().flatten() {
                if let Some(poly) = h3::polygon_from_h3index(*h3index) {

                    // ignore indexes spanning the whole extend as they are
                    // located on the "backside" of the world
                    if !include_dateline_wrap && polygon_has_dateline_wrap(&poly) {
                        continue;
                    }

                    // build the feature
                    let mut feature = Feature::new(&defn)?;
                    feature.set_geometry(poly.to_gdal().unwrap()).unwrap();
                    feature.set_field_string(index_field_name, &h3_to_string(*h3index))?;
                    feature.set_field_integer(res_field_name, get_resolution(*h3index))?;

                    for (i, val_opt) in attr.iter().enumerate() {
                        if let Some(val) = val_opt {
                            let field_name = format!("value_{}", i);
                            match val {
                                // OGR Integer fields are i32
                                Value::Int16(v) => feature.set_field_integer(&field_name, *v as i32)?,
                                Value::Uint8(v) => feature.set_field_integer(&field_name, *v as i32)?,
                                Value::Uint16(v) => feature.set_field_integer(&field_name, *v as i32)?,
                                Value::Int32(v) => feature.set_field_integer(&field_name, *v as i32)?,
                                Value::Uint32(v) => feature.set_field_integer(&field_name, *v as i32)?,

                                // OGR Double fields are f64
                                Value::Float32(v) => feature.set_field_double(&field_name, v.0 as f64)?,
                                Value::Float64(v) => feature.set_field_double(&field_name, v.0 as f64)?,
                            };
                        }
                    }
                    feature.create(layer)?;
                }

                num_written_features += 1;
                if (num_written_features % 5000) == 0 {
                    do_send_progress(num_written_features);
                }
            }
        }
        do_send_progress(num_written_features);
        Ok(())
    }

    pub fn count_h3indexes(&self) -> usize {
        self.indexes.iter()
            .map(|(_, compacted_index_stack)| compacted_index_stack.len())
            .sum()
    }
}
