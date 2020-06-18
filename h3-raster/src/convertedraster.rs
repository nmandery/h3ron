use std::collections::HashMap;
#[cfg(feature = "sqlite")]
use std::path::Path;

use byteorder::ByteOrder;
use crossbeam::channel::Sender;
use gdal::spatial_ref::SpatialRef;
use gdal::vector::{Defn, Feature, FieldDefn, ToGdal};
#[cfg(feature = "sqlite")]
use rusqlite::{Connection, ToSql};
#[cfg(feature = "sqlite")]
use rusqlite::{NO_PARAMS, OptionalExtension};
#[cfg(feature = "sqlite")]
use rusqlite::types::ToSqlOutput;

use h3::{get_resolution, h3_to_string};
use h3::stack::IndexStack;

use crate::geo::polygon_has_dateline_wrap;
use crate::input::Value;

pub type Attributes = Vec<Option<Value>>;
pub type GroupedH3Indexes = HashMap<Attributes, IndexStack>;

pub struct ConvertedRaster {
    pub value_types: Vec<Value>,
    pub indexes: GroupedH3Indexes,
}

#[cfg(feature = "sqlite")]
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
    #[cfg(feature = "sqlite")]
    pub fn write_to_sqlite(&self, db_file: &Path, table_name: &str, h3index_as_blob: bool, send_progress: Option<Sender<usize>>) -> rusqlite::Result<()> {
        let mut conn = Connection::open(db_file)?;
        self.write_to_sqlite_conn(&mut conn, table_name, h3index_as_blob, send_progress)
    }

    #[cfg(feature = "sqlite")]
    ///
    /// `h3index_as_blob` allows storing the h3indexes as big endian encoded binary bolb. This
    /// representation has a smaller size. It also can be converted to the text representation directly in
    /// sqlite:
    /// > select lower(trim(quote(h3index), 'X''')) from raster_to_h3 limit 4;
    /// lower(trim(quote(h3index), 'X'''))
    /// ----------------------------------
    /// 081807ffffffffff
    /// 08111bffffffffff
    /// 081b37ffffffffff
    /// 0810fbffffffffff
    /// Run Time: real 0.001 user 0.000132 sys 0.000132
    ///
    pub fn write_to_sqlite_conn(&self, conn: &mut Connection, table_name: &str, h3index_as_blob: bool, send_progress: Option<Sender<usize>>) -> rusqlite::Result<()> {
        let do_send_progress = |counter| {
            if let Some(sp) = &send_progress {
                sp.send(counter).unwrap();
            }
        };

        // create the tables
        conn.execute("create table if not exists h3_datasets (name TEXT UNIQUE, created DATETIME DEFAULT CURRENT_TIMESTAMP)", NO_PARAMS)?;

        let attribute_table_name = format!("{}_attributes", table_name);
        let mut columns = vec![
            ("attribute_set_id".to_string(), "INTEGER".to_string())
        ];
        for (i, value_type) in self.value_types.iter().enumerate() {
            let field_type = match value_type {
                Value::Int16(_) | Value::Uint8(_) | Value::Uint16(_) | Value::Uint32(_) | Value::Int32(_) => "INTEGER".to_string(),
                Value::Float32(_) | Value::Float64(_) => "REAL".to_string(),
            };
            columns.push((format!("value_{}", i), field_type));
        }
        conn.execute(
            &format!("create table if not exists {} ({})",
                     attribute_table_name,
                     columns.iter().map(|(c, t)| format!("{} {}", c, t)).collect::<Vec<String>>().join(", ")
            ),
            NO_PARAMS,
        )?;

        if h3index_as_blob {
            conn.execute(
                // using text representation for the indexes as sqlite has not uint64
                &format!("create table if not exists {} (h3index BLOB, h3res INTEGER, attribute_set_id INTEGER)",
                         table_name
                ),
                NO_PARAMS,
            )?;
        } else {
            conn.execute(
                // using text representation for the indexes as sqlite has not uint64
                &format!("create table if not exists {} (h3index TEXT, h3res INTEGER, attribute_set_id INTEGER)",
                         table_name
                ),
                NO_PARAMS,
            )?;
        }

        let tx = conn.transaction()?;
        {
            // register the dataset
            tx.execute(
                "insert or ignore into h3_datasets (name) values (?1);",
                params![&table_name],
            )?;

            let mut insert_attributes_stmt = tx.prepare(&format!(
                "insert into {} ({}) values ({});",
                attribute_table_name,
                columns.iter().map(|(c, _t)| c.to_string()).collect::<Vec<String>>().join(", "),
                (0..columns.len()).map(|v| format!("?{}", v + 1)).collect::<Vec<String>>().join(", ")
            ))?;

            let mut insert_index_stmt = tx.prepare(&format!(
                "insert into {} (h3index, h3res, attribute_set_id) values (?1, ?2, ?3);",
                table_name
            ))?;

            let mut attribute_set_id = match tx.query_row(
                &format!("select coalesce(max(attribute_set_id), 0) from {}", attribute_table_name),
                NO_PARAMS,
                |row| row.get::<usize, u32>(0)).optional()? {
                None => 1,
                Some(n) => n + 1_u32,
            };
            let mut num_written_features: usize = 0;
            do_send_progress(num_written_features);
            for (attr, compacted_stack) in self.indexes.iter() {
                insert_attributes_stmt.execute({
                    let mut sql_params: Vec<&dyn ToSql> = vec![
                        &attribute_set_id
                    ];
                    for val_opt in attr.iter() {
                        sql_params.push(val_opt);
                    }
                    sql_params
                })?;

                for h3index in compacted_stack.indexes_by_resolution.values().flatten() {
                    let resolution = get_resolution(*h3index);
                    if h3index_as_blob {
                        let mut buf = [0; 8];
                        byteorder::BigEndian::write_u64(&mut buf, *h3index);
                        let h3index_bytes = buf.to_vec();
                        let sql_params: Vec<&dyn ToSql> = vec![
                            &h3index_bytes, &resolution, &attribute_set_id
                        ];
                        insert_index_stmt.execute(sql_params)?;
                    } else {
                        let index_str = h3_to_string(*h3index);
                        let sql_params: Vec<&dyn ToSql> = vec![
                            &index_str, &resolution, &attribute_set_id
                        ];
                        insert_index_stmt.execute(sql_params)?;
                    }

                    num_written_features += 1;
                    if (num_written_features % 5000) == 0 {
                        do_send_progress(num_written_features);
                    }
                }

                attribute_set_id += 1;
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
