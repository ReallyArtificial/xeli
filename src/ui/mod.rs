pub mod ai_bar;
pub mod command_palette;
pub mod file_picker;
pub mod filter_bar;
pub mod header;
pub mod status;
pub mod table;
pub mod theme;

use crate::app::{App, AppMode, GroupByStage, JoinStage};
use ratatui::Frame;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let colors = theme::get_theme_colors(&app.theme);

    // Layout: header (2 lines) + filter bar (if filters) + table + status (1 line) + input bar (if modal)
    let mut constraints = vec![
        ratatui::layout::Constraint::Length(2), // header
    ];

    // Filter bar when filters exist
    if !app.filters.is_empty() {
        constraints.push(ratatui::layout::Constraint::Length(1));
    }

    // Table fills remaining space
    constraints.push(ratatui::layout::Constraint::Min(5));

    // Status bar (2 lines: mode+message, persistent key hints)
    constraints.push(ratatui::layout::Constraint::Length(2));

    // Input bar for search/filter/ai/sql/formula/computed column modes
    match app.mode {
        AppMode::Search | AppMode::AiQuery | AppMode::AiKeySetup | AppMode::SqlQuery
        | AppMode::Filter | AppMode::FormulaBar | AppMode::ComputedColumn => {
            constraints.push(ratatui::layout::Constraint::Length(1));
        }
        _ => {}
    }

    let layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0;

    // Header
    header::render(f, layout[idx], app, &colors);
    idx += 1;

    // Filter bar
    if !app.filters.is_empty() {
        filter_bar::render(f, layout[idx], app, &colors);
        idx += 1;
    }

    // Main table
    table::render(f, layout[idx], app, &colors);
    idx += 1;

    // Status bar
    status::render(f, layout[idx], app, &colors);
    idx += 1;

    // Input bar
    if idx < layout.len() {
        match app.mode {
            AppMode::Search => {
                ai_bar::render_search(f, layout[idx], app, &colors);
            }
            AppMode::AiQuery => {
                ai_bar::render_ai(f, layout[idx], app, &colors);
            }
            AppMode::AiKeySetup => {
                ai_bar::render_ai_key_setup(f, layout[idx], app, &colors);
            }
            AppMode::SqlQuery => {
                ai_bar::render_sql(f, layout[idx], app, &colors);
            }
            AppMode::Filter => {
                filter_bar::render_input(f, layout[idx], app, &colors);
            }
            AppMode::FormulaBar => {
                ai_bar::render_formula(f, layout[idx], app, &colors);
            }
            AppMode::ComputedColumn => {
                ai_bar::render_computed_column(f, layout[idx], app, &colors);
            }
            _ => {}
        }
    }

    // Overlays
    match app.mode {
        AppMode::Command => {
            command_palette::render(f, app, &colors);
        }
        AppMode::CellDetail => {
            render_cell_detail(f, app, &colors);
        }
        AppMode::ColumnStats => {
            render_column_stats(f, app, &colors);
        }
        AppMode::Export => {
            render_export_dialog(f, app, &colors);
        }
        AppMode::Help => {
            render_help(f, app, &colors);
        }
        AppMode::Sparkline => {
            render_sparkline(f, app, &colors);
        }
        AppMode::GroupBy => {
            render_groupby_wizard(f, app, &colors);
        }
        AppMode::Join => {
            render_join_wizard(f, app, &colors);
        }
        _ => {}
    }
}

fn render_cell_detail(f: &mut Frame, app: &App, colors: &theme::ThemeColors) {
    let area = centered_rect(60, 50, f.area());
    let block = ratatui::widgets::Block::default()
        .title(" Cell Detail ")
        .title_style(ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD))
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(ratatui::style::Style::default().fg(colors.border))
        .style(ratatui::style::Style::default().bg(colors.bg));

    let value = app.current_cell_value().unwrap_or("(empty)");
    let vis_cols = app.visible_columns();
    let col_name = vis_cols
        .get(app.cursor_col)
        .map(|(_, c)| c.name.as_str())
        .unwrap_or("?");

    let text = vec![
        ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("Column: ", ratatui::style::Style::default().fg(colors.muted)),
            ratatui::text::Span::styled(col_name, ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD)),
        ]),
        ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                format!("Row: {}", app.cursor_row + 1),
                ratatui::style::Style::default().fg(colors.muted),
            ),
        ]),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(ratatui::text::Span::styled(value, ratatui::style::Style::default().fg(colors.fg))),
    ];

    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

