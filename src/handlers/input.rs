use crate::app::{App, AiKeyStage, AppMode, CellEditRecord, ComputedColumnStage, Filter, FilterStage, GroupByStage, JoinStage, SortDirection};
use crate::data::export;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tokio::sync::mpsc;
use crate::event::AppEvent;

const FILTER_OPERATORS: &[&str] = &[
    "=", "!=", ">", ">=", "<", "<=", "contains", "starts with", "ends with", "is null",
    "is not null", "regex",
];

const AGG_FUNCTIONS: &[&str] = &["COUNT", "SUM", "AVG", "MIN", "MAX"];
const JOIN_TYPES: &[&str] = &["INNER", "LEFT", "RIGHT", "FULL"];

pub fn handle_key(app: &mut App, key: KeyEvent, ai_tx: &mpsc::UnboundedSender<AppEvent>) {
    // Clear transient messages on any keypress
    app.status_message = None;
    app.error_message = None;

    match &app.mode {
        AppMode::Normal => handle_normal_mode(app, key, ai_tx),
        AppMode::Search => handle_search_mode(app, key),
        AppMode::Filter => handle_filter_mode(app, key),
        AppMode::AiQuery => handle_ai_mode(app, key, ai_tx),
        AppMode::AiKeySetup => handle_ai_key_setup_mode(app, key),
        AppMode::SqlQuery => handle_sql_mode(app, key),
        AppMode::Command => handle_command_mode(app, key, ai_tx),
        AppMode::CellDetail => handle_overlay_mode(app, key),
        AppMode::CellEdit => handle_cell_edit_mode(app, key),
        AppMode::ColumnStats => handle_overlay_mode(app, key),
        AppMode::Export => handle_export_mode(app, key),
        AppMode::Help => handle_overlay_mode(app, key),
        AppMode::Sparkline => handle_overlay_mode(app, key),
        AppMode::FormulaBar => handle_formula_bar_mode(app, key),
        AppMode::ComputedColumn => handle_computed_column_mode(app, key),
        AppMode::GroupBy => handle_groupby_mode(app, key),
        AppMode::Join => handle_join_mode(app, key),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent, _ai_tx: &mpsc::UnboundedSender<AppEvent>) {
    // While viewing an ad-hoc query result (AI/SQL/formula/group-by), return to
    // the base table before doing anything that's defined over `data`.
    if app.viewing_query_result {
        // Esc is the dedicated "back to the full table" key.
        if key.code == KeyCode::Esc {
            return_to_base_table(app);
            return;
        }
        // These ops re-derive from `data`, so snap back first, then let the key
        // fall through to operate on the real table.
        let is_structural = matches!(
            (key.modifiers, key.code),
            (_, KeyCode::Char('s'))
                | (_, KeyCode::Char('f'))
                | (_, KeyCode::Char('F'))
                | (_, KeyCode::Char('g'))
                | (_, KeyCode::Char('c'))
                | (_, KeyCode::Char('='))
                | (_, KeyCode::Char('J'))
                | (_, KeyCode::Char('v'))
                | (KeyModifiers::CONTROL, KeyCode::Char('i'))
        );
        if is_structural {
            return_to_base_table(app);
            // fall through — the match below now runs against the base table
        }
    }

    let vis_col_count = app.visible_columns().len();
    let row_count = app.filtered_rows;

    match (key.modifiers, key.code) {
        // Ctrl combos must come first
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => app.should_quit = true,
        (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
            enter_ai_or_key_setup(app);
        }
        (KeyModifiers::CONTROL, KeyCode::Char('q')) => {
            enter_sql_mode(app);
        }

        // Quit
        (_, KeyCode::Char('q')) => app.should_quit = true,

        // Navigation
        (_, KeyCode::Char('j')) | (_, KeyCode::Down) => {
            if app.cursor_row + 1 < row_count {
                app.cursor_row += 1;
                ensure_cursor_visible(app);
            }
        }
        (_, KeyCode::Char('k')) | (_, KeyCode::Up) => {
            if app.cursor_row > 0 {
                app.cursor_row -= 1;
                ensure_cursor_visible(app);
            }
        }
        (_, KeyCode::Char('h')) | (_, KeyCode::Left) => {
            if app.cursor_col > 0 {
                app.cursor_col -= 1;
                ensure_col_visible(app);
            }
        }
        (_, KeyCode::Char('l')) | (_, KeyCode::Right) => {
            if app.cursor_col + 1 < vis_col_count {
                app.cursor_col += 1;
                ensure_col_visible(app);
            }
        }

        // Fast navigation
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            let page = app.viewport_height.saturating_sub(6) as usize / 2;
            app.cursor_row = (app.cursor_row + page).min(row_count.saturating_sub(1));
            ensure_cursor_visible(app);
        }
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
            let page = app.viewport_height.saturating_sub(6) as usize / 2;
            app.cursor_row = app.cursor_row.saturating_sub(page);
            ensure_cursor_visible(app);
        }
        (_, KeyCode::Char('G')) => {
            if row_count > 0 {
                app.cursor_row = row_count - 1;
                ensure_cursor_visible(app);
            }
        }
        // g -> Group-By wizard
        (_, KeyCode::Char('g')) => {
            app.groupby_stage = GroupByStage::SelectGroupCols;
            app.groupby_selected = vec![false; app.columns.len()];
            app.groupby_cursor = 0;
            app.groupby_agg_col = 0;
            app.groupby_agg_func_idx = 0;
            app.mode = AppMode::GroupBy;
        }
        (_, KeyCode::Char('0')) => {
            app.cursor_col = 0;
            app.col_scroll_offset = 0;
        }
        (_, KeyCode::Char('$')) => {
            if vis_col_count > 0 {
                app.cursor_col = vis_col_count - 1;
                ensure_col_visible(app);
            }
        }
        (_, KeyCode::Tab) => {
            if app.cursor_col + 1 < vis_col_count {
                app.cursor_col += 1;
            } else {
                app.cursor_col = 0;
                app.col_scroll_offset = 0;
            }
            ensure_col_visible(app);
        }
        (KeyModifiers::SHIFT, KeyCode::BackTab) => {
            if app.cursor_col > 0 {
                app.cursor_col -= 1;
            } else if vis_col_count > 0 {
                app.cursor_col = vis_col_count - 1;
            }
            ensure_col_visible(app);
        }

        // Column resize
        (KeyModifiers::SHIFT, KeyCode::Char('H')) => {
            let actual_idx = app.visible_columns().get(app.cursor_col).map(|(i, _)| *i);
            if let Some(idx) = actual_idx {
                let min = app.min_col_width;
                let w = app.col_widths.get_mut(idx).unwrap();
                *w = (*w).saturating_sub(2).max(min);
            }
        }
        (KeyModifiers::SHIFT, KeyCode::Char('L')) => {
            let actual_idx = app.visible_columns().get(app.cursor_col).map(|(i, _)| *i);
            if let Some(idx) = actual_idx {
                let max = app.max_col_width;
                let w = app.col_widths.get_mut(idx).unwrap();
                *w = (*w + 2).min(max);
            }
        }

        // Sort
        (_, KeyCode::Char('s')) => {
            let actual_idx = app.visible_columns().get(app.cursor_col).map(|(i, _)| *i);
            if let Some(actual_idx) = actual_idx {
                app.push_view_state();
                if app.sort_column == Some(actual_idx) {
                    app.sort_direction = match &app.sort_direction {
                        SortDirection::None => SortDirection::Asc,
                        SortDirection::Asc => SortDirection::Desc,
                        SortDirection::Desc => SortDirection::None,
                    };
                    if app.sort_direction == SortDirection::None {
                        app.sort_column = None;
                    }
                } else {
                    app.sort_column = Some(actual_idx);
                    app.sort_direction = SortDirection::Asc;
                }
                app.scroll_offset = 0;
                app.cursor_row = 0;
                let _ = app.refresh_data();
            }
        }

        // Search
        (_, KeyCode::Char('/')) => {
            app.mode = AppMode::Search;
            app.search_query.clear();
            app.search_matches.clear();
        }
        (_, KeyCode::Char('n')) => {
            if !app.search_matches.is_empty() {
                app.search_match_idx = (app.search_match_idx + 1) % app.search_matches.len();
                jump_to_search_match(app);
            }
        }
        (KeyModifiers::SHIFT, KeyCode::Char('N')) => {
            if !app.search_matches.is_empty() {
                app.search_match_idx = if app.search_match_idx == 0 {
                    app.search_matches.len() - 1
                } else {
                    app.search_match_idx - 1
                };
                jump_to_search_match(app);
            }
        }

        // Filter
        (_, KeyCode::Char('f')) => {
            app.mode = AppMode::Filter;
            app.filter_stage = FilterStage::SelectColumn;
            app.filter_selected_col = app.cursor_col;
            app.filter_selected_op = 0;
            app.filter_input.clear();
        }
        (KeyModifiers::SHIFT, KeyCode::Char('F')) => {
            app.push_view_state();
            app.filters.clear();
            app.scroll_offset = 0;
            app.cursor_row = 0;
            let _ = app.refresh_data();
            app.status_message = Some("Filters cleared".to_string());
        }

        // Cell detail
        (_, KeyCode::Enter) => {
            app.mode = AppMode::CellDetail;
        }

        // Column stats
        (KeyModifiers::CONTROL, KeyCode::Char('i')) => {
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

        // Hide/show columns
        (_, KeyCode::Char('-')) => {
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
        (_, KeyCode::Char('+')) => {
            for h in app.hidden_cols.iter_mut() {
                *h = false;
            }
            app.status_message = Some("All columns visible".to_string());
        }
        // Formula bar
        (_, KeyCode::Char('=')) => {
            app.mode = AppMode::FormulaBar;
            app.formula_input.clear();
        }

        // Toggle row numbers
        (_, KeyCode::Char('r')) => {
            app.show_row_numbers = !app.show_row_numbers;
        }

        // Theme
        (_, KeyCode::Char('t')) => {
            app.theme = app.theme.next();
            app.status_message = Some(format!("Theme: {}", app.theme.name()));
        }

        // Export
        (_, KeyCode::Char('e')) => {
            app.mode = AppMode::Export;
            app.export_format_idx = 0;
            app.export_path.clear();
        }

        // Copy cell
        (_, KeyCode::Char('y')) => {
            if let Some(val) = app.current_cell_value() {
                // Try to copy to clipboard via pbcopy/xclip
                let val = val.to_string();
                if copy_to_clipboard(&val).is_ok() {
                    app.status_message = Some("Copied to clipboard".to_string());
                } else {
                    app.status_message = Some(format!("Value: {}", val));
                }
            }
        }

        // Edit cell
        (_, KeyCode::Char('i')) if app.viewing_query_result => {
            // A query result is detached from the base table, so a cell here has
            // no rowid to write back to. Editing would corrupt the wrong row.
            app.status_message =
                Some("Can't edit a query result — press Esc to return to the table".to_string());
        }
        (_, KeyCode::Char('i')) => {
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

        // Undo (also the way out of a query result)
        (_, KeyCode::Char('u')) => {
            if app.viewing_query_result {
                return_to_base_table(app);
            } else {
                match app.pop_view_state() {
                    Ok(true) => app.status_message = Some("Undo".to_string()),
                    Ok(false) => app.status_message = Some("Nothing to undo".to_string()),
                    Err(e) => app.error_message = Some(format!("Undo error: {}", e)),
                }
            }
        }

        // Command palette
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
            app.mode = AppMode::Command;
            app.command_query.clear();
            app.command_selected = 0;
        }

        // Sparkline
        (_, KeyCode::Char('v')) => {
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

        // Computed column
        (_, KeyCode::Char('c')) => {
            app.computed_col_stage = ComputedColumnStage::EnterName;
            app.computed_col_name.clear();
            app.computed_col_expr.clear();
            app.mode = AppMode::ComputedColumn;
        }

        // Join
        (KeyModifiers::SHIFT, KeyCode::Char('J')) => {
            app.join_stage = JoinStage::EnterPath;
            app.join_path.clear();
            app.join_type_idx = 0;
            app.join_col1_idx = 0;
            app.join_col2_idx = 0;
            app.join_data2_cols.clear();
            app.join_active_side = 0;
            app.mode = AppMode::Join;
        }

        // Help
        (_, KeyCode::Char('?')) => {
            app.mode = AppMode::Help;
        }

        _ => {}
    }
}

fn handle_search_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.search_query.clear();
            app.search_matches.clear();
        }
        KeyCode::Enter => {
            execute_search(app);
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            if !app.search_query.is_empty() {
                execute_search(app);
            } else {
                app.search_matches.clear();
            }
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            execute_search(app);
        }
        _ => {}
    }
}

