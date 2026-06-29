use anyhow::{Context, Result};
use duckdb::{params, Connection};

pub struct DataEngine {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
}

/// Read a single cell value from a DuckDB row, handling all types.
/// Tries multiple type conversions since duckdb-rs doesn't auto-cast.
fn read_cell(row: &duckdb::Row, i: usize) -> String {
    // Try String first (VARCHAR, TEXT)
    if let Ok(val) = row.get::<_, Option<String>>(i) {
        return val.unwrap_or_else(|| "NULL".to_string());
    }
    // Try i64 (INTEGER, BIGINT, HUGEINT)
    if let Ok(val) = row.get::<_, Option<i64>>(i) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    // Try i32 (INTEGER)
    if let Ok(val) = row.get::<_, Option<i32>>(i) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    // Try f64 (DOUBLE, FLOAT)
    if let Ok(val) = row.get::<_, Option<f64>>(i) {
        return val
            .map(|v| {
                if v.fract() == 0.0 && v.abs() < 1e15 {
                    format!("{}", v as i64)
                } else {
                    format!("{}", v)
                }
            })
            .unwrap_or_else(|| "NULL".to_string());
    }
    // Try bool (BOOLEAN)
    if let Ok(val) = row.get::<_, Option<bool>>(i) {
        return val
            .map(|v| if v { "true" } else { "false" }.to_string())
            .unwrap_or_else(|| "NULL".to_string());
    }
    // Fallback
    "NULL".to_string()
}