fn render_column_stats(f: &mut Frame, app: &App, colors: &theme::ThemeColors) {
    let area = centered_rect(50, 60, f.area());
    let vis_cols = app.visible_columns();
    let col_name = vis_cols
        .get(app.cursor_col)
        .map(|(_, c)| c.name.as_str())
        .unwrap_or("?");

    let block = ratatui::widgets::Block::default()
        .title(format!(" Stats: {} ", col_name))
        .title_style(ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD))
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(ratatui::style::Style::default().fg(colors.border))
        .style(ratatui::style::Style::default().bg(colors.bg));

    let rows: Vec<ratatui::widgets::Row> = app
        .stats_data
        .iter()
        .map(|(key, val)| {
            ratatui::widgets::Row::new(vec![
                ratatui::widgets::Cell::from(ratatui::text::Span::styled(
                    key.clone(),
                    ratatui::style::Style::default().fg(colors.muted),
                )),
                ratatui::widgets::Cell::from(ratatui::text::Span::styled(
                    val.clone(),
                    ratatui::style::Style::default().fg(colors.fg),
                )),
            ])
        })
        .collect();

    let table = ratatui::widgets::Table::new(
        rows,
        [ratatui::layout::Constraint::Length(12), ratatui::layout::Constraint::Min(20)],
    )
    .block(block)
    .column_spacing(2);

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(table, area);
}

fn render_export_dialog(f: &mut Frame, app: &App, colors: &theme::ThemeColors) {
    let area = centered_rect(50, 30, f.area());
    let block = ratatui::widgets::Block::default()
        .title(" Export ")
        .title_style(ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD))
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(ratatui::style::Style::default().fg(colors.border))
        .style(ratatui::style::Style::default().bg(colors.bg));

    let formats = ["CSV", "JSON", "Parquet"];
    let mut lines = vec![
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "Select format:",
            ratatui::style::Style::default().fg(colors.muted),
        )),
        ratatui::text::Line::from(""),
    ];

    for (i, fmt) in formats.iter().enumerate() {
        let style = if i == app.export_format_idx {
            ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD)
        } else {
            ratatui::style::Style::default().fg(colors.fg)
        };
        let prefix = if i == app.export_format_idx { " > " } else { "   " };
        lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
            format!("{}{}", prefix, fmt),
            style,
        )));
    }

    if !app.export_path.is_empty() {
        lines.push(ratatui::text::Line::from(""));
        lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
            format!("Path: {}", app.export_path),
            ratatui::style::Style::default().fg(colors.muted),
        )));
    }

    lines.push(ratatui::text::Line::from(""));
    lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
        "Enter to export, Esc to cancel",
        ratatui::style::Style::default().fg(colors.muted),
    )));

    let paragraph = ratatui::widgets::Paragraph::new(lines).block(block);

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