fn handle_filter_mode(app: &mut App, key: KeyEvent) {
    let vis_col_count = app.visible_columns().len();

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        _ => match &app.filter_stage {
            FilterStage::SelectColumn => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if vis_col_count > 0 {
                        app.filter_selected_col = (app.filter_selected_col + 1) % vis_col_count;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    app.filter_selected_col = if app.filter_selected_col == 0 {
                        vis_col_count.saturating_sub(1)
                    } else {
                        app.filter_selected_col - 1
                    };
                }
                KeyCode::Enter | KeyCode::Tab => {
                    app.filter_stage = FilterStage::SelectOperator;
                }
                _ => {}
            },
            FilterStage::SelectOperator => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    app.filter_selected_op = (app.filter_selected_op + 1) % FILTER_OPERATORS.len();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    app.filter_selected_op = if app.filter_selected_op == 0 {
                        FILTER_OPERATORS.len() - 1
                    } else {
                        app.filter_selected_op - 1
                    };
                }
                KeyCode::Enter | KeyCode::Tab => {
                    let op = FILTER_OPERATORS[app.filter_selected_op];
                    if op == "is null" || op == "is not null" {
                        // No value needed
                        apply_filter(app);
                    } else {
                        app.filter_stage = FilterStage::EnterValue;
                    }
                }
                KeyCode::BackTab => {
                    app.filter_stage = FilterStage::SelectColumn;
                }
                _ => {}
            },
            FilterStage::EnterValue => match key.code {
                KeyCode::Enter => {
                    apply_filter(app);
                }
                KeyCode::Backspace => {
                    app.filter_input.pop();
                }
                KeyCode::BackTab => {
                    app.filter_stage = FilterStage::SelectOperator;
                }
                KeyCode::Char(c) => {
                    app.filter_input.push(c);
                }
                _ => {}
            },
        },
    }
}

