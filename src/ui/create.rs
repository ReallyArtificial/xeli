//! The "create a new table" screen — a standalone TUI shown before the main app
//! (like the file picker). Templates lead, because most creation is "give me the
//! standard tracker, not a blank grid"; a live preview shows the typed columns —
//! status pills and all — *before* you commit, and Ctrl+K hands off to the AI.

use crate::app::Theme;
use crate::data::schema::{ColumnType, TableSchema};
use crate::data::templates::{self, Template};
use crate::ui::theme::{get_theme_colors, pill_color, ThemeColors};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;
use std::time::Duration;

/// What the user chose on the create screen.
pub enum CreateChoice {
    /// A ready-to-build schema plus a suggested filename.
    Schema(TableSchema, String),
    /// A natural-language description — the caller asks the AI to design it.
    Ai(String),
    /// Cancelled.
    Cancel,
}

enum Stage {
    /// Browsing templates.
    List,
    /// Typing an AI description.
    Ai,
}

struct State {
    templates: Vec<Template>,
    cursor: usize,
    stage: Stage,
    ai_input: String,
    colors: ThemeColors,
}

pub fn run(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    theme: &Theme,
) -> Result<CreateChoice> {
    let mut state = State {
        templates: templates::all(),
        cursor: 0,
        stage: Stage::List,
        ai_input: String::new(),
        colors: get_theme_colors(theme),
    };

    loop {
        terminal.draw(|f| draw(f, &state))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                let len = state.templates.len();
                match state.stage {
                    Stage::List => match (key.modifiers, key.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyCode::Esc) => {
                            return Ok(CreateChoice::Cancel)
                        }
                        (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
                            state.stage = Stage::Ai;
                        }
                        (_, KeyCode::Up) | (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
                            if len > 0 {
                                state.cursor =
                                    if state.cursor == 0 { len - 1 } else { state.cursor - 1 };
                            }
                        }
                        (_, KeyCode::Down) | (KeyModifiers::CONTROL, KeyCode::Char('n')) => {
                            if len > 0 {
                                state.cursor = (state.cursor + 1) % len;
                            }
                        }
                        (_, KeyCode::Enter) => {
                            if let Some(t) = state.templates.get(state.cursor) {
                                return Ok(CreateChoice::Schema(
                                    t.schema(),
                                    t.filename.to_string(),
                                ));
                            }
                        }
                        _ => {}
                    },
                    Stage::Ai => match key.code {
                        KeyCode::Esc => {
                            state.stage = Stage::List;
                            state.ai_input.clear();
                        }
                        KeyCode::Enter => {
                            let prompt = state.ai_input.trim().to_string();
                            if !prompt.is_empty() {
                                return Ok(CreateChoice::Ai(prompt));
                            }
                        }
                        KeyCode::Backspace => {
                            state.ai_input.pop();
                        }
                        KeyCode::Char(c) => state.ai_input.push(c),
                        _ => {}
                    },
                }
            }
        }
    }
}

fn draw(f: &mut Frame, state: &State) {
    let area = centered_rect(74, 80, f.area());
    let c = &state.colors;

    let block = Block::default()
        .title(" Create a new table ")
        .title_style(Style::default().fg(c.accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(c.border))
        .style(Style::default().bg(c.bg));

    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // list
            Constraint::Length(1), // separator
            Constraint::Length(7), // preview
            Constraint::Length(1), // footer
        ])
        .split(inner);

    match state.stage {
        Stage::List => draw_list(f, chunks[0], state),
        Stage::Ai => draw_ai(f, chunks[0], state),
    }

    // Separator
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(c.border),
        ))),
        chunks[1],
    );

    draw_preview(f, chunks[2], state);

    // Footer
    let footer = match state.stage {
        Stage::List => Line::from(vec![
            Span::styled(
                " ↑↓ choose · Enter create · ",
                Style::default().fg(c.muted),
            ),
            Span::styled("⌃K describe with AI", Style::default().fg(c.accent2)),
            Span::styled(" · Esc cancel ", Style::default().fg(c.muted)),
        ]),
        Stage::Ai => Line::from(Span::styled(
            " Enter: let AI design it · Esc: back to templates ",
            Style::default().fg(c.muted),
        )),
    };
    f.render_widget(Paragraph::new(footer), chunks[3]);
}

fn draw_list(f: &mut Frame, area: Rect, state: &State) {
    let c = &state.colors;
    let max_items = area.height as usize;
    let start = if state.templates.len() <= max_items {
        0
    } else {
        state
            .cursor
            .saturating_sub(max_items / 2)
            .min(state.templates.len() - max_items)
    };

    let mut lines: Vec<Line> = Vec::new();
    for (offset, t) in state.templates.iter().skip(start).take(max_items).enumerate() {
        let idx = start + offset;
        let is_cursor = idx == state.cursor;
        let style = if is_cursor {
            Style::default()
                .fg(c.cursor_fg)
                .bg(c.cursor_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(c.fg)
        };
        let prefix = if is_cursor { " ▸ " } else { "   " };
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(format!("{}  ", t.icon), style),
            Span::styled(format!("{:<26}", t.name), style),
            Span::styled(
                t.description,
                if is_cursor {
                    Style::default().fg(c.cursor_fg).bg(c.cursor_bg)
                } else {
                    Style::default().fg(c.muted)
                },
            ),
        ]));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_ai(f: &mut Frame, area: Rect, state: &State) {
    let c = &state.colors;
    let lines = vec![
        Line::from(Span::styled(
            "  Describe the tracker you want — xeli designs the typed columns:",
            Style::default().fg(c.fg),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                " AI ",
                Style::default()
                    .fg(c.bg)
                    .bg(c.purple)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(&state.ai_input, Style::default().fg(c.fg)),
            Span::styled(
                "_",
                Style::default().fg(c.accent).add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  e.g. \"bug tracker for a mobile app with severity and which OS\"",
            Style::default().fg(c.muted),
        )),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

/// Live preview of the highlighted template's typed columns — status pills and
/// all — so the user sees what they're getting before creating it.
fn draw_preview(f: &mut Frame, area: Rect, state: &State) {
    let c = &state.colors;
    let schema = match state.stage {
        Stage::List => state.templates.get(state.cursor).map(|t| t.schema()),
        Stage::Ai => None,
    };

    let mut lines: Vec<Line> = Vec::new();
    if let Some(schema) = schema {
        for col in schema.columns.iter().take(area.height as usize) {
            let mut spans = vec![
                Span::styled(
                    format!("  {} ", col.col_type.glyph()),
                    Style::default().fg(c.accent2),
                ),
                Span::styled(format!("{:<14}", col.name), Style::default().fg(c.fg)),
            ];
            if let ColumnType::Select { values, .. } = &col.col_type {
                for v in values.iter().take(6) {
                    spans.push(Span::styled(
                        format!("● {} ", v.label),
                        Style::default().fg(pill_color(v.color)),
                    ));
                }
            } else {
                spans.push(Span::styled(
                    col.col_type.label().to_lowercase(),
                    Style::default().fg(c.muted),
                ));
            }
            lines.push(Line::from(spans));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  The AI will propose typed columns you can edit before creating.",
            Style::default().fg(c.muted),
        )));
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(v[1])[1]
}