fn render_sparkline(f: &mut Frame, app: &App, colors: &theme::ThemeColors) {
    let area = centered_rect(60, 50, f.area());
    let vis_cols = app.visible_columns();
    let col_name = vis_cols
        .get(app.cursor_col)
        .map(|(_, c)| c.name.as_str())
        .unwrap_or("?");

    let block = ratatui::widgets::Block::default()
        .title(format!(" Sparkline: {} ", col_name))
        .title_style(ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD))
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(ratatui::style::Style::default().fg(colors.border))
        .style(ratatui::style::Style::default().bg(colors.bg));

    let mut lines = vec![
        ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("Min: ", ratatui::style::Style::default().fg(colors.muted)),
            ratatui::text::Span::styled(format!("{:.4}", app.sparkline_min), ratatui::style::Style::default().fg(colors.fg)),
            ratatui::text::Span::styled("  Max: ", ratatui::style::Style::default().fg(colors.muted)),
            ratatui::text::Span::styled(format!("{:.4}", app.sparkline_max), ratatui::style::Style::default().fg(colors.fg)),
            ratatui::text::Span::styled("  Avg: ", ratatui::style::Style::default().fg(colors.muted)),
            ratatui::text::Span::styled(format!("{:.4}", app.sparkline_avg), ratatui::style::Style::default().fg(colors.fg)),
        ]),
        ratatui::text::Line::from(""),
    ];

    let max_count = app.sparkline_data.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
    // Available width for bars (area width minus block borders and label)
    let bar_max_width = (area.width as usize).saturating_sub(22);
    let blocks_chars = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

    for (label, count) in &app.sparkline_data {
        let ratio = *count as f64 / max_count as f64;
        let full_blocks = (ratio * bar_max_width as f64) as usize;
        let frac = (ratio * bar_max_width as f64 * 8.0) as usize % 8;
        let mut bar = "█".repeat(full_blocks);
        if frac > 0 && full_blocks < bar_max_width {
            bar.push(blocks_chars[frac]);
        }
        if bar.is_empty() && *count > 0 {
            bar = "▏".to_string();
        }

        lines.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                format!("{:>12} ", label),
                ratatui::style::Style::default().fg(colors.muted),
            ),
            ratatui::text::Span::styled(
                bar,
                ratatui::style::Style::default().fg(colors.accent),
            ),
            ratatui::text::Span::styled(
                format!(" {}", count),
                ratatui::style::Style::default().fg(colors.fg),
            ),
        ]));
    }

    lines.push(ratatui::text::Line::from(""));
    lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
        "Esc/q/Enter: close",
        ratatui::style::Style::default().fg(colors.muted),
    )));

    let paragraph = ratatui::widgets::Paragraph::new(lines).block(block);

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

fn render_groupby_wizard(f: &mut Frame, app: &App, colors: &theme::ThemeColors) {
    let area = centered_rect(55, 70, f.area());

    let title = match &app.groupby_stage {
        GroupByStage::SelectGroupCols => " Group By: Select Columns ",
        GroupByStage::SelectAggregation => " Group By: Select Aggregation ",
    };

    let block = ratatui::widgets::Block::default()
        .title(title)
        .title_style(ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD))
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(ratatui::style::Style::default().fg(colors.border))
        .style(ratatui::style::Style::default().bg(colors.bg));

    let agg_functions = ["COUNT", "SUM", "AVG", "MIN", "MAX"];
    let max_items = (area.height as usize).saturating_sub(6);

    let mut lines: Vec<ratatui::text::Line> = Vec::new();

    match &app.groupby_stage {
        GroupByStage::SelectGroupCols => {
            for (i, col) in app.columns.iter().enumerate().take(max_items) {
                let checked = app.groupby_selected.get(i).copied().unwrap_or(false);
                let checkbox = if checked { "[x]" } else { "[ ]" };
                let is_cursor = i == app.groupby_cursor;

                let style = if is_cursor {
                    ratatui::style::Style::default().fg(colors.cursor_fg).bg(colors.cursor_bg).add_modifier(ratatui::style::Modifier::BOLD)
                } else {
                    ratatui::style::Style::default().fg(colors.fg)
                };

                lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                    format!("  {} {}", checkbox, col.name),
                    style,
                )));
            }
            lines.push(ratatui::text::Line::from(""));
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "Space:toggle  j/k:move  Enter:next  Esc:cancel",
                ratatui::style::Style::default().fg(colors.muted),
            )));
        }
        GroupByStage::SelectAggregation => {
            // Show summary of selected group cols
            let group_cols: Vec<&str> = app.columns.iter().enumerate()
                .filter(|(i, _)| app.groupby_selected.get(*i).copied().unwrap_or(false))
                .map(|(_, c)| c.name.as_str())
                .collect();
            lines.push(ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("Group by: ", ratatui::style::Style::default().fg(colors.muted)),
                ratatui::text::Span::styled(group_cols.join(", "), ratatui::style::Style::default().fg(colors.accent)),
            ]));
            lines.push(ratatui::text::Line::from(""));

            for (i, col) in app.columns.iter().enumerate().take(max_items.saturating_sub(3)) {
                let is_cursor = i == app.groupby_agg_col;
                let func = if is_cursor {
                    agg_functions[app.groupby_agg_func_idx]
                } else {
                    "..."
                };

                let style = if is_cursor {
                    ratatui::style::Style::default().fg(colors.cursor_fg).bg(colors.cursor_bg).add_modifier(ratatui::style::Modifier::BOLD)
                } else {
                    ratatui::style::Style::default().fg(colors.fg)
                };

                let prefix = if is_cursor { " > " } else { "   " };
                lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                    format!("{}{:<20} [{}]", prefix, col.name, func),
                    style,
                )));
            }
            lines.push(ratatui::text::Line::from(""));
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "h/l:function  j/k:column  Enter:apply  Esc:cancel",
                ratatui::style::Style::default().fg(colors.muted),
            )));
        }
    }

    let paragraph = ratatui::widgets::Paragraph::new(lines).block(block);

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

