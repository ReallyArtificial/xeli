use crate::data::engine::{ColumnInfo, DataEngine};
use crate::data::loader::FileFormat;
use crate::data::schema::{ColumnType, SelectValue, TableSchema};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Search,
    Filter,
    Command,
    AiQuery,
    AiKeySetup,
    SqlQuery,
    CellDetail,
    CellEdit,
    ColumnStats,
    Export,
    Help,
    Sparkline,
    FormulaBar,
    ComputedColumn,
    GroupBy,
    Join,
    /// Add a new typed column (name → type → values).
    NewColumn,
    /// Pick a value for a Select cell from its dropdown.
    SelectCell,
    /// Kanban board grouped by a Select column.
    Board,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NewColumnStage {
    EnterName,
    PickType,
    /// Only for Select columns: choose a ready-made value set.
    PickPreset,
    /// Only for Select columns: type a custom comma-separated value set.
    EnterValues,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiKeyStage {
    PickProvider,
    EnterKey,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    None,
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct Filter {
    pub column: String,
    pub operator: String,
    pub value: String,
    pub enabled: bool,
}

impl Filter {
    pub fn to_sql(&self) -> String {
        let safe_col = format!("\"{}\"", self.column.replace('"', "\"\""));
        match self.operator.as_str() {
            "=" => format!("{} = '{}'", safe_col, self.value.replace('\'', "''")),
            "!=" => format!("{} != '{}'", safe_col, self.value.replace('\'', "''")),
            ">" => format!("{} > '{}'", safe_col, self.value.replace('\'', "''")),
            ">=" => format!("{} >= '{}'", safe_col, self.value.replace('\'', "''")),
            "<" => format!("{} < '{}'", safe_col, self.value.replace('\'', "''")),
            "<=" => format!("{} <= '{}'", safe_col, self.value.replace('\'', "''")),
            "contains" => format!("{} ILIKE '%{}%'", safe_col, self.value.replace('\'', "''")),
            "starts with" => format!("{} ILIKE '{}%'", safe_col, self.value.replace('\'', "''")),
            "ends with" => format!("{} ILIKE '%{}'", safe_col, self.value.replace('\'', "''")),
            "is null" => format!("{} IS NULL", safe_col),
            "is not null" => format!("{} IS NOT NULL", safe_col),
            "regex" => format!("regexp_matches({}::VARCHAR, '{}')", safe_col, self.value.replace('\'', "''")),
            _ => format!("{} = '{}'", safe_col, self.value.replace('\'', "''")),
        }
    }

    pub fn display(&self) -> String {
        match self.operator.as_str() {
            "is null" | "is not null" => format!("{} {}", self.column, self.operator),
            _ => format!("{} {} {}", self.column, self.operator, self.value),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    Dracula,
    Nord,
    Catppuccin,
    TokyoNight,
    Solarized,
}

impl Theme {
    pub fn name(&self) -> &str {
        match self {
            Theme::Dracula => "Dracula",
            Theme::Nord => "Nord",
            Theme::Catppuccin => "Catppuccin",
            Theme::TokyoNight => "Tokyo Night",
            Theme::Solarized => "Solarized",
        }
    }

    pub fn next(&self) -> Theme {
        match self {
            Theme::Dracula => Theme::Nord,
            Theme::Nord => Theme::Catppuccin,
            Theme::Catppuccin => Theme::TokyoNight,
            Theme::TokyoNight => Theme::Solarized,
            Theme::Solarized => Theme::Dracula,
        }
    }
}

pub struct App {
    pub mode: AppMode,
    pub file_path: String,
    pub file_format: FileFormat,
    pub engine: DataEngine,

    // Data
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
    pub filtered_rows: usize,

    /// Column names of the base `data` table as loaded — never changes, even
    /// after a query swaps out `columns`. Used to hint SQL/AI mode.
    pub data_columns: Vec<String>,

    /// True while the table shows an ad-hoc query result (AI / SQL / formula /
    /// group-by) rather than the base `data` table. Such results are held
    /// whole in `rows`, so we must not let `refresh_data` re-page over `data`.
    pub viewing_query_result: bool,

    /// The SQL that produced the current query result, so Export can write the
    /// result the user is looking at (not the base table). `None` on the base table.
    pub result_sql: Option<String>,

    // Cursor
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_offset: usize,
    pub col_scroll_offset: usize,

    // Column widths
    pub col_widths: Vec<u16>,
    pub hidden_cols: Vec<bool>,
    pub min_col_width: u16,
    pub max_col_width: u16,

    // Sort
    pub sort_column: Option<usize>,
    pub sort_direction: SortDirection,

    // Filters
    pub filters: Vec<Filter>,
    pub filter_input: String,
    pub filter_stage: FilterStage,
    pub filter_selected_col: usize,
    pub filter_selected_op: usize,

    // Search
    pub search_query: String,
    pub search_matches: Vec<(usize, usize)>,
    pub search_match_idx: usize,

    // Input buffers
    pub input_buffer: String,
    pub ai_response: String,
    pub ai_loading: bool,
    pub ai_sql: String,

    // SQL mode
    pub sql_input: String,
    pub sql_history: Vec<String>,
    pub sql_history_idx: Option<usize>,

    // Column stats
    pub stats_data: Vec<(String, String)>,

    // Export
    pub export_format_idx: usize,
    pub export_path: String,

    // Command palette
    pub command_query: String,
    pub command_selected: usize,

    // UI
    pub theme: Theme,
    pub page_size: usize,
    pub viewport_width: u16,
    pub viewport_height: u16,
    pub show_row_numbers: bool,

    // Messages
    pub status_message: Option<String>,
    pub error_message: Option<String>,

    // State
    pub should_quit: bool,

    // Undo
    pub view_stack: Vec<ViewState>,

    // Cell editing
    pub edit_buffer: String,
    pub edit_original: String,
    pub edit_cursor_pos: usize,
    pub edit_history: Vec<CellEditRecord>,

    // Sparkline
    pub sparkline_data: Vec<(String, usize)>,
    pub sparkline_min: f64,
    pub sparkline_max: f64,
    pub sparkline_avg: f64,

    // Formula bar
    pub formula_input: String,

    // Computed column
    pub computed_col_stage: ComputedColumnStage,
    pub computed_col_name: String,
    pub computed_col_expr: String,

    // Group-by wizard
    pub groupby_stage: GroupByStage,
    pub groupby_selected: Vec<bool>,
    pub groupby_cursor: usize,
    pub groupby_agg_col: usize,
    pub groupby_agg_func_idx: usize,

    // Join wizard
    pub join_stage: JoinStage,
    pub join_path: String,
    pub join_type_idx: usize,
    pub join_col1_idx: usize,
    pub join_col2_idx: usize,
    pub join_data2_cols: Vec<ColumnInfo>,
    pub join_active_side: u8,

    // AI key setup (shown when user hits Ctrl+K with no key configured)
    pub ai_key_stage: AiKeyStage,
    pub ai_key_provider_idx: usize, // 0 = Anthropic, 1 = OpenAI
    pub ai_key_input: String,

    // --- Create / typed-column feature ---
    /// The semantic types of the current table's columns, when known (created
    /// tables, or files with a `*.xeli.json` sidecar). Drives pills, the select
    /// dropdown, type-aware sorting, and xlsx dropdown export. `None` = a plain
    /// untyped table (fully backwards-compatible).
    pub schema: Option<TableSchema>,
    /// Set once the table has unsaved edits, so the title can show a `●` and
    /// quit can nudge. Created tables start dirty (nothing on disk yet).
    pub dirty: bool,

    // New-column wizard
    pub new_col_stage: NewColumnStage,
    pub new_col_name: String,
    pub new_col_type_idx: usize,
    pub new_col_preset_idx: usize,
    pub new_col_values_input: String,

    // Select-cell dropdown
    pub select_options: Vec<SelectValue>,
    pub select_cursor: usize,
    pub select_filter: String,

    // Board (kanban) view
    pub board_group_col: Option<usize>,
    pub board_lane_cursor: usize,
    pub board_card_cursor: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilterStage {
    SelectColumn,
    SelectOperator,
    EnterValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComputedColumnStage {
    EnterName,
    EnterExpression,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupByStage {
    SelectGroupCols,
    SelectAggregation,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JoinStage {
    EnterPath,
    SelectJoinType,
    SelectColumns,
}

#[derive(Debug, Clone)]
pub struct ViewState {
    pub sort_column: Option<usize>,
    pub sort_direction: SortDirection,
    pub filters: Vec<Filter>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_offset: usize,
}

#[derive(Debug, Clone)]
pub struct CellEditRecord {
    pub row: usize,
    pub col_name: String,
    pub old_value: String,
    pub new_value: String,
}

impl App {
    pub fn new(file_path: String, file_format: FileFormat, engine: DataEngine) -> Result<Self> {
        let columns = engine.get_schema()?;
        let data_columns: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
        let total_rows = engine.get_total_rows()?;
        let col_count = columns.len();
        let col_widths = vec![15u16; col_count];
        let hidden_cols = vec![false; col_count];

        let mut app = App {
            mode: AppMode::Normal,
            file_path,
            file_format,
            engine,
            columns,
            rows: Vec::new(),
            total_rows,
            filtered_rows: total_rows,
            data_columns,
            viewing_query_result: false,
            result_sql: None,
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            col_scroll_offset: 0,
            col_widths,
            hidden_cols,
            min_col_width: 4,
            max_col_width: 60,
            sort_column: None,
            sort_direction: SortDirection::None,
            filters: Vec::new(),
            filter_input: String::new(),
            filter_stage: FilterStage::SelectColumn,
            filter_selected_col: 0,
            filter_selected_op: 0,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_idx: 0,
            input_buffer: String::new(),
            ai_response: String::new(),
            ai_loading: false,
            ai_sql: String::new(),
            sql_input: String::new(),
            sql_history: Vec::new(),
            sql_history_idx: None,
            stats_data: Vec::new(),
            export_format_idx: 0,
            export_path: String::new(),
            command_query: String::new(),
            command_selected: 0,
            theme: Theme::Dracula,
            page_size: 100,
            viewport_width: 80,
            viewport_height: 24,
            show_row_numbers: true,
            status_message: None,
            error_message: None,
            should_quit: false,
            view_stack: Vec::new(),
            edit_buffer: String::new(),
            edit_original: String::new(),
            edit_cursor_pos: 0,
            edit_history: Vec::new(),

            sparkline_data: Vec::new(),
            sparkline_min: 0.0,
            sparkline_max: 0.0,
            sparkline_avg: 0.0,

            formula_input: String::new(),

            computed_col_stage: ComputedColumnStage::EnterName,
            computed_col_name: String::new(),
            computed_col_expr: String::new(),

            groupby_stage: GroupByStage::SelectGroupCols,
            groupby_selected: Vec::new(),
            groupby_cursor: 0,
            groupby_agg_col: 0,
            groupby_agg_func_idx: 0,

            join_stage: JoinStage::EnterPath,
            join_path: String::new(),
            join_type_idx: 0,
            join_col1_idx: 0,
            join_col2_idx: 0,
            join_data2_cols: Vec::new(),
            join_active_side: 0,

            ai_key_stage: AiKeyStage::PickProvider,
            ai_key_provider_idx: 0,
            ai_key_input: String::new(),

            schema: None,
            dirty: false,
            new_col_stage: NewColumnStage::EnterName,
            new_col_name: String::new(),
            new_col_type_idx: 0,
            new_col_preset_idx: 0,
            new_col_values_input: String::new(),
            select_options: Vec::new(),
            select_cursor: 0,
            select_filter: String::new(),
            board_group_col: None,
            board_lane_cursor: 0,
            board_card_cursor: 0,
        };

        app.refresh_data()?;
        app.auto_size_columns();

        Ok(app)
    }

    pub fn refresh_data(&mut self) -> Result<()> {
        // A query result is held whole in `rows`; re-paging `data` here would
        // overwrite it with mismatched base-table data. Scrolling/sorting a
        // result is a no-op until the user returns to the base table.
        if self.viewing_query_result {
            return Ok(());
        }

        let order_by = self.build_order_by();
        let where_clause = self.build_where_clause();

        let result = self.engine.query_page(
            self.scroll_offset,
            self.page_size,
            order_by.as_deref(),
            where_clause.as_deref(),
        )?;

        self.rows = result.rows;
        self.filtered_rows = result.total_rows;

        Ok(())
    }

    pub fn auto_size_columns(&mut self) {
        for (i, col) in self.columns.iter().enumerate() {
            let header_width = col.name.len() as u16 + 2; // padding
            let mut max_content = header_width;

            for row in &self.rows {
                if let Some(val) = row.get(i) {
                    let w = val.len() as u16 + 2;
                    if w > max_content {
                        max_content = w;
                    }
                }
            }

            self.col_widths[i] = max_content.clamp(self.min_col_width, self.max_col_width);
        }
    }

    pub fn build_order_by(&self) -> Option<String> {
        if let (Some(col_idx), dir) = (&self.sort_column, &self.sort_direction) {
            if *dir != SortDirection::None {
                if let Some(col) = self.columns.get(*col_idx) {
                    let dir_str = match dir {
                        SortDirection::Asc => "ASC",
                        SortDirection::Desc => "DESC",
                        SortDirection::None => unreachable!(),
                    };
                    let safe = col.name.replace('"', "\"\"");
                    // Numeric typed columns are stored as VARCHAR, so order by a
                    // numeric cast (NULLs/garbage fall to the end) instead of
                    // comparing "100" < "9" lexicographically.
                    let expr = match self.col_type_of(&col.name) {
                        Some(t) if t.is_numeric() => {
                            format!("TRY_CAST(\"{}\" AS DOUBLE) {} NULLS LAST", safe, dir_str)
                        }
                        _ => format!("\"{}\" {}", safe, dir_str),
                    };
                    return Some(expr);
                }
            }
        }
        None
    }

    /// The semantic type of a column by name, if a schema is loaded.
    pub fn col_type_of(&self, name: &str) -> Option<&ColumnType> {
        self.schema.as_ref().and_then(|s| s.col_type(name))
    }

    /// The visible column the cursor is on (actual index, name, type).
    pub fn current_column_name(&self) -> Option<String> {
        self.visible_columns()
            .get(self.cursor_col)
            .map(|(_, c)| c.name.clone())
    }

    pub fn current_column_type(&self) -> Option<ColumnType> {
        let name = self.current_column_name()?;
        self.col_type_of(&name).cloned()
    }

    /// Select-cell options narrowed by the live filter (subsequence match).
    pub fn select_filtered_options(&self) -> Vec<SelectValue> {
        if self.select_filter.is_empty() {
            return self.select_options.clone();
        }
        let q = self.select_filter.to_lowercase();
        self.select_options
            .iter()
            .filter(|v| v.label.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    /// First Select column index (visible position) — the board's default lane source.
    pub fn first_select_col(&self) -> Option<usize> {
        self.visible_columns()
            .iter()
            .position(|(_, c)| matches!(self.col_type_of(&c.name), Some(ColumnType::Select { .. })))
    }

    pub fn build_where_clause(&self) -> Option<String> {
        let active_filters: Vec<String> = self
            .filters
            .iter()
            .filter(|f| f.enabled)
            .map(|f| f.to_sql())
            .collect();

        if active_filters.is_empty() {
            None
        } else {
            Some(active_filters.join(" AND "))
        }
    }

    /// Return to the full base `data` table from a query result, clearing
    /// filters and sort so the user gets a clean slate to work from.
    pub fn reset_to_base_table(&mut self) -> Result<()> {
        self.viewing_query_result = false;
        self.result_sql = None;
        self.columns = self.engine.get_schema()?;
        let n = self.columns.len();
        self.col_widths = vec![15u16; n];
        self.hidden_cols = vec![false; n];
        self.sort_column = None;
        self.sort_direction = SortDirection::None;
        self.filters.clear();
        self.scroll_offset = 0;
        self.col_scroll_offset = 0;
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.view_stack.clear();
        self.refresh_data()?;
        self.auto_size_columns();
        Ok(())
    }

    pub fn push_view_state(&mut self) {
        self.view_stack.push(ViewState {
            sort_column: self.sort_column,
            sort_direction: self.sort_direction.clone(),
            filters: self.filters.clone(),
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
            scroll_offset: self.scroll_offset,
        });
    }

    pub fn pop_view_state(&mut self) -> Result<bool> {
        if let Some(state) = self.view_stack.pop() {
            self.sort_column = state.sort_column;
            self.sort_direction = state.sort_direction;
            self.filters = state.filters;
            self.cursor_row = state.cursor_row;
            self.cursor_col = state.cursor_col;
            self.scroll_offset = state.scroll_offset;
            self.refresh_data()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn visible_columns(&self) -> Vec<(usize, &ColumnInfo)> {
        self.columns
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.hidden_cols.get(*i).copied().unwrap_or(false))
            .collect()
    }

    pub fn filename(&self) -> &str {
        std::path::Path::new(&self.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.file_path)
    }

    pub fn current_cell_value(&self) -> Option<&str> {
        let row_idx = self.cursor_row.checked_sub(self.scroll_offset)?;
        let row = self.rows.get(row_idx)?;
        let vis_cols = self.visible_columns();
        let (actual_col, _) = vis_cols.get(self.cursor_col)?;
        row.get(*actual_col).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::engine::DataEngine;

    fn load_employees() -> App {
        let engine = DataEngine::new().unwrap();
        engine.load_file("examples/employees.csv", "csv").unwrap();
        App::new(
            "examples/employees.csv".to_string(),
            FileFormat::Csv,
            engine,
        )
        .unwrap()
    }

    #[test]
    fn reset_restores_base_table_after_query_result() {
        let mut app = load_employees();
        app.refresh_data().unwrap();
        let base_cols = app.data_columns.clone();
        let base_row_count = app.filtered_rows;
        assert!(base_row_count > 0, "fixture should have rows");
        assert!(base_cols.len() >= 5, "fixture should have several columns");

        // Simulate an AI/SQL query result replacing the view with a narrow,
        // detached projection.
        app.viewing_query_result = true;
        app.columns = vec![ColumnInfo {
            name: "total".to_string(),
            data_type: "VARCHAR".to_string(),
        }];
        app.rows = vec![vec!["999".to_string()]];
        app.filtered_rows = 1;

        // While detached, refresh_data must NOT clobber the in-memory result.
        app.refresh_data().unwrap();
        assert_eq!(app.columns.len(), 1, "refresh should be a no-op on a result");
        assert_eq!(app.rows, vec![vec!["999".to_string()]]);

        // Returning restores the full base table.
        app.reset_to_base_table().unwrap();
        assert!(!app.viewing_query_result);
        assert_eq!(
            app.columns.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
            base_cols
        );
        assert_eq!(app.filtered_rows, base_row_count);
        assert!(!app.rows.is_empty(), "rows should be reloaded from data");
        assert!(app.filters.is_empty());
        assert_eq!(app.sort_column, None);
    }
}
