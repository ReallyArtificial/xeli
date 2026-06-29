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

/// Export the whole base table to xlsx, materializing every Select column as a
/// real clickable data-validation dropdown. This is the hand-off-to-non-devs
/// artifact: a teammate opens it in Excel/Sheets/LibreOffice and gets the same
/// constrained status column — fully offline, no account, no API.
pub fn export_xlsx(
    engine: &DataEngine,
    path: &str,
    schema: Option<&crate::data::schema::TableSchema>,
) -> Result<()> {
    use rust_xlsxwriter::{DataValidation, Format, Workbook};

    let (columns, rows) = engine.all_rows_as_strings()?;
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_fmt = Format::new().set_bold();
    for (c, col) in columns.iter().enumerate() {
        worksheet.write_string_with_format(0, c as u16, col.name.as_str(), &header_fmt)?;
    }
    for (r, row) in rows.iter().enumerate() {
        for (c, val) in row.iter().enumerate() {
            if val != "NULL" {
                worksheet.write_string((r + 1) as u32, c as u16, val.as_str())?;
            }
        }
    }

    if let Some(schema) = schema {
        // Extend the dropdown well past the current data so new rows are covered.
        let last_row = (rows.len() + 1000).max(1) as u32;
        for (c, col) in columns.iter().enumerate() {
            if let Some(crate::data::schema::ColumnType::Select { values, .. }) =
                schema.col_type(&col.name)
            {
                let labels: Vec<&str> = values.iter().map(|v| v.label.as_str()).collect();
                let dv = DataValidation::new().allow_list_strings(&labels)?;
                worksheet.add_data_validation(1, c as u16, last_row, c as u16, &dv)?;
            }
        }
    }

    workbook
        .save(path)
        .context("Failed to write xlsx file")?;
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