fn render_join_wizard(f: &mut Frame, app: &App, colors: &theme::ThemeColors) {
    let area = centered_rect(60, 70, f.area());

    let title = match &app.join_stage {
        JoinStage::EnterPath => " Join: Enter File Path ",
        JoinStage::SelectJoinType => " Join: Select Type ",
        JoinStage::SelectColumns => " Join: Select Columns ",
    };

    let block = ratatui::widgets::Block::default()
        .title(title)
        .title_style(ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD))
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(ratatui::style::Style::default().fg(colors.border))
        .style(ratatui::style::Style::default().bg(colors.bg));

    let join_types = ["INNER", "LEFT", "RIGHT", "FULL"];
    let mut lines: Vec<ratatui::text::Line> = Vec::new();

    match &app.join_stage {
        JoinStage::EnterPath => {
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "Enter path to second file:",
                ratatui::style::Style::default().fg(colors.muted),
            )));
            lines.push(ratatui::text::Line::from(""));
            lines.push(ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    " > ",
                    ratatui::style::Style::default().fg(colors.accent),
                ),
                ratatui::text::Span::styled(&app.join_path, ratatui::style::Style::default().fg(colors.fg)),
                ratatui::text::Span::styled("_", ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::SLOW_BLINK)),
            ]));
            lines.push(ratatui::text::Line::from(""));
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "Enter:load file  Esc:cancel",
                ratatui::style::Style::default().fg(colors.muted),
            )));
        }
        JoinStage::SelectJoinType => {
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "Select join type:",
                ratatui::style::Style::default().fg(colors.muted),
            )));
            lines.push(ratatui::text::Line::from(""));

            for (i, jt) in join_types.iter().enumerate() {
                let is_selected = i == app.join_type_idx;
                let style = if is_selected {
                    ratatui::style::Style::default().fg(colors.cursor_fg).bg(colors.cursor_bg).add_modifier(ratatui::style::Modifier::BOLD)
                } else {
                    ratatui::style::Style::default().fg(colors.fg)
                };
                let prefix = if is_selected { " > " } else { "   " };
                lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                    format!("{}{} JOIN", prefix, jt),
                    style,
                )));
            }
            lines.push(ratatui::text::Line::from(""));
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "j/k:select  Enter:next  Shift+Tab:back  Esc:cancel",
                ratatui::style::Style::default().fg(colors.muted),
            )));
        }
        JoinStage::SelectColumns => {
            let max_items = (area.height as usize).saturating_sub(8) / 2;

            // Left side header
            lines.push(ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    "data (left)",
                    if app.join_active_side == 0 {
                        ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD)
                    } else {
                        ratatui::style::Style::default().fg(colors.muted)
                    },
                ),
            ]));

            for (i, col) in app.columns.iter().enumerate().take(max_items) {
                let is_cursor = app.join_active_side == 0 && i == app.join_col1_idx;
                let style = if is_cursor {
                    ratatui::style::Style::default().fg(colors.cursor_fg).bg(colors.cursor_bg).add_modifier(ratatui::style::Modifier::BOLD)
                } else {
                    ratatui::style::Style::default().fg(colors.fg)
                };
                let prefix = if is_cursor { " > " } else { "   " };
                lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                    format!("{}{}", prefix, col.name),
                    style,
                )));
            }

            lines.push(ratatui::text::Line::from(""));

            // Right side header
            lines.push(ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    "data2 (right)",
                    if app.join_active_side == 1 {
                        ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD)
                    } else {
                        ratatui::style::Style::default().fg(colors.muted)
                    },
                ),
            ]));

            for (i, col) in app.join_data2_cols.iter().enumerate().take(max_items) {
                let is_cursor = app.join_active_side == 1 && i == app.join_col2_idx;
                let style = if is_cursor {
                    ratatui::style::Style::default().fg(colors.cursor_fg).bg(colors.cursor_bg).add_modifier(ratatui::style::Modifier::BOLD)
                } else {
                    ratatui::style::Style::default().fg(colors.fg)
                };
                let prefix = if is_cursor { " > " } else { "   " };
                lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                    format!("{}{}", prefix, col.name),
                    style,
                )));
            }

            lines.push(ratatui::text::Line::from(""));
            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "Tab:switch side  j/k:select  Enter:join  Esc:cancel",
                ratatui::style::Style::default().fg(colors.muted),
            )));
        }
    }

    let paragraph = ratatui::widgets::Paragraph::new(lines).block(block);

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