fn apply_filter(app: &mut App) {
    let vis_cols = app.visible_columns();
    if let Some((_, col)) = vis_cols.get(app.filter_selected_col) {
        let filter = Filter {
            column: col.name.clone(),
            operator: FILTER_OPERATORS[app.filter_selected_op].to_string(),
            value: app.filter_input.clone(),
            enabled: true,
        };

        app.push_view_state();
        app.filters.push(filter);
        app.scroll_offset = 0;
        app.cursor_row = 0;

        match app.refresh_data() {
            Ok(_) => {
                app.status_message = Some(format!("Filter applied ({} rows)", app.filtered_rows));
            }
            Err(e) => {
                // Remove bad filter
                app.filters.pop();
                let _ = app.refresh_data();
                app.error_message = Some(format!("Filter error: {}", e));
            }
        }
    }

    app.mode = AppMode::Normal;
    app.filter_input.clear();
}

/// Enter AI query mode, OR — if no API key is configured — first show the
/// inline two-step key setup (pick provider → paste key → save), then auto-
/// advance into AI query mode.
/// Leave a query result and show the full base `data` table again.
pub fn return_to_base_table(app: &mut App) {
    match app.reset_to_base_table() {
        Ok(()) => app.status_message = Some(format!("Back to {}", app.filename())),
        Err(e) => app.error_message = Some(format!("Couldn't reload table: {}", e)),
    }
}

pub fn enter_sql_mode(app: &mut App) {
    app.mode = AppMode::SqlQuery;
    app.sql_history_idx = None;
    app.error_message = None;
    // Pre-fill a working query so Enter does something useful immediately and
    // the user discovers the table name (`data`) without guessing.
    if app.sql_input.is_empty() {
        app.sql_input = "SELECT * FROM data".to_string();
    }
}

pub fn enter_ai_or_key_setup(app: &mut App) {
    let config = crate::ai::config::AiConfig::load();
    let has_any_key = config.openai_api_key.is_some() || config.anthropic_api_key.is_some();

    if has_any_key {
        app.mode = AppMode::AiQuery;
        app.input_buffer.clear();
        app.ai_response.clear();
        app.ai_sql.clear();
    } else {
        app.mode = AppMode::AiKeySetup;
        app.ai_key_stage = AiKeyStage::PickProvider;
        app.ai_key_provider_idx = 0; // default Anthropic
        app.ai_key_input.clear();
    }
}

