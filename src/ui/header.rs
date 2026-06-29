use crate::app::App;
use crate::ui::theme::ThemeColors;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render(f: &mut Frame, area: Rect, app: &App, colors: &ThemeColors) {
    // Line 1: branding + filename + dimensions
    let mut spans1 = vec![
        Span::styled(
            " xeli ",
            Style::default()
                .fg(colors.cursor_fg)
                .bg(colors.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default().bg(colors.bg)),
        Span::styled(
            format!(" {} ", app.file_format.icon()),
            Style::default()
                .fg(colors.cursor_fg)
                .bg(colors.accent2)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} ", app.filename()),
            Style::default()
                .fg(colors.fg)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    // Unsaved-changes dot — plain text is the source of truth, so make "you have
    // edits not on disk" impossible to miss (and trivial to act on: Ctrl+S).
    if app.dirty {
        spans1.push(Span::styled(
            "\u{25CF} ",
            Style::default().fg(colors.warning).add_modifier(Modifier::BOLD),
        ));
    }

    spans1.push(Span::styled(
        format!("\u{2502} {}r \u{00D7} {}c ", app.filtered_rows, app.columns.len()),
        Style::default().fg(colors.muted),
    ));

    // Sort indicator
    if let Some(sort_col) = app.sort_column {
        let dir = match &app.sort_direction {
            crate::app::SortDirection::Asc => "\u{25B2}",
            crate::app::SortDirection::Desc => "\u{25BC}",
            crate::app::SortDirection::None => "",
        };
        if let Some(col) = app.columns.get(sort_col) {
            if !dir.is_empty() {
                spans1.push(Span::styled(
                    format!("\u{2502} {} {} ", col.name, dir),
                    Style::default().fg(colors.yellow),
                ));
            }
        }
    }

    // Filter count
    let active = app.filters.iter().filter(|f| f.enabled).count();
    if active > 0 {
        spans1.push(Span::styled(
            format!(
                "\u{2502} \u{26D4} {} filter{} ",
                active,
                if active > 1 { "s" } else { "" }
            ),
            Style::default().fg(colors.pink),
        ));
    }

    // Filtered row count
    if app.filtered_rows != app.total_rows {
        spans1.push(Span::styled(
            format!("\u{2502} {}/{} rows ", app.filtered_rows, app.total_rows),
            Style::default().fg(colors.warning),
        ));
    }

    // Line 2: cursor position + theme
    let line2 = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            format!(
                "R{}/{}",
                if app.filtered_rows > 0 {
                    app.cursor_row + 1
                } else {
                    0
                },
                app.filtered_rows,
            ),
            Style::default().fg(colors.muted),
        ),
        Span::styled(
            format!(
                "  C{}/{}",
                app.cursor_col + 1,
                app.visible_columns().len(),
            ),
            Style::default().fg(colors.muted),
        ),
        Span::styled(
            format!("  \u{25CF} {}", app.theme.name()),
            Style::default().fg(colors.purple),
        ),
    ]);

    let paragraph =
        Paragraph::new(vec![Line::from(spans1), line2]).style(Style::default().bg(colors.bg));

    f.render_widget(paragraph, area);
}