fn render_help(f: &mut Frame, _app: &App, colors: &theme::ThemeColors) {
    let area = centered_rect(70, 80, f.area());
    let block = ratatui::widgets::Block::default()
        .title(" Help — xeli ")
        .title_style(ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD))
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(ratatui::style::Style::default().fg(colors.border))
        .style(ratatui::style::Style::default().bg(colors.bg));

    let help_text = vec![
        ("Navigation", vec![
            ("h/j/k/l or arrows", "Move cursor"),
            ("gg / G", "Go to top / bottom"),
            ("Ctrl+d / Ctrl+u", "Page down / up"),
            ("0 / $", "First / last column"),
            ("Tab / Shift+Tab", "Next / prev column"),
        ]),
        ("Search & Filter", vec![
            ("/ ", "Search (regex)"),
            ("n / N", "Next / prev search match"),
            ("f", "Add filter"),
            ("F", "Clear all filters"),
            ("s", "Sort current column (cycle)"),
        ]),
        ("AI & SQL", vec![
            ("Ctrl+K", "AI natural language query"),
            ("Ctrl+Q", "Direct SQL query — your data is the table 'data'"),
        ]),
        ("View", vec![
            ("Enter", "Cell detail view"),
            ("Ctrl+I", "Column statistics"),
            ("- / +", "Hide / show column"),
            ("H / L", "Resize column narrower / wider"),
            ("r", "Toggle row numbers"),
            ("t", "Cycle theme"),
        ]),
        ("Power Features", vec![
            ("=", "Formula bar (evaluate expression)"),
            ("c", "Add computed column"),
            ("g", "Group-by / pivot wizard"),
            ("J", "Join another file"),
            ("v", "Sparkline chart for column"),
        ]),
        ("Actions", vec![
            ("e", "Export data"),
            ("y", "Copy cell to clipboard"),
            ("u", "Undo (restore previous view)"),
            ("Ctrl+P", "Command palette"),
            ("?", "This help screen"),
            ("q / Esc", "Quit / close dialog"),
        ]),
    ];

    let mut lines = Vec::new();
    for (section, bindings) in &help_text {
        lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
            format!("  {}", section),
            ratatui::style::Style::default().fg(colors.accent).add_modifier(ratatui::style::Modifier::BOLD),
        )));
        for (key, desc) in bindings {
            lines.push(ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    format!("    {:20}", key),
                    ratatui::style::Style::default().fg(colors.yellow),
                ),
                ratatui::text::Span::styled(
                    desc.to_string(),
                    ratatui::style::Style::default().fg(colors.fg),
                ),
            ]));
        }
        lines.push(ratatui::text::Line::from(""));
    }

    let paragraph = ratatui::widgets::Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

pub fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let popup_layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
            ratatui::layout::Constraint::Percentage(percent_y),
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
            ratatui::layout::Constraint::Percentage(percent_x),
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