fn handle_ai_key_setup_mode(app: &mut App, key: KeyEvent) {
    match &app.ai_key_stage {
        AiKeyStage::PickProvider => match key.code {
            KeyCode::Esc => {
                app.mode = AppMode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('k') | KeyCode::Up => {
                app.ai_key_provider_idx = 1 - app.ai_key_provider_idx;
            }
            KeyCode::Enter | KeyCode::Tab => {
                app.ai_key_stage = AiKeyStage::EnterKey;
                app.ai_key_input.clear();
            }
            _ => {}
        },
        AiKeyStage::EnterKey => match key.code {
            KeyCode::Esc => {
                app.mode = AppMode::Normal;
                app.ai_key_input.clear();
            }
            KeyCode::BackTab => {
                app.ai_key_stage = AiKeyStage::PickProvider;
            }
            KeyCode::Backspace => {
                app.ai_key_input.pop();
            }
            KeyCode::Char(c) => {
                app.ai_key_input.push(c);
            }
            KeyCode::Enter => {
                let key_str = app.ai_key_input.trim().to_string();
                if key_str.is_empty() {
                    app.error_message = Some("Key is empty".to_string());
                    return;
                }
                let provider = if app.ai_key_provider_idx == 0 { "anthropic" } else { "openai" };
                let mut config = crate::ai::config::AiConfig::load();
                match provider {
                    "anthropic" => {
                        config.anthropic_api_key = Some(key_str);
                        config.provider = "anthropic".to_string();
                    }
                    _ => {
                        config.openai_api_key = Some(key_str);
                        config.provider = "openai".to_string();
                    }
                }
                match config.save() {
                    Ok(_) => {
                        app.status_message = Some(format!("Saved {} key. Ask away!", provider));
                        app.ai_key_input.clear();
                        // Auto-advance into AI query so user can type their question.
                        app.mode = AppMode::AiQuery;
                        app.input_buffer.clear();
                        app.ai_response.clear();
                        app.ai_sql.clear();
                    }
                    Err(e) => {
                        app.error_message = Some(format!("Could not save key: {}", e));
                    }
                }
            }
            _ => {}
        },
    }
}

fn handle_ai_mode(app: &mut App, key: KeyEvent, ai_tx: &mpsc::UnboundedSender<AppEvent>) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.ai_loading = false;
        }
        KeyCode::Enter => {
            if !app.input_buffer.is_empty() && !app.ai_loading {
                let query = app.input_buffer.clone();
                app.ai_loading = true;

                // Build schema info for AI
                let schema: Vec<String> = app
                    .columns
                    .iter()
                    .map(|c| format!("{} ({})", c.name, c.data_type))
                    .collect();

                let sample_values: Vec<String> = app
                    .columns
                    .iter()
                    .filter_map(|c| {
                        app.engine
                            .get_sample_values(&c.name, 3)
                            .ok()
                            .map(|vals| format!("{}: {}", c.name, vals.join(", ")))
                    })
                    .collect();

                let tx = ai_tx.clone();
                let prompt = crate::ai::prompt::build_prompt(&query, &schema, &sample_values);
                let config = crate::ai::config::AiConfig::load();

                tokio::spawn(async move {
                    match crate::ai::client::query_ai(&config, &prompt).await {
                        Ok(sql) => {
                            let _ = tx.send(AppEvent::AiResponse(sql));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::AiError(e.to_string()));
                        }
                    }
                });
            }
        }
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
}

fn handle_sql_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            if !app.sql_input.is_empty() {
                let sql = app.sql_input.clone();
                app.sql_history.push(sql.clone());
                app.sql_history_idx = None;

                match app.engine.execute_query(&sql) {
                    Ok(result) => {
                        app.push_view_state();
                        app.viewing_query_result = true;
                        app.result_sql = Some(sql.clone());
                        app.columns = result.columns;
                        app.rows = result.rows;
                        app.total_rows = result.total_rows;
                        app.filtered_rows = result.total_rows;
                        app.cursor_row = 0;
                        app.cursor_col = 0;
                        app.scroll_offset = 0;
                        app.col_scroll_offset = 0;
                        app.col_widths = vec![15u16; app.columns.len()];
                        app.hidden_cols = vec![false; app.columns.len()];
                        app.auto_size_columns();
                        app.status_message = Some(format!("Query returned {} rows", result.total_rows));
                        app.mode = AppMode::Normal;
                    }
                    Err(e) => {
                        app.error_message = Some(clean_sql_error(&e.to_string(), &app.data_columns));
                        // Stay in SQL mode so the user can fix the query in place.
                    }
                }
            }
        }
        KeyCode::Up => {
            if !app.sql_history.is_empty() {
                let idx = match app.sql_history_idx {
                    Some(i) => i.saturating_sub(1),
                    None => app.sql_history.len() - 1,
                };
                app.sql_history_idx = Some(idx);
                app.sql_input = app.sql_history[idx].clone();
            }
        }
        KeyCode::Down => {
            if let Some(idx) = app.sql_history_idx {
                if idx + 1 < app.sql_history.len() {
                    app.sql_history_idx = Some(idx + 1);
                    app.sql_input = app.sql_history[idx + 1].clone();
                } else {
                    app.sql_history_idx = None;
                    app.sql_input.clear();
                }
            }
        }
        KeyCode::Backspace => {
            app.sql_input.pop();
        }
        KeyCode::Char(c) => {
            app.sql_input.push(c);
        }
        _ => {}
    }
}

/// DuckDB binder/catalog errors are verbose (they append `Candidate bindings:`
/// and `LINE n:` noise that overflows the one-line status bar). Trim to the
/// essential message and, for the common "wrong table/column" mistakes, point
/// the user at the real table name and columns.
fn clean_sql_error(raw: &str, data_columns: &[String]) -> String {
    // Cut the trailing noise DuckDB appends.
    let mut msg = raw;
    for marker in ["\nLINE ", "LINE ", "\nCandidate bindings", "Candidate bindings"] {
        if let Some(i) = msg.find(marker) {
            msg = &msg[..i];
        }
    }
    let msg = msg.trim().trim_end_matches('!').trim();

    let lower = msg.to_lowercase();
    let mentions_table = lower.contains("does not exist")
        || lower.contains("not found in from clause")
        || lower.contains("catalog error")
        || lower.contains("table with name");

    if mentions_table {
        let cols = preview_columns(data_columns);
        format!("{}  →  query the table named `data` (columns: {})", msg, cols)
    } else {
        format!("SQL error: {}", msg)
    }
}

/// A short, comma-separated preview of column names for hints/errors.
fn preview_columns(cols: &[String]) -> String {
    const MAX: usize = 8;
    if cols.len() > MAX {
        format!("{}, … (+{} more)", cols[..MAX].join(", "), cols.len() - MAX)
    } else {
        cols.join(", ")
    }
}

