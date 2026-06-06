use crate::data::engine::DataEngine;
use anyhow::{Context, Result};

/// Export the rows produced by an arbitrary query (an AI/SQL/formula/group-by
/// result) by wrapping it in COPY, so Export writes what's on screen rather
/// than the base `data` table. `format_idx`: 0=CSV, 1=JSON, 2=Parquet.
pub fn export_result(engine: &DataEngine, path: &str, format_idx: usize, inner_sql: &str) -> Result<()> {
    let inner = inner_sql.trim().trim_end_matches(';').trim();
    let options = match format_idx {
        1 => "(FORMAT JSON, ARRAY true)",
        2 => "(FORMAT PARQUET)",
        _ => "(HEADER, DELIMITER ',')",
    };
    let sql = format!("COPY ({}) TO '{}' {}", inner, path.replace('\'', "''"), options);
    engine.execute_raw(&sql).context("Failed to export result")?;
    Ok(())
}

pub fn export_csv(engine: &DataEngine, path: &str, where_clause: Option<&str>, order_by: Option<&str>) -> Result<()> {
    let mut sql = String::from("COPY (SELECT * FROM data");
    if let Some(w) = where_clause {
        if !w.is_empty() {
            sql.push_str(&format!(" WHERE {}", w));
        }
    }
    if let Some(o) = order_by {
        if !o.is_empty() {
            sql.push_str(&format!(" ORDER BY {}", o));
        }
    }
    sql.push_str(&format!(") TO '{}' (HEADER, DELIMITER ',')", path.replace('\'', "''")));
    engine.execute_raw(&sql).context("Failed to export CSV")?;
    Ok(())
}

pub fn export_json(engine: &DataEngine, path: &str, where_clause: Option<&str>, order_by: Option<&str>) -> Result<()> {
    let mut sql = String::from("COPY (SELECT * FROM data");
    if let Some(w) = where_clause {
        if !w.is_empty() {
            sql.push_str(&format!(" WHERE {}", w));
        }
    }
    if let Some(o) = order_by {
        if !o.is_empty() {
            sql.push_str(&format!(" ORDER BY {}", o));
        }
    }
    sql.push_str(&format!(") TO '{}' (FORMAT JSON, ARRAY true)", path.replace('\'', "''")));
    engine.execute_raw(&sql).context("Failed to export JSON")?;
    Ok(())
}

pub fn export_parquet(engine: &DataEngine, path: &str, where_clause: Option<&str>, order_by: Option<&str>) -> Result<()> {
    let mut sql = String::from("COPY (SELECT * FROM data");
    if let Some(w) = where_clause {
        if !w.is_empty() {
            sql.push_str(&format!(" WHERE {}", w));
        }
    }
    if let Some(o) = order_by {
        if !o.is_empty() {
            sql.push_str(&format!(" ORDER BY {}", o));
        }
    }
    sql.push_str(&format!(") TO '{}' (FORMAT PARQUET)", path.replace('\'', "''")));
    engine.execute_raw(&sql).context("Failed to export Parquet")?;
    Ok(())
}