impl DataEngine {
    pub fn new() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("Failed to open DuckDB in-memory database")?;
        Ok(Self { conn })
    }

    pub fn execute_raw(&self, sql: &str) -> Result<()> {
        self.conn
            .execute_batch(sql)
            .context("Failed to execute SQL")?;
        Ok(())
    }

    pub fn load_file(&self, path: &str, format: &str) -> Result<()> {
        let _ = self.conn.execute_batch("DROP TABLE IF EXISTS data");

        let sql = match format {
            "csv" | "tsv" => {
                format!(
                    "CREATE TABLE data AS SELECT * FROM read_csv('{}', auto_detect=true, header=true)",
                    path.replace('\'', "''")
                )
            }
            "json" | "jsonl" | "ndjson" => {
                format!(
                    "CREATE TABLE data AS SELECT * FROM read_json('{}', auto_detect=true)",
                    path.replace('\'', "''")
                )
            }
            "parquet" => {
                format!(
                    "CREATE TABLE data AS SELECT * FROM read_parquet('{}')",
                    path.replace('\'', "''")
                )
            }
            "xlsx" | "xls" => {
                let _ = self.conn.execute_batch("INSTALL spatial; LOAD spatial;");
                format!(
                    "CREATE TABLE data AS SELECT * FROM st_read('{}')",
                    path.replace('\'', "''")
                )
            }
            _ => {
                format!(
                    "CREATE TABLE data AS SELECT * FROM read_csv('{}', auto_detect=true)",
                    path.replace('\'', "''")
                )
            }
        };

        self.conn
            .execute_batch(&sql)
            .with_context(|| format!("Failed to load file as {}", format))?;

        Ok(())
    }

    pub fn get_schema(&self) -> Result<Vec<ColumnInfo>> {
        let mut stmt = self
            .conn
            .prepare("SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'data' ORDER BY ordinal_position")?;

        let columns = stmt
            .query_map(params![], |row| {
                Ok(ColumnInfo {
                    name: row.get(0)?,
                    data_type: row.get(1)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(columns)
    }

    pub fn get_total_rows(&self) -> Result<usize> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM data")?;
        let count: i64 = stmt.query_row(params![], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Deterministic ORDER BY expression shared by pagination, rowid lookup, and
    /// search so a display row, its rowid, and its search ordinal all agree.
    /// `rowid` is the tiebreaker (and sole key when there's no user sort).
    fn order_expr(order_by: Option<&str>) -> String {
        match order_by {
            Some(o) if !o.is_empty() => format!("{}, rowid", o),
            _ => "rowid".to_string(),
        }
    }

    pub fn query_page(
        &self,
        offset: usize,
        limit: usize,
        order_by: Option<&str>,
        where_clause: Option<&str>,
    ) -> Result<QueryResult> {
        let columns = self.get_schema()?;

        // Cast all columns to VARCHAR so we get strings back reliably
        let select_cols: Vec<String> = columns
            .iter()
            .map(|c| {
                let safe = c.name.replace('"', "\"\"");
                format!("\"{}\"::VARCHAR AS \"{}\"", safe, safe)
            })
            .collect();

        let mut sql = format!("SELECT {} FROM data", select_cols.join(", "));

        if let Some(w) = where_clause {
            if !w.is_empty() {
                sql.push_str(&format!(" WHERE {}", w));
            }
        }

        sql.push_str(&format!(" ORDER BY {}", Self::order_expr(order_by)));

        // Get total count with filters
        let count_sql = if let Some(w) = where_clause {
            if !w.is_empty() {
                format!("SELECT COUNT(*) FROM data WHERE {}", w)
            } else {
                "SELECT COUNT(*) FROM data".to_string()
            }
        } else {
            "SELECT COUNT(*) FROM data".to_string()
        };

        let mut count_stmt = self.conn.prepare(&count_sql)?;
        let total_rows: i64 = count_stmt.query_row(params![], |row| row.get(0))?;

        sql.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        let mut stmt = self.conn.prepare(&sql)?;
        let col_count = columns.len();

        let rows: Vec<Vec<String>> = stmt
            .query_map(params![], |row| {
                let mut values = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    values.push(read_cell(row, i));
                }
                Ok(values)
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(QueryResult {
            columns,
            rows,
            total_rows: total_rows as usize,
        })
    }

    pub fn execute_query(&self, sql: &str) -> Result<QueryResult> {
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows_iter = stmt.query(params![])?;

        let (col_count, col_names): (usize, Vec<String>) = match rows_iter.as_ref() {
            Some(s) => {
                let cc = s.column_count();
                let names: Vec<String> = (0..cc)
                    .map(|i| s.column_name(i).map(|n| n.to_string()).unwrap_or_else(|_| "?".to_string()))
                    .collect();
                (cc, names)
            }
            None => (0, Vec::new()),
        };

        let mut all_rows: Vec<Vec<String>> = Vec::new();
        while let Some(row) = rows_iter.next()? {
            let mut values = Vec::with_capacity(col_count);
            for i in 0..col_count {
                values.push(read_cell(row, i));
            }
            all_rows.push(values);
        }

        let total_rows = all_rows.len();
        let columns = col_names
            .into_iter()
            .map(|name| ColumnInfo {
                name,
                data_type: "VARCHAR".to_string(),
            })
            .collect();

        Ok(QueryResult {
            columns,
            rows: all_rows,
            total_rows,
        })
    }

    pub fn get_column_stats(&self, column: &str, where_clause: Option<&str>) -> Result<Vec<(String, String)>> {
        let safe_col = format!("\"{}\"", column.replace('"', "\"\""));
        let where_sql = match where_clause {
            Some(w) if !w.is_empty() => format!(" WHERE {}", w),
            _ => String::new(),
        };
        let and_filter = match where_clause {
            Some(w) if !w.is_empty() => format!(" AND ({})", w),
            _ => String::new(),
        };
        let sql = format!(
            r#"SELECT
                COUNT(*)::VARCHAR,
                COUNT({col})::VARCHAR,
                (COUNT(*) - COUNT({col}))::VARCHAR,
                COUNT(DISTINCT {col})::VARCHAR,
                MIN({col}::VARCHAR)::VARCHAR,
                MAX({col}::VARCHAR)::VARCHAR
            FROM data{where_sql}"#,
            col = safe_col,
            where_sql = where_sql,
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let row = stmt.query_row(params![], |row| {
            Ok(vec![
                ("Total".to_string(), read_cell(row, 0)),
                ("Non-null".to_string(), read_cell(row, 1)),
                ("Nulls".to_string(), read_cell(row, 2)),
                ("Unique".to_string(), read_cell(row, 3)),
                ("Min".to_string(), read_cell(row, 4)),
                ("Max".to_string(), read_cell(row, 5)),
            ])
        })?;

        // Try numeric stats
        let num_sql = format!(
            "SELECT AVG({col})::VARCHAR, MEDIAN({col})::VARCHAR, STDDEV({col})::VARCHAR FROM data WHERE TRY_CAST({col} AS DOUBLE) IS NOT NULL{and_filter}",
            col = safe_col,
            and_filter = and_filter,
        );
        if let Ok(mut num_stmt) = self.conn.prepare(&num_sql) {
            if let Ok(num_row) = num_stmt.query_row(params![], |r| {
                Ok((
                    r.get::<_, Option<String>>(0).unwrap_or(None),
                    r.get::<_, Option<String>>(1).unwrap_or(None),
                    r.get::<_, Option<String>>(2).unwrap_or(None),
                ))
            }) {
                let mut stats = row;
                if let Some(avg) = num_row.0 {
                    stats.push(("Mean".to_string(), avg));
                }
                if let Some(med) = num_row.1 {
                    stats.push(("Median".to_string(), med));
                }
                if let Some(std) = num_row.2 {
                    stats.push(("Std Dev".to_string(), std));
                }
                return Ok(stats);
            }
        }

        Ok(row)
    }

    pub fn get_rowid(
        &self,
        display_offset: usize,
        order_by: Option<&str>,
        where_clause: Option<&str>,
    ) -> Result<i64> {
        let mut sql = "SELECT rowid FROM data".to_string();

        if let Some(w) = where_clause {
            if !w.is_empty() {
                sql.push_str(&format!(" WHERE {}", w));
            }
        }

        sql.push_str(&format!(" ORDER BY {}", Self::order_expr(order_by)));

        sql.push_str(&format!(" LIMIT 1 OFFSET {}", display_offset));

        let mut stmt = self.conn.prepare(&sql)?;
        let rowid: i64 = stmt
            .query_row(params![], |row| row.get(0))
            .context("Failed to resolve rowid for display row")?;

        Ok(rowid)
    }

    /// Find every cell matching `pattern` (an RE2 regex) across the whole
    /// filtered table — not just the loaded page. Returns `(row_ordinal,
    /// col_idx)` pairs where `row_ordinal` is the absolute position in the same
    /// order pagination uses, so highlights and `n`/`N` line up with the view.
    pub fn find_search_matches(
        &self,
        pattern: &str,
        where_clause: Option<&str>,
        order_by: Option<&str>,
        columns: &[String],
    ) -> Result<Vec<(usize, usize)>> {
        if columns.is_empty() || pattern.is_empty() {
            return Ok(Vec::new());
        }
        let safe_pat = pattern.replace('\'', "''");
        let flags: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                format!(
                    "regexp_matches(\"{}\"::VARCHAR, '{}') AS m{}",
                    c.replace('"', "\"\""),
                    safe_pat,
                    i
                )
            })
            .collect();
        let flag_names: Vec<String> = (0..columns.len()).map(|i| format!("m{}", i)).collect();
        let where_sql = match where_clause {
            Some(w) if !w.is_empty() => format!(" WHERE {}", w),
            _ => String::new(),
        };
        let sql = format!(
            "SELECT __rn, {sel} FROM (\
               SELECT (ROW_NUMBER() OVER (ORDER BY {order}) - 1) AS __rn, {flags} FROM data{where_sql}\
             ) WHERE {ors} ORDER BY __rn",
            sel = flag_names.join(", "),
            order = Self::order_expr(order_by),
            flags = flags.join(", "),
            where_sql = where_sql,
            ors = flag_names.join(" OR "),
        );

        let ncols = columns.len();
        let mut stmt = self.conn.prepare(&sql)?;
        let mut matches = Vec::new();
        let rows = stmt.query_map(params![], |row| {
            let rn: i64 = row.get(0)?;
            let mut row_flags = Vec::with_capacity(ncols);
            for i in 0..ncols {
                row_flags.push(row.get::<_, Option<bool>>(i + 1)?.unwrap_or(false));
            }
            Ok((rn as usize, row_flags))
        })?;
        for r in rows {
            let (rn, row_flags) = r?;
            for (col_idx, hit) in row_flags.iter().enumerate() {
                if *hit {
                    matches.push((rn, col_idx));
                }
            }
        }
        Ok(matches)
    }

    pub fn update_cell(&self, rowid: i64, column: &str, new_value: &str) -> Result<()> {
        let safe_col = format!("\"{}\"", column.replace('"', "\"\""));
        let sql = format!(
            "UPDATE data SET {} = ? WHERE rowid = ?",
            safe_col
        );

        self.conn
            .execute(&sql, params![new_value, rowid])
            .context("Failed to update cell")?;

        Ok(())
    }

    pub fn get_histogram_data(&self, column: &str, where_clause: Option<&str>) -> Result<(Vec<(String, usize)>, f64, f64, f64)> {
        let safe_col = format!("\"{}\"", column.replace('"', "\"\""));
        let and_filter = match where_clause {
            Some(w) if !w.is_empty() => format!(" AND ({})", w),
            _ => String::new(),
        };

        // Check if column is numeric
        let check_sql = format!(
            "SELECT COUNT(*) FROM data WHERE TRY_CAST({} AS DOUBLE) IS NOT NULL{and_filter}",
            safe_col,
            and_filter = and_filter,
        );
        let mut check_stmt = self.conn.prepare(&check_sql)?;
        let numeric_count: i64 = check_stmt.query_row(params![], |row| row.get(0))?;
        if numeric_count == 0 {
            anyhow::bail!("Column is not numeric");
        }

        // Get min, max, avg
        let stats_sql = format!(
            "SELECT MIN(TRY_CAST({col} AS DOUBLE)), MAX(TRY_CAST({col} AS DOUBLE)), AVG(TRY_CAST({col} AS DOUBLE)) FROM data WHERE TRY_CAST({col} AS DOUBLE) IS NOT NULL{and_filter}",
            col = safe_col,
            and_filter = and_filter,
        );
        let mut stats_stmt = self.conn.prepare(&stats_sql)?;
        let (min_val, max_val, avg_val): (f64, f64, f64) = stats_stmt.query_row(params![], |row| {
            Ok((
                row.get::<_, f64>(0).unwrap_or(0.0),
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0),
            ))
        })?;

        let bins = 10usize;
        let range = max_val - min_val;

        if range == 0.0 {
            // All values are the same
            let label = format!("{:.2}", min_val);
            return Ok((vec![(label, numeric_count as usize)], min_val, max_val, avg_val));
        }

        let bin_width = range / bins as f64;

        let hist_sql = format!(
            "SELECT LEAST(FLOOR((TRY_CAST({col} AS DOUBLE) - {min}) / {bw}), {max_bin})::INTEGER AS bin, COUNT(*) AS cnt \
             FROM data WHERE TRY_CAST({col} AS DOUBLE) IS NOT NULL{and_filter} \
             GROUP BY bin ORDER BY bin",
            col = safe_col,
            min = min_val,
            bw = bin_width,
            max_bin = bins - 1,
            and_filter = and_filter,
        );
        let mut hist_stmt = self.conn.prepare(&hist_sql)?;
        let hist_rows: Vec<(i32, usize)> = hist_stmt
            .query_map(params![], |row| {
                Ok((
                    row.get::<_, i32>(0).unwrap_or(0),
                    row.get::<_, i64>(1).unwrap_or(0) as usize,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut data: Vec<(String, usize)> = Vec::with_capacity(bins);
        for i in 0..bins {
            let lo = min_val + i as f64 * bin_width;
            let hi = lo + bin_width;
            let label = format!("{:.1}-{:.1}", lo, hi);
            let count = hist_rows
                .iter()
                .find(|(b, _)| *b == i as i32)
                .map(|(_, c)| *c)
                .unwrap_or(0);
            data.push((label, count));
        }

        Ok((data, min_val, max_val, avg_val))
    }

    /// The SQL a formula-bar expression expands to. Exposed so callers can
    /// record it (e.g. for Export) alongside running it.
    pub fn expression_sql(expr: &str) -> String {
        let upper = expr.to_uppercase();
        let is_aggregate = upper.contains("SUM(") || upper.contains("AVG(") || upper.contains("COUNT(")
            || upper.contains("MIN(") || upper.contains("MAX(")
            || upper.contains("MEDIAN(") || upper.contains("STDDEV(");

        if is_aggregate {
            format!("SELECT {} AS result FROM data", expr)
        } else {
            format!("SELECT *, ({}) AS result FROM data", expr)
        }
    }

    pub fn evaluate_expression(&self, expr: &str) -> Result<QueryResult> {
        self.execute_query(&Self::expression_sql(expr))
    }

    pub fn add_computed_column(&self, name: &str, expr: &str) -> Result<()> {
        let safe_name = name.replace('"', "\"\"");
        let sql = format!(
            "ALTER TABLE data ADD COLUMN \"{}\" VARCHAR",
            safe_name
        );
        self.conn.execute_batch(&sql)
            .with_context(|| format!("Failed to add column '{}'", name))?;

        let update_sql = format!(
            "UPDATE data SET \"{}\" = ({})::VARCHAR",
            safe_name, expr
        );
        self.conn.execute_batch(&update_sql)
            .with_context(|| format!("Failed to compute column '{}': expression error", name))?;

        Ok(())
    }

    pub fn load_as_table(&self, path: &str, format: &str, table_name: &str) -> Result<()> {
        let _ = self.conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", table_name));

        let sql = match format {
            "csv" | "tsv" => {
                format!(
                    "CREATE TABLE {} AS SELECT * FROM read_csv('{}', auto_detect=true, header=true)",
                    table_name, path.replace('\'', "''")
                )
            }
            "json" | "jsonl" | "ndjson" => {
                format!(
                    "CREATE TABLE {} AS SELECT * FROM read_json('{}', auto_detect=true)",
                    table_name, path.replace('\'', "''")
                )
            }
            "parquet" => {
                format!(
                    "CREATE TABLE {} AS SELECT * FROM read_parquet('{}')",
                    table_name, path.replace('\'', "''")
                )
            }
            "xlsx" | "xls" => {
                let _ = self.conn.execute_batch("INSTALL spatial; LOAD spatial;");
                format!(
                    "CREATE TABLE {} AS SELECT * FROM st_read('{}')",
                    table_name, path.replace('\'', "''")
                )
            }
            _ => {
                format!(
                    "CREATE TABLE {} AS SELECT * FROM read_csv('{}', auto_detect=true)",
                    table_name, path.replace('\'', "''")
                )
            }
        };

        self.conn
            .execute_batch(&sql)
            .with_context(|| format!("Failed to load '{}' as table '{}'", path, table_name))?;

        Ok(())
    }

    pub fn get_table_schema(&self, table_name: &str) -> Result<Vec<ColumnInfo>> {
        let sql = format!(
            "SELECT column_name, data_type FROM information_schema.columns WHERE table_name = '{}' ORDER BY ordinal_position",
            table_name.replace('\'', "''")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let columns = stmt
            .query_map(params![], |row| {
                Ok(ColumnInfo {
                    name: row.get(0)?,
                    data_type: row.get(1)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(columns)
    }

    pub fn execute_join(&self, join_type: &str, col1: &str, col2: &str) -> Result<()> {
        let safe_col1 = col1.replace('"', "\"\"");
        let safe_col2 = col2.replace('"', "\"\"");

        // `SELECT data.*, data2.*` errors (or yields ambiguous columns) whenever
        // the two files share a column name — which the join key always does if
        // both sides name it the same. Build the projection explicitly and
        // suffix any overlapping right-side column with `_2`.
        let left_cols = self.get_schema()?;
        let right_cols = self.get_table_schema("data2")?;
        let left_names: Vec<String> = left_cols.iter().map(|c| c.name.clone()).collect();

        let mut select_parts: Vec<String> = left_cols
            .iter()
            .map(|c| format!("data.\"{}\"", c.name.replace('"', "\"\"")))
            .collect();
        for c in &right_cols {
            let safe = c.name.replace('"', "\"\"");
            if left_names.contains(&c.name) {
                let alias = format!("{}_2", c.name).replace('"', "\"\"");
                select_parts.push(format!("data2.\"{}\" AS \"{}\"", safe, alias));
            } else {
                select_parts.push(format!("data2.\"{}\"", safe));
            }
        }

        let sql = format!(
            "CREATE OR REPLACE TABLE data AS \
             SELECT {} \
             FROM data {join_type} JOIN data2 \
             ON data.\"{}\" = data2.\"{}\"",
            select_parts.join(", "),
            safe_col1,
            safe_col2,
            join_type = join_type,
        );
        self.conn.execute_batch(&sql)
            .context("Failed to execute join")?;

        let _ = self.conn.execute_batch("DROP TABLE IF EXISTS data2");
        Ok(())
    }

    pub fn get_sample_values(&self, column: &str, limit: usize) -> Result<Vec<String>> {
        let safe_col = format!("\"{}\"", column.replace('"', "\"\""));
        let sql = format!(
            "SELECT DISTINCT {}::VARCHAR FROM data WHERE {} IS NOT NULL LIMIT {}",
            safe_col, safe_col, limit
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let values = stmt
            .query_map(params![], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(values)
    }

    // --- Table creation from a typed schema ---------------------------------
    //
    // Created tables are stored as plain VARCHAR columns: editing never hits a
    // cast error and the file stays as texty as a CSV. The semantic types live
    // in the `TableSchema` sidecar, not in DuckDB.

    /// Create the base `data` table from a schema and seed it with blank rows so
    /// the user lands on a ready-to-fill grid (select columns get their default,
    /// which is why a fresh tracker shows status pills immediately).
    pub fn create_table_from_schema(
        &self,
        schema: &crate::data::schema::TableSchema,
        seed_rows: usize,
    ) -> Result<()> {
        let _ = self.conn.execute_batch("DROP TABLE IF EXISTS data");

        let cols: Vec<String> = schema
            .columns
            .iter()
            .map(|c| format!("\"{}\" VARCHAR", c.name.replace('"', "\"\"")))
            .collect();
        if cols.is_empty() {
            anyhow::bail!("a table needs at least one column");
        }
        let ddl = format!("CREATE TABLE data ({})", cols.join(", "));
        self.conn
            .execute_batch(&ddl)
            .context("Failed to create table")?;

        for _ in 0..seed_rows {
            self.insert_blank_row(schema)?;
        }
        Ok(())
    }

    /// Insert one blank row: select columns get their default value, everything
    /// else is NULL (rendered as an empty cell).
    pub fn insert_blank_row(&self, schema: &crate::data::schema::TableSchema) -> Result<()> {
        let names: Vec<String> = schema
            .columns
            .iter()
            .map(|c| format!("\"{}\"", c.name.replace('"', "\"\"")))
            .collect();
        let values: Vec<String> = schema
            .columns
            .iter()
            .map(|c| match c.col_type.default_value() {
                Some(v) => format!("'{}'", v.replace('\'', "''")),
                None => "NULL".to_string(),
            })
            .collect();
        let sql = format!(
            "INSERT INTO data ({}) VALUES ({})",
            names.join(", "),
            values.join(", ")
        );
        self.conn
            .execute_batch(&sql)
            .context("Failed to add row")?;
        Ok(())
    }

    /// Add a new column to `data`. If it's a Select column with a default, every
    /// existing row is back-filled with that default so the column isn't a wall
    /// of blanks the moment it appears.
    pub fn add_typed_column(
        &self,
        spec: &crate::data::schema::ColumnSpec,
    ) -> Result<()> {
        let safe = spec.name.replace('"', "\"\"");
        self.conn
            .execute_batch(&format!("ALTER TABLE data ADD COLUMN \"{}\" VARCHAR", safe))
            .with_context(|| format!("Failed to add column '{}'", spec.name))?;

        if let Some(default) = spec.col_type.default_value() {
            self.conn
                .execute_batch(&format!(
                    "UPDATE data SET \"{}\" = '{}'",
                    safe,
                    default.replace('\'', "''")
                ))
                .ok();
        }
        Ok(())
    }

    /// Delete the row at a display offset (resolved through the same ordering as
    /// pagination so the cursor row is the row that goes).
    pub fn delete_row_at(
        &self,
        display_offset: usize,
        order_by: Option<&str>,
        where_clause: Option<&str>,
    ) -> Result<()> {
        let rowid = self.get_rowid(display_offset, order_by, where_clause)?;
        self.conn
            .execute("DELETE FROM data WHERE rowid = ?", params![rowid])
            .context("Failed to delete row")?;
        Ok(())
    }

    /// Every row of `data` as strings, in rowid order — used by xlsx export where
    /// we need the whole table, not a page.
    pub fn all_rows_as_strings(&self) -> Result<(Vec<ColumnInfo>, Vec<Vec<String>>)> {
        let columns = self.get_schema()?;
        let select_cols: Vec<String> = columns
            .iter()
            .map(|c| {
                let safe = c.name.replace('"', "\"\"");
                format!("\"{}\"::VARCHAR AS \"{}\"", safe, safe)
            })
            .collect();
        let sql = format!(
            "SELECT {} FROM data ORDER BY rowid",
            select_cols.join(", ")
        );
        let col_count = columns.len();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows: Vec<Vec<String>> = stmt
            .query_map(params![], |row| {
                let mut values = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    values.push(read_cell(row, i));
                }
                Ok(values)
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok((columns, rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(stats: &[(String, String)], key: &str) -> String {
        stats.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap()
    }

    #[test]
    fn search_finds_matches_beyond_the_first_page() {
        let engine = DataEngine::new().unwrap();
        // 200 rows — well past the 100-row page size.
        engine
            .execute_raw("CREATE TABLE data AS SELECT i AS id, ('row' || i) AS label FROM range(0, 200) t(i)")
            .unwrap();
        let cols = vec!["id".to_string(), "label".to_string()];

        let matches = engine
            .find_search_matches("^row150$", None, None, &cols)
            .unwrap();
        // Row ordinal 150 (rowid order), column index 1 (label).
        assert_eq!(matches, vec![(150, 1)]);
    }

    #[test]
    fn search_respects_active_filter() {
        let engine = DataEngine::new().unwrap();
        engine
            .execute_raw("CREATE TABLE data AS SELECT i AS id, ('row' || i) AS label FROM range(0, 10) t(i)")
            .unwrap();
        let cols = vec!["id".to_string(), "label".to_string()];

        // With a filter excluding id>=5, 'row7' must not be found.
        let matches = engine
            .find_search_matches("row7", Some("\"id\" < 5"), None, &cols)
            .unwrap();
        assert!(matches.is_empty(), "filtered-out row should not match: {matches:?}");
    }

    #[test]
    fn column_stats_respect_filter() {
        let engine = DataEngine::new().unwrap();
        engine
            .execute_raw("CREATE TABLE data AS SELECT i AS id, (i % 2) AS grp FROM range(0, 100) t(i)")
            .unwrap();

        let all = engine.get_column_stats("id", None).unwrap();
        assert_eq!(stat(&all, "Total"), "100");

        let filtered = engine.get_column_stats("id", Some("\"grp\" = 0")).unwrap();
        assert_eq!(stat(&filtered, "Total"), "50");
    }

    #[test]
    fn export_result_writes_the_query_not_the_base_table() {
        let engine = DataEngine::new().unwrap();
        engine
            .execute_raw("CREATE TABLE data AS SELECT i AS id FROM range(0, 5) t(i)")
            .unwrap();
        let path = std::env::temp_dir().join("xeli_export_result_test.csv");
        let p = path.to_str().unwrap();

        // Trailing semicolon must be tolerated when wrapping in COPY(...).
        crate::data::export::export_result(&engine, p, 0, "SELECT id FROM data WHERE id < 3;").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[0], "id");
        assert_eq!(lines.len(), 4, "header + 3 filtered rows, got {lines:?}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn create_seed_add_delete_and_export_roundtrip() {
        use crate::data::schema::{self, ColumnSpec, ColumnType, TableSchema};

        let engine = DataEngine::new().unwrap();
        let schema = TableSchema::new(vec![
            ColumnSpec { name: "task".into(), col_type: ColumnType::Text },
            ColumnSpec {
                name: "status".into(),
                col_type: schema::select_from_labels(&["Todo", "Doing", "Done"]),
            },
        ]);

        // Create + seed 3 blank rows; the select default fills in.
        engine.create_table_from_schema(&schema, 3).unwrap();
        assert_eq!(engine.get_total_rows().unwrap(), 3);
        let cols: Vec<String> = engine.get_schema().unwrap().iter().map(|c| c.name.clone()).collect();
        assert_eq!(cols, vec!["task", "status"]);

        // Seeded rows carry the select default so the grid shows pills immediately.
        let (_c, rows) = engine.all_rows_as_strings().unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r[1] == "Todo"), "default status not seeded: {rows:?}");
        assert!(rows.iter().all(|r| r[0] == "NULL"), "non-default col should be NULL: {rows:?}");

        // Add a row, then a typed column (back-filled with its default).
        engine.insert_blank_row(&schema).unwrap();
        assert_eq!(engine.get_total_rows().unwrap(), 4);
        let prio = ColumnSpec { name: "priority".into(), col_type: schema::select_from_labels(&["Low", "High"]) };
        engine.add_typed_column(&prio).unwrap();
        let (_c2, rows2) = engine.all_rows_as_strings().unwrap();
        assert!(rows2.iter().all(|r| r[2] == "Low"), "new column not back-filled: {rows2:?}");

        // Delete one row.
        engine.delete_row_at(0, None, None).unwrap();
        assert_eq!(engine.get_total_rows().unwrap(), 3);

        // CSV export is plain text with the right header.
        let dir = std::env::temp_dir().join("xeli_create_test");
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("t.csv");
        crate::data::export::export_csv(&engine, csv.to_str().unwrap(), None, Some("rowid")).unwrap();
        let content = std::fs::read_to_string(&csv).unwrap();
        assert_eq!(content.lines().next().unwrap(), "task,status,priority");
        assert_eq!(content.lines().count(), 4, "header + 3 rows");

        // xlsx export with dropdowns produces a real (zip) workbook.
        let mut full = schema.clone();
        full.columns.push(prio);
        let xlsx = dir.join("t.xlsx");
        crate::data::export::export_xlsx(&engine, xlsx.to_str().unwrap(), Some(&full)).unwrap();
        let bytes = std::fs::read(&xlsx).unwrap();
        assert!(bytes.len() > 100, "xlsx should be non-trivial");
        assert_eq!(&bytes[0..2], b"PK", "xlsx should be a zip archive");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn join_dedups_overlapping_column_names() {
        let engine = DataEngine::new().unwrap();
        engine
            .execute_raw("CREATE TABLE data AS SELECT i AS id, ('L' || i) AS name FROM range(0, 3) t(i)")
            .unwrap();
        engine
            .execute_raw("CREATE TABLE data2 AS SELECT i AS id, ('R' || i) AS name FROM range(0, 3) t(i)")
            .unwrap();

        // Before the fix this errored with duplicate column names.
        engine.execute_join("INNER", "id", "id").unwrap();
        let names: Vec<String> = engine.get_schema().unwrap().iter().map(|c| c.name.clone()).collect();

        let mut deduped = names.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), names.len(), "duplicate column names: {names:?}");
        assert!(names.contains(&"id_2".to_string()), "missing suffixed key: {names:?}");
        assert!(names.contains(&"name_2".to_string()), "missing suffixed col: {names:?}");
    }
}