fn handle_command_mode(app: &mut App, key: KeyEvent, ai_tx: &mpsc::UnboundedSender<AppEvent>) {
    let commands = crate::ui::command_palette::filtered_commands(&app.command_query);

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            if let Some(cmd) = commands.get(app.command_selected) {
                app.mode = AppMode::Normal;
                crate::handlers::command::execute_command(app, cmd.name, ai_tx);
            }
        }
        KeyCode::Up => {
            if app.command_selected > 0 {
                app.command_selected -= 1;
            }
        }
        KeyCode::Down => {
            if app.command_selected + 1 < commands.len() {
                app.command_selected += 1;
            }
        }
        KeyCode::Backspace => {
            app.command_query.pop();
            app.command_selected = 0;
        }
        KeyCode::Char(c) => {
            app.command_query.push(c);
            app.command_selected = 0;
        }
        _ => {}
    }
}

fn handle_overlay_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

fn handle_cell_edit_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            // Cancel edit, restore original
            app.edit_buffer.clear();
            app.edit_original.clear();
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            // Commit the edit
            let vis_cols = app.visible_columns();
            let col_info = vis_cols.get(app.cursor_col).map(|(_, col)| col.name.clone());

            if let Some(col_name) = col_info {
                let order_by = app.build_order_by();
                let where_clause = app.build_where_clause();

                match app.engine.get_rowid(
                    app.cursor_row,
                    order_by.as_deref(),
                    where_clause.as_deref(),
                ) {
                    Ok(rowid) => {
                        let new_value = app.edit_buffer.clone();
                        match app.engine.update_cell(rowid, &col_name, &new_value) {
                            Ok(_) => {
                                app.edit_history.push(CellEditRecord {
                                    row: app.cursor_row,
                                    col_name,
                                    old_value: app.edit_original.clone(),
                                    new_value,
                                });
                                let _ = app.refresh_data();
                                app.status_message = Some("Cell updated".to_string());
                            }
                            Err(e) => {
                                app.error_message =
                                    Some(format!("Edit error: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        app.error_message =
                            Some(format!("Row lookup error: {}", e));
                    }
                }
            }

            app.edit_buffer.clear();
            app.edit_original.clear();
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            if app.edit_cursor_pos > 0 {
                let byte_pos = app
                    .edit_buffer
                    .char_indices()
                    .nth(app.edit_cursor_pos - 1)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let next_byte = app
                    .edit_buffer
                    .char_indices()
                    .nth(app.edit_cursor_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(app.edit_buffer.len());
                app.edit_buffer.replace_range(byte_pos..next_byte, "");
                app.edit_cursor_pos -= 1;
            }
        }
        KeyCode::Delete => {
            let char_count = app.edit_buffer.chars().count();
            if app.edit_cursor_pos < char_count {
                let byte_pos = app
                    .edit_buffer
                    .char_indices()
                    .nth(app.edit_cursor_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(app.edit_buffer.len());
                let next_byte = app
                    .edit_buffer
                    .char_indices()
                    .nth(app.edit_cursor_pos + 1)
                    .map(|(i, _)| i)
                    .unwrap_or(app.edit_buffer.len());
                app.edit_buffer.replace_range(byte_pos..next_byte, "");
            }
        }
        KeyCode::Left => {
            if app.edit_cursor_pos > 0 {
                app.edit_cursor_pos -= 1;
            }
        }
        KeyCode::Right => {
            let char_count = app.edit_buffer.chars().count();
            if app.edit_cursor_pos < char_count {
                app.edit_cursor_pos += 1;
            }
        }
        KeyCode::Home => {
            app.edit_cursor_pos = 0;
        }
        KeyCode::End => {
            app.edit_cursor_pos = app.edit_buffer.chars().count();
        }
        KeyCode::Char(c) => {
            let byte_pos = app
                .edit_buffer
                .char_indices()
                .nth(app.edit_cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(app.edit_buffer.len());
            app.edit_buffer.insert(byte_pos, c);
            app.edit_cursor_pos += 1;
        }
        _ => {}
    }
}

fn handle_export_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.export_format_idx = (app.export_format_idx + 1) % 3;
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.export_format_idx = if app.export_format_idx == 0 { 2 } else { app.export_format_idx - 1 };
        }
        KeyCode::Enter => {
            let base_name = std::path::Path::new(&app.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");

            let ext = match app.export_format_idx {
                1 => "json",
                2 => "parquet",
                _ => "csv",
            };
            let path = format!("{}_export.{}", base_name, ext);

            // Export what's on screen: the query result if one is showing,
            // otherwise the (filtered/sorted) base table.
            let result = if let Some(sql) = app.result_sql.clone() {
                export::export_result(&app.engine, &path, app.export_format_idx, &sql)
            } else {
                let export_fn: fn(&crate::data::engine::DataEngine, &str, Option<&str>, Option<&str>) -> anyhow::Result<()> =
                    match app.export_format_idx {
                        1 => export::export_json,
                        2 => export::export_parquet,
                        _ => export::export_csv,
                    };
                let where_clause = app.build_where_clause();
                let order_by = app.build_order_by();
                export_fn(&app.engine, &path, where_clause.as_deref(), order_by.as_deref())
            };

            match result {
                Ok(_) => {
                    app.export_path = path.clone();
                    app.status_message = Some(format!("Exported to {}", path));
                }
                Err(e) => {
                    app.error_message = Some(format!("Export error: {}", e));
                }
            }
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

fn handle_formula_bar_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            if !app.formula_input.is_empty() {
                match app.engine.evaluate_expression(&app.formula_input) {
                    Ok(result) => {
                        app.push_view_state();
                        app.viewing_query_result = true;
                        app.result_sql =
                            Some(crate::data::engine::DataEngine::expression_sql(&app.formula_input));
                        let row_count = result.total_rows;
                        app.columns = result.columns;
                        app.rows = result.rows;
                        app.total_rows = row_count;
                        app.filtered_rows = row_count;
                        app.cursor_row = 0;
                        app.cursor_col = 0;
                        app.scroll_offset = 0;
                        app.col_scroll_offset = 0;
                        app.col_widths = vec![15u16; app.columns.len()];
                        app.hidden_cols = vec![false; app.columns.len()];
                        app.auto_size_columns();
                        app.status_message = Some(format!("Formula: {} rows", row_count));
                        app.mode = AppMode::Normal;
                    }
                    Err(e) => {
                        app.error_message = Some(format!("Formula error: {}", e));
                    }
                }
            }
        }
        KeyCode::Backspace => {
            app.formula_input.pop();
        }
        KeyCode::Char(c) => {
            app.formula_input.push(c);
        }
        _ => {}
    }
}

fn handle_computed_column_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        _ => match &app.computed_col_stage {
            ComputedColumnStage::EnterName => match key.code {
                KeyCode::Enter | KeyCode::Tab => {
                    if !app.computed_col_name.is_empty() {
                        app.computed_col_stage = ComputedColumnStage::EnterExpression;
                    }
                }
                KeyCode::Backspace => {
                    app.computed_col_name.pop();
                }
                KeyCode::Char(c) => {
                    app.computed_col_name.push(c);
                }
                _ => {}
            },
            ComputedColumnStage::EnterExpression => match key.code {
                KeyCode::Enter => {
                    if !app.computed_col_expr.is_empty() {
                        let name = app.computed_col_name.clone();
                        let expr = app.computed_col_expr.clone();
                        match app.engine.add_computed_column(&name, &expr) {
                            Ok(_) => {
                                // Refresh schema and data
                                match app.engine.get_schema() {
                                    Ok(new_cols) => {
                                        app.columns = new_cols;
                                        app.data_columns = app.columns.iter().map(|c| c.name.clone()).collect();
                                        app.col_widths = vec![15u16; app.columns.len()];
                                        app.hidden_cols = vec![false; app.columns.len()];
                                        app.total_rows = app.engine.get_total_rows().unwrap_or(app.total_rows);
                                        let _ = app.refresh_data();
                                        app.auto_size_columns();
                                        app.status_message = Some(format!("Added column '{}'", name));
                                    }
                                    Err(e) => {
                                        app.error_message = Some(format!("Schema refresh error: {}", e));
                                    }
                                }
                                app.mode = AppMode::Normal;
                            }
                            Err(e) => {
                                app.error_message = Some(format!("Computed column error: {}", e));
                            }
                        }
                    }
                }
                KeyCode::BackTab => {
                    app.computed_col_stage = ComputedColumnStage::EnterName;
                }
                KeyCode::Backspace => {
                    app.computed_col_expr.pop();
                }
                KeyCode::Char(c) => {
                    app.computed_col_expr.push(c);
                }
                _ => {}
            },
        },
    }
}

