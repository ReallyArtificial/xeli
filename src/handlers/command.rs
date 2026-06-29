use crate::app::{App, AppMode, ComputedColumnStage, GroupByStage, JoinStage};
use crate::event::AppEvent;
use tokio::sync::mpsc;

pub fn execute_command(app: &mut App, command_name: &str, _ai_tx: &mpsc::UnboundedSender<AppEvent>) {
    // Structural commands are defined over the base `data` table; if a query
    // result is showing, snap back first (mirrors the keyboard handler).
    const STRUCTURAL: &[&str] = &[
        "Sort", "Filter", "Clear Filters", "Group By", "Join",
        "Formula Bar", "Computed Column", "Sparkline", "Column Stats",
        "Add Row", "Add Column", "Board View",
    ];
    if app.viewing_query_result && STRUCTURAL.contains(&command_name) {
        crate::handlers::input::return_to_base_table(app);
    }

    match command_name {
        "Search" => {
            app.mode = AppMode::Search;
            app.search_query.clear();
            app.search_matches.clear();
        }
        "Filter" => {
            app.mode = AppMode::Filter;
            app.filter_stage = crate::app::FilterStage::SelectColumn;
            app.filter_selected_col = app.cursor_col;
            app.filter_selected_op = 0;
            app.filter_input.clear();
        }
        "Clear Filters" => {
            app.push_view_state();
            app.filters.clear();
            app.scroll_offset = 0;
            app.cursor_row = 0;
            let _ = app.refresh_data();
            app.status_message = Some("Filters cleared".to_string());
        }
        "Sort" => {
            let actual_idx = app.visible_columns().get(app.cursor_col).map(|(i, _)| *i);
            if let Some(actual_idx) = actual_idx {
                app.push_view_state();
                if app.sort_column == Some(actual_idx) {
                    app.sort_direction = match &app.sort_direction {
                        crate::app::SortDirection::None => crate::app::SortDirection::Asc,
                        crate::app::SortDirection::Asc => crate::app::SortDirection::Desc,
                        crate::app::SortDirection::Desc => crate::app::SortDirection::None,
                    };
                } else {
                    app.sort_column = Some(actual_idx);
                    app.sort_direction = crate::app::SortDirection::Asc;
                }
                app.scroll_offset = 0;
                app.cursor_row = 0;
                let _ = app.refresh_data();
            }
        }
        "Back to Table" => {
            crate::handlers::input::return_to_base_table(app);
        }
        "AI Query" => {
            crate::handlers::input::enter_ai_or_key_setup(app);
        }
        "SQL Query" => {
            crate::handlers::input::enter_sql_mode(app);
        }
        "Export" => {
            app.mode = AppMode::Export;
            app.export_format_idx = 0;
            app.export_path.clear();
        }
        "Column Stats" => {
            let vis_cols = app.visible_columns();
            if let Some((_, col)) = vis_cols.get(app.cursor_col) {
                match app.engine.get_column_stats(&col.name, app.build_where_clause().as_deref()) {
                    Ok(stats) => {
                        app.stats_data = stats;
                        app.mode = AppMode::ColumnStats;
                    }
                    Err(e) => {
                        app.error_message = Some(format!("Stats error: {}", e));
                    }
                }
            }
        }
        "Edit Cell" => {
            let value = app
                .current_cell_value()
                .unwrap_or("NULL")
                .to_string();
            let value = if value == "NULL" {
                String::new()
            } else {
                value
            };
            app.edit_original = value.clone();
            app.edit_buffer = value;
            app.edit_cursor_pos = app.edit_buffer.len();
            app.mode = AppMode::CellEdit;
        }
        "Cell Detail" => {
            app.mode = AppMode::CellDetail;
        }
        "Toggle Row Numbers" => {
            app.show_row_numbers = !app.show_row_numbers;
        }
        "Cycle Theme" => {
            app.theme = app.theme.next();
            app.status_message = Some(format!("Theme: {}", app.theme.name()));
        }
        "Hide Column" => {
            let vis_info = {
                let vis_cols = app.visible_columns();
                let len = vis_cols.len();
                let idx = vis_cols.get(app.cursor_col).map(|(i, _)| *i);
                (len, idx)
            };
            if vis_info.0 > 1 {
                if let Some(actual_idx) = vis_info.1 {
                    app.hidden_cols[actual_idx] = true;
                    let new_len = app.visible_columns().len();
                    if app.cursor_col >= new_len {
                        app.cursor_col = new_len.saturating_sub(1);
                    }
                }
            }
        }
        "Show All Columns" => {
            for h in app.hidden_cols.iter_mut() {
                *h = false;
            }
            app.status_message = Some("All columns visible".to_string());
        }
        "Sparkline" => {
            let vis_cols = app.visible_columns();
            if let Some((_, col)) = vis_cols.get(app.cursor_col) {
                match app.engine.get_histogram_data(&col.name, app.build_where_clause().as_deref()) {
                    Ok((data, min, max, avg)) => {
                        app.sparkline_data = data;
                        app.sparkline_min = min;
                        app.sparkline_max = max;
                        app.sparkline_avg = avg;
                        app.mode = AppMode::Sparkline;
                    }
                    Err(e) => {
                        app.error_message = Some(format!("{}", e));
                    }
                }
            }
        }
        "Formula Bar" => {
            app.mode = AppMode::FormulaBar;
            app.formula_input.clear();
        }
        "Add Row" => {
            crate::handlers::input::add_row(app);
        }
        "Add Column" => {
            crate::handlers::input::start_new_column(app);
        }
        "Delete Row" => {
            crate::handlers::input::delete_row(app);
        }
        "Board View" => {
            crate::handlers::input::toggle_board(app);
        }
        "Save" => {
            crate::handlers::input::save_table(app);
        }
        "Computed Column" => {
            app.computed_col_stage = ComputedColumnStage::EnterName;
            app.computed_col_name.clear();
            app.computed_col_expr.clear();
            app.mode = AppMode::ComputedColumn;
        }
        "Group By" => {
            app.groupby_stage = GroupByStage::SelectGroupCols;
            app.groupby_selected = vec![false; app.columns.len()];
            app.groupby_cursor = 0;
            app.groupby_agg_col = 0;
            app.groupby_agg_func_idx = 0;
            app.mode = AppMode::GroupBy;
        }
        "Join" => {
            app.join_stage = JoinStage::EnterPath;
            app.join_path.clear();
            app.join_type_idx = 0;
            app.join_col1_idx = 0;
            app.join_col2_idx = 0;
            app.join_data2_cols.clear();
            app.join_active_side = 0;
            app.mode = AppMode::Join;
        }
        "Undo" => {
            match app.pop_view_state() {
                Ok(true) => app.status_message = Some("Undo".to_string()),
                Ok(false) => app.status_message = Some("Nothing to undo".to_string()),
                Err(e) => app.error_message = Some(format!("Undo error: {}", e)),
            }
        }
        "Go to Top" => {
            app.cursor_row = 0;
            app.scroll_offset = 0;
            let _ = app.refresh_data();
        }
        "Go to Bottom" => {
            if app.filtered_rows > 0 {
                app.cursor_row = app.filtered_rows - 1;
                let visible = app.viewport_height.saturating_sub(6) as usize;
                app.scroll_offset = app.cursor_row.saturating_sub(visible) + 1;
                let _ = app.refresh_data();
            }
        }
        "Help" => {
            app.mode = AppMode::Help;
        }
        "Quit" => {
            app.should_quit = true;
        }
        _ => {}
    }
}
