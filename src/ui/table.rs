use crate::app::{App, AppMode, SortDirection};
use crate::ui::theme::ThemeColors;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};
use ratatui::Frame;

pub fn render(f: &mut Frame, area: Rect, app: &App, colors: &ThemeColors) {
    let vis_cols = app.visible_columns();

    if vis_cols.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(colors.border))
            .style(Style::default().bg(colors.bg));
        let msg = ratatui::widgets::Paragraph::new("  No columns to display")
            .style(Style::default().fg(colors.muted))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    // Determine row number column width
    let row_num_width = if app.show_row_numbers {
        format!("{}", app.total_rows).len().max(3) as u16 + 2
    } else {
        0
    };

    // Calculate which columns fit in the viewport
    let available_width = area.width.saturating_sub(2).saturating_sub(row_num_width); // -2 for borders
    let visible_range_start = app.col_scroll_offset;
    let mut total_width = 0u16;
    let mut visible_range_end = vis_cols.len();

    for (vi, (actual_idx, _)) in vis_cols.iter().enumerate() {
        if vi < visible_range_start {
            continue;
        }
        let w = app.col_widths.get(*actual_idx).copied().unwrap_or(15);
        if total_width + w + 1 > available_width && vi > visible_range_start {
            visible_range_end = vi;
            break;
        }
        total_width += w + 1; // +1 for column spacing
    }

    // Build constraints
    let mut constraints: Vec<Constraint> = Vec::new();
    let mut header_cells: Vec<Cell> = Vec::new();

    if app.show_row_numbers {
        constraints.push(Constraint::Length(row_num_width));
        header_cells.push(Cell::from(Span::styled(
            format!("{:>width$}", "#", width = row_num_width as usize - 1),
            Style::default()
                .fg(colors.muted)
                .add_modifier(Modifier::DIM),
        )));
    }

    for (vi, (actual_idx, col)) in vis_cols.iter().enumerate() {
        if vi < visible_range_start || vi >= visible_range_end {
            continue;
        }

        let w = app.col_widths.get(*actual_idx).copied().unwrap_or(15);
        // Last visible column stretches to fill remaining space
        if vi == visible_range_end - 1 {
            constraints.push(Constraint::Min(w));
        } else {
            constraints.push(Constraint::Length(w));
        }

        // Sort indicator
        let sort_indicator = if app.sort_column == Some(*actual_idx) {
            match &app.sort_direction {
                SortDirection::Asc => " \u{25B2}",   // ▲
                SortDirection::Desc => " \u{25BC}",  // ▼
                SortDirection::None => "",
            }
        } else {
            ""
        };

        let is_cursor_col = app.cursor_col == vi;
        let header_style = if is_cursor_col {
            Style::default()
                .fg(colors.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default()
                .fg(colors.header_fg)
                .add_modifier(Modifier::BOLD)
        };

        header_cells.push(Cell::from(Span::styled(
            format!(" {}{}", col.name, sort_indicator),
            header_style,
        )));
    }

    let header = Row::new(header_cells)
        .style(Style::default().bg(colors.header_bg))
        .height(1)
        .bottom_margin(0);

    // Build data rows. For a base table `rows` is already the current page; for
    // an in-memory query result `rows` holds the whole set, so slice by
    // scroll_offset to scroll. Either way `actual_row = scroll_offset + idx`.
    let display_rows: &[Vec<String>] = if app.viewing_query_result {
        app.rows.get(app.scroll_offset..).unwrap_or(&[])
    } else {
        &app.rows
    };
    let rows: Vec<Row> = display_rows
        .iter()
        .enumerate()
        .map(|(display_idx, row_data)| {
            let actual_row = app.scroll_offset + display_idx;
            let is_selected_row = actual_row == app.cursor_row;
            let is_alt = display_idx % 2 == 1;

            let row_bg = if is_selected_row {
                colors.selection_bg
            } else if is_alt {
                colors.row_alt
            } else {
                colors.bg
            };

            let row_fg = if is_selected_row {
                colors.selection_fg
            } else {
                colors.fg
            };

            let mut cells: Vec<Cell> = Vec::new();

            // Row number
            if app.show_row_numbers {
                cells.push(Cell::from(Span::styled(
                    format!("{:>width$} ", actual_row + 1, width = row_num_width as usize - 2),
                    Style::default()
                        .fg(if is_selected_row { colors.accent } else { colors.muted })
                        .bg(row_bg)
                        .add_modifier(Modifier::DIM),
                )));
            }

            // Data cells
            for (vi, (actual_idx, _)) in vis_cols.iter().enumerate() {
                if vi < visible_range_start || vi >= visible_range_end {
                    continue;
                }

                let value = row_data
                    .get(*actual_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");

                let is_cursor = is_selected_row && app.cursor_col == vi;
                let is_editing = is_cursor && app.mode == AppMode::CellEdit;

                // Search match highlighting
                let is_search_match = !app.search_query.is_empty()
                    && app.search_matches.contains(&(actual_row, *actual_idx));

                let max_width = app.col_widths.get(*actual_idx).copied().unwrap_or(15) as usize;

                if is_editing {
                    // Show edit buffer with cursor indicator
                    let buf = &app.edit_buffer;
                    let cursor_pos = app.edit_cursor_pos;
                    let before: String = buf.chars().take(cursor_pos).collect();
                    let cursor_char = buf.chars().nth(cursor_pos).unwrap_or(' ');
                    let after: String = buf.chars().skip(cursor_pos + 1).collect();

                    let edit_style = Style::default()
                        .fg(colors.cursor_fg)
                        .bg(colors.accent)
                        .add_modifier(Modifier::UNDERLINED);
                    let cursor_style = Style::default()
                        .fg(colors.accent)
                        .bg(colors.cursor_fg)
                        .add_modifier(Modifier::BOLD);

                    // Truncate if needed to fit column width
                    let total_chars = buf.chars().count().max(cursor_pos + 1);
                    let avail = max_width.saturating_sub(2); // padding

                    let (display_before, display_cursor, display_after) = if total_chars <= avail {
                        (before, cursor_char, after)
                    } else {
                        // Show around cursor position
                        let start = cursor_pos.saturating_sub(avail / 2);
                        let b: String = buf.chars().skip(start).take(cursor_pos - start).collect();
                        let c = buf.chars().nth(cursor_pos).unwrap_or(' ');
                        let remaining = avail.saturating_sub(cursor_pos - start + 1);
                        let a: String = buf.chars().skip(cursor_pos + 1).take(remaining).collect();
                        (b, c, a)
                    };

                    cells.push(Cell::from(Line::from(vec![
                        Span::styled(" ", edit_style),
                        Span::styled(display_before, edit_style),
                        Span::styled(display_cursor.to_string(), cursor_style),
                        Span::styled(display_after, edit_style),
                    ])));
                } else {
                    let display_value = if value.chars().count() > max_width.saturating_sub(2) {
                        let truncated: String = value.chars().take(max_width.saturating_sub(3)).collect();
                        format!(" {}\u{2026}", truncated)
                    } else {
                        format!(" {}", value)
                    };

                    let style = if is_cursor {
                        // Active cell: bright accent bg, dark text, bold
                        Style::default()
                            .fg(colors.cursor_fg)
                            .bg(colors.cursor_bg)
                            .add_modifier(Modifier::BOLD)
                    } else if is_search_match {
                        Style::default()
                            .fg(colors.bg)
                            .bg(colors.search_match)
                            .add_modifier(Modifier::BOLD)
                    } else if value == "NULL" {
                        // NULL values: dim and muted
                        Style::default()
                            .fg(colors.muted)
                            .bg(row_bg)
                            .add_modifier(Modifier::DIM)
                    } else {
                        Style::default().fg(row_fg).bg(row_bg)
                    };

                    cells.push(Cell::from(Span::styled(display_value, style)));
                }
            }

            Row::new(cells).style(Style::default().bg(row_bg))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colors.border))
        .style(Style::default().bg(colors.bg));

    let table = Table::new(rows, &constraints)
        .header(header)
        .block(block)
        .column_spacing(1);

    f.render_widget(table, area);
}