fn handle_groupby_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        _ => match &app.groupby_stage {
            GroupByStage::SelectGroupCols => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if !app.columns.is_empty() {
                        app.groupby_cursor = (app.groupby_cursor + 1) % app.columns.len();
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if !app.columns.is_empty() {
                        app.groupby_cursor = if app.groupby_cursor == 0 {
                            app.columns.len() - 1
                        } else {
                            app.groupby_cursor - 1
                        };
                    }
                }
                KeyCode::Char(' ') => {
                    if let Some(sel) = app.groupby_selected.get_mut(app.groupby_cursor) {
                        *sel = !*sel;
                    }
                }
                KeyCode::Enter => {
                    if app.groupby_selected.iter().any(|&s| s) {
                        app.groupby_stage = GroupByStage::SelectAggregation;
                        app.groupby_agg_col = 0;
                        app.groupby_agg_func_idx = 0;
                    }
                }
                _ => {}
            },
            GroupByStage::SelectAggregation => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if !app.columns.is_empty() {
                        app.groupby_agg_col = (app.groupby_agg_col + 1) % app.columns.len();
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if !app.columns.is_empty() {
                        app.groupby_agg_col = if app.groupby_agg_col == 0 {
                            app.columns.len() - 1
                        } else {
                            app.groupby_agg_col - 1
                        };
                    }
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    app.groupby_agg_func_idx = (app.groupby_agg_func_idx + 1) % AGG_FUNCTIONS.len();
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    app.groupby_agg_func_idx = if app.groupby_agg_func_idx == 0 {
                        AGG_FUNCTIONS.len() - 1
                    } else {
                        app.groupby_agg_func_idx - 1
                    };
                }
                KeyCode::Enter => {
                    // Build group-by SQL
                    let group_cols: Vec<String> = app.columns.iter().enumerate()
                        .filter(|(i, _)| app.groupby_selected.get(*i).copied().unwrap_or(false))
                        .map(|(_, c)| format!("\"{}\"", c.name.replace('"', "\"\"")))
                        .collect();
                    let agg_func = AGG_FUNCTIONS[app.groupby_agg_func_idx];
                    let agg_col_name = &app.columns[app.groupby_agg_col].name;
                    let safe_agg_col = format!("\"{}\"", agg_col_name.replace('"', "\"\""));

                    let sql = format!(
                        "SELECT {group_cols}, {func}({agg_col}) AS \"{func}_{col_name}\" FROM data GROUP BY {group_cols} ORDER BY {group_cols}",
                        group_cols = group_cols.join(", "),
                        func = agg_func,
                        agg_col = safe_agg_col,
                        col_name = agg_col_name.replace('"', "\"\""),
                    );

                    match app.engine.execute_query(&sql) {
                        Ok(result) => {
                            app.push_view_state();
                            app.viewing_query_result = true;
                            app.result_sql = Some(sql.clone());
                            let row_count = result.total_rows;
                            app.columns = result.columns;
                            app.rows = result.rows;
                            app.total_rows = row_count;
                            app.filtered_rows = row_count;
                            app.cursor_row = 0;
                            app.cursor_col = 0;
                            app.scroll_offset = 0;
                            app.col_scroll_offset = 0;
                            app.col_widths = vec![15u16; app.columns.len()];
                            app.hidden_cols = vec![false; app.columns.len()];
                            app.auto_size_columns();
                            app.status_message = Some(format!("Grouped: {} rows", row_count));
                            app.mode = AppMode::Normal;
                        }
                        Err(e) => {
                            app.error_message = Some(format!("Group-by error: {}", e));
                        }
                    }
                }
                KeyCode::BackTab => {
                    app.groupby_stage = GroupByStage::SelectGroupCols;
                }
                _ => {}
            },
        },
    }
}

fn handle_join_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            let _ = app.engine.execute_raw("DROP TABLE IF EXISTS data2");
            app.mode = AppMode::Normal;
        }
        _ => match &app.join_stage {
            JoinStage::EnterPath => match key.code {
                KeyCode::Enter => {
                    if !app.join_path.is_empty() {
                        let path = app.join_path.clone();
                        // Detect format
                        match crate::data::loader::detect_format(&path) {
                            Ok(format) => {
                                match app.engine.load_as_table(&path, format.as_str(), "data2") {
                                    Ok(_) => {
                                        match app.engine.get_table_schema("data2") {
                                            Ok(cols) => {
                                                app.join_data2_cols = cols;
                                                app.join_type_idx = 0;
                                                app.join_stage = JoinStage::SelectJoinType;
                                            }
                                            Err(e) => {
                                                app.error_message = Some(format!("Schema error: {}", e));
                                                let _ = app.engine.execute_raw("DROP TABLE IF EXISTS data2");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        app.error_message = Some(format!("Load error: {}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                app.error_message = Some(format!("Format error: {}", e));
                            }
                        }
                    }
                }
                KeyCode::Backspace => {
                    app.join_path.pop();
                }
                KeyCode::Char(c) => {
                    app.join_path.push(c);
                }
                _ => {}
            },
            JoinStage::SelectJoinType => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    app.join_type_idx = (app.join_type_idx + 1) % JOIN_TYPES.len();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    app.join_type_idx = if app.join_type_idx == 0 {
                        JOIN_TYPES.len() - 1
                    } else {
                        app.join_type_idx - 1
                    };
                }
                KeyCode::Enter => {
                    app.join_col1_idx = 0;
                    app.join_col2_idx = 0;
                    app.join_active_side = 0;
                    app.join_stage = JoinStage::SelectColumns;
                }
                KeyCode::BackTab => {
                    app.join_stage = JoinStage::EnterPath;
                }
                _ => {}
            },
            JoinStage::SelectColumns => match key.code {
                KeyCode::Tab => {
                    app.join_active_side = if app.join_active_side == 0 { 1 } else { 0 };
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if app.join_active_side == 0 {
                        if !app.columns.is_empty() {
                            app.join_col1_idx = (app.join_col1_idx + 1) % app.columns.len();
                        }
                    } else if !app.join_data2_cols.is_empty() {
                        app.join_col2_idx = (app.join_col2_idx + 1) % app.join_data2_cols.len();
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if app.join_active_side == 0 {
                        if !app.columns.is_empty() {
                            app.join_col1_idx = if app.join_col1_idx == 0 {
                                app.columns.len() - 1
                            } else {
                                app.join_col1_idx - 1
                            };
                        }
                    } else if !app.join_data2_cols.is_empty() {
                        app.join_col2_idx = if app.join_col2_idx == 0 {
                            app.join_data2_cols.len() - 1
                        } else {
                            app.join_col2_idx - 1
                        };
                    }
                }
                KeyCode::Enter => {
                    let join_type = JOIN_TYPES[app.join_type_idx];
                    let col1 = &app.columns[app.join_col1_idx].name.clone();
                    let col2 = &app.join_data2_cols[app.join_col2_idx].name.clone();

                    match app.engine.execute_join(join_type, col1, col2) {
                        Ok(_) => {
                            match app.engine.get_schema() {
                                Ok(new_cols) => {
                                    let total = app.engine.get_total_rows().unwrap_or(0);
                                    let col_count = new_cols.len();
                                    app.columns = new_cols;
                                    app.data_columns = app.columns.iter().map(|c| c.name.clone()).collect();
                                    app.total_rows = total;
                                    app.filtered_rows = total;
                                    app.cursor_row = 0;
                                    app.cursor_col = 0;
                                    app.scroll_offset = 0;
                                    app.col_scroll_offset = 0;
                                    app.col_widths = vec![15u16; col_count];
                                    app.hidden_cols = vec![false; col_count];
                                    app.sort_column = None;
                                    app.sort_direction = SortDirection::None;
                                    app.filters.clear();
                                    let _ = app.refresh_data();
                                    app.auto_size_columns();
                                    app.status_message = Some(format!("Joined: {} rows x {} cols", total, col_count));
                                    app.mode = AppMode::Normal;
                                }
                                Err(e) => {
                                    app.error_message = Some(format!("Schema error: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            app.error_message = Some(format!("Join error: {}", e));
                        }
                    }
                }
                KeyCode::BackTab => {
                    app.join_stage = JoinStage::SelectJoinType;
                }
                _ => {}
            },
        },
    }
}

pub fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollDown => {
            if app.cursor_row + 3 < app.filtered_rows {
                app.cursor_row += 3;
                ensure_cursor_visible(app);
            }
        }
        MouseEventKind::ScrollUp => {
            app.cursor_row = app.cursor_row.saturating_sub(3);
            ensure_cursor_visible(app);
        }
        MouseEventKind::Down(_) => {
            // Map a click to a data row. Screen layout above the first data row:
            // header (2) + filter bar (1, only when filters exist) + table top
            // border (1) + table header (1).
            let filter_rows = if app.filters.is_empty() { 0 } else { 1 };
            let first_data_row = 2 + filter_rows + 1 + 1;
            if (mouse.row as usize) >= first_data_row {
                let clicked_row = (mouse.row as usize) - first_data_row + app.scroll_offset;
                if clicked_row < app.filtered_rows {
                    app.cursor_row = clicked_row;
                    ensure_cursor_visible(app);
                }
            }
        }
        _ => {}
    }
}

pub fn handle_ai_response(app: &mut App, sql: String) {
    app.ai_loading = false;
    app.ai_sql = sql.clone();

    // Clean up the SQL
    let sql = sql
        .trim()
        .trim_start_matches("```sql")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    app.ai_response = sql.clone();

    match app.engine.execute_query(&sql) {
        Ok(result) => {
            app.push_view_state();
            app.viewing_query_result = true;
            app.result_sql = Some(sql.clone());
            app.columns = result.columns;
            app.rows = result.rows;
            app.total_rows = result.total_rows;
            app.filtered_rows = result.total_rows;
            app.cursor_row = 0;
            app.cursor_col = 0;
            app.scroll_offset = 0;
            app.col_scroll_offset = 0;
            app.col_widths = vec![15u16; app.columns.len()];
            app.hidden_cols = vec![false; app.columns.len()];
            app.auto_size_columns();
            app.status_message = Some(format!("AI query: {} rows", result.total_rows));
            app.mode = AppMode::Normal;
        }
        Err(e) => {
            app.error_message = Some(format!("AI SQL error: {}", e));
            app.mode = AppMode::Normal;
        }
    }
}

pub fn handle_ai_error(app: &mut App, error: String) {
    app.ai_loading = false;
    app.error_message = Some(format!("AI error: {}", error));
    app.mode = AppMode::Normal;
}

fn ensure_cursor_visible(app: &mut App) {
    let visible_rows = app.viewport_height.saturating_sub(6) as usize;

    if app.cursor_row < app.scroll_offset {
        app.scroll_offset = app.cursor_row;
        let _ = app.refresh_data();
    } else if app.cursor_row >= app.scroll_offset + visible_rows {
        app.scroll_offset = app.cursor_row.saturating_sub(visible_rows) + 1;
        let _ = app.refresh_data();
    }
}

fn ensure_col_visible(app: &mut App) {
    if app.cursor_col < app.col_scroll_offset {
        app.col_scroll_offset = app.cursor_col;
    }
    // Rough estimate — will be refined by the render step
    if app.cursor_col > app.col_scroll_offset + 8 {
        app.col_scroll_offset = app.cursor_col.saturating_sub(4);
    }
}

fn execute_search(app: &mut App) {
    app.search_matches.clear();
    app.search_match_idx = 0;

    if app.search_query.is_empty() {
        return;
    }

    // Build a valid pattern: use the query as a regex, falling back to an escaped
    // literal if it doesn't compile (so a stray `(` still searches literally).
    let (re, pattern) = match regex::Regex::new(&app.search_query) {
        Ok(r) => (r, app.search_query.clone()),
        Err(_) => {
            let escaped = regex::escape(&app.search_query);
            match regex::Regex::new(&escaped) {
                Ok(r) => (r, escaped),
                Err(_) => return,
            }
        }
    };

    if app.viewing_query_result {
        // The result lives whole in `rows` (absolute indices), so scan it here.
        for (row_idx, row_data) in app.rows.iter().enumerate() {
            for (col_idx, val) in row_data.iter().enumerate() {
                if re.is_match(val) {
                    app.search_matches.push((row_idx, col_idx));
                }
            }
        }
    } else {
        // Base table: only a page is loaded, so search the full table server-side.
        let where_clause = app.build_where_clause();
        let order_by = app.build_order_by();
        let col_names: Vec<String> = app.columns.iter().map(|c| c.name.clone()).collect();
        match app.engine.find_search_matches(
            &pattern,
            where_clause.as_deref(),
            order_by.as_deref(),
            &col_names,
        ) {
            Ok(matches) => app.search_matches = matches,
            Err(e) => {
                app.error_message = Some(format!("Search error: {}", e));
                return;
            }
        }
    }

    if !app.search_matches.is_empty() {
        jump_to_search_match(app);
    }
}

fn jump_to_search_match(app: &mut App) {
    if let Some(&(row, _col)) = app.search_matches.get(app.search_match_idx) {
        app.cursor_row = row;
        ensure_cursor_visible(app);
    }
}

fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    // Try pbcopy (macOS), then xclip, then xsel
    let commands = [
        ("pbcopy", vec![]),
        ("xclip", vec!["-selection", "clipboard"]),
        ("xsel", vec!["--clipboard", "--input"]),
    ];

    for (cmd, args) in &commands {
        if let Ok(mut child) = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(text.as_bytes())?;
            }
            child.wait()?;
            return Ok(());
        }
    }

    anyhow::bail!("No clipboard tool available")
}

#[cfg(test)]
mod sql_error_tests {
    use super::{clean_sql_error, preview_columns};

    #[test]
    fn trims_binder_noise_and_points_at_data_table() {
        let cols = vec!["product".to_string(), "category".to_string(), "price".to_string()];
        let raw = "Binder Error: Referenced column \"category\" not found in FROM clause!Candidate bindings: \"tableowner\", \"hastriggers\"LINE 1: ...";
        let cleaned = clean_sql_error(raw, &cols);

        assert!(!cleaned.contains("Candidate bindings"), "noise leaked: {cleaned}");
        assert!(!cleaned.contains("LINE 1"), "LINE noise leaked: {cleaned}");
        assert!(cleaned.contains("`data`"), "missing table hint: {cleaned}");
        assert!(cleaned.contains("product, category, price"), "missing column hint: {cleaned}");
    }

    #[test]
    fn passes_through_non_table_errors_concisely() {
        let cols = vec!["a".to_string()];
        let cleaned = clean_sql_error("Parser Error: syntax error at or near \"slect\"", &cols);
        assert!(cleaned.starts_with("SQL error:"), "got: {cleaned}");
        assert!(!cleaned.contains("`data`"), "should not add table hint: {cleaned}");
    }

    #[test]
    fn previews_cap_long_column_lists() {
        let cols: Vec<String> = (0..12).map(|i| format!("c{i}")).collect();
        let p = preview_columns(&cols);
        assert!(p.contains("+4 more"), "got: {p}");
    }
}
