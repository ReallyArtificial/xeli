use crate::ui::theme::{get_theme_colors, ThemeColors};
use crate::app::Theme;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

struct PickerState {
    all_files: Vec<PathBuf>,
    root: PathBuf,
    query: String,
    cursor: usize,
    colors: ThemeColors,
}

impl PickerState {
    /// Path shown to the user — relative to the directory xeli was launched in,
    /// so nested files like `data/sales.csv` stay distinguishable.
    fn display_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    }

    fn filtered(&self) -> Vec<&PathBuf> {
        if self.query.is_empty() {
            return self.all_files.iter().collect();
        }
        let q = self.query.to_lowercase();
        self.all_files
            .iter()
            .filter(|p| fuzzy_contains(&self.display_path(p).to_lowercase(), &q))
            .collect()
    }
}

/// Simple subsequence match — every char of needle appears in haystack in order.
fn fuzzy_contains(haystack: &str, needle: &str) -> bool {
    let mut hay = haystack.chars();
    'outer: for nc in needle.chars() {
        for hc in hay.by_ref() {
            if hc == nc {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

/// What the user chose in the picker.
pub enum PickOutcome {
    Open(PathBuf),
    Create,
}

/// Run a small TUI to pick a data file — or create a new one. The first row is
/// always "Create a new table"; the rest are the discovered files. Returns
/// Ok(None) if the user cancels with Esc/q.
pub fn pick(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    files: Vec<PathBuf>,
    theme: &Theme,
) -> Result<Option<PickOutcome>> {
    let mut state = PickerState {
        all_files: files,
        root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        query: String::new(),
        // Row 0 is "Create new"; default the highlight to the first file so Enter
        // still opens (the create affordance sits one step up).
        cursor: 1,
        colors: get_theme_colors(theme),
    };

    // How many rows the list pane can show — kept in sync by draw() so PageUp/Down
    // jump by a real screenful.
    let mut page = 10usize;

    loop {
        terminal.draw(|f| page = draw(f, &state).max(1))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Total rows = the create row (index 0) + the filtered files.
                let len = state.filtered().len() + 1;
                let reset_cursor = |s: &PickerState| if s.filtered().is_empty() { 0 } else { 1 };
                match (key.modifiers, key.code) {
                    (KeyModifiers::CONTROL, KeyCode::Char('c')) => return Ok(None),
                    (_, KeyCode::Esc) => return Ok(None),
                    (_, KeyCode::Enter) => {
                        if state.cursor == 0 {
                            return Ok(Some(PickOutcome::Create));
                        }
                        let filtered = state.filtered();
                        if let Some(pick) = filtered.get(state.cursor - 1) {
                            return Ok(Some(PickOutcome::Open((*pick).clone())));
                        }
                    }
                    // Up / Ctrl+P — wrap to bottom from the top.
                    (_, KeyCode::Up) | (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
                        state.cursor = if state.cursor == 0 { len - 1 } else { state.cursor - 1 };
                    }
                    // Down / Ctrl+N — wrap to top from the bottom.
                    (_, KeyCode::Down) | (KeyModifiers::CONTROL, KeyCode::Char('n')) => {
                        state.cursor = if state.cursor + 1 >= len { 0 } else { state.cursor + 1 };
                    }
                    (_, KeyCode::PageUp) => {
                        state.cursor = state.cursor.saturating_sub(page);
                    }
                    (_, KeyCode::PageDown) => {
                        state.cursor = (state.cursor + page).min(len - 1);
                    }
                    (_, KeyCode::Home) => state.cursor = 0,
                    (_, KeyCode::End) => state.cursor = len.saturating_sub(1),
                    (_, KeyCode::Backspace) => {
                        state.query.pop();
                        state.cursor = reset_cursor(&state);
                    }
                    (_, KeyCode::Char(c)) => {
                        state.query.push(c);
                        state.cursor = reset_cursor(&state);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Draws the picker and returns how many file rows the list pane can show, so
/// the event loop can page by a real screenful.
fn draw(f: &mut Frame, state: &PickerState) -> usize {
    let area = centered_rect(70, 70, f.area());

    let filtered = state.filtered();
    let title = if state.query.is_empty() {
        format!(" Pick a data file · {} found ", state.all_files.len())
    } else {
        format!(" Pick a data file · {}/{} match ", filtered.len(), state.all_files.len())
    };

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(state.colors.accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(state.colors.border))
        .style(Style::default().bg(state.colors.bg));

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search
            Constraint::Length(1), // separator
            Constraint::Min(1),    // list
            Constraint::Length(1), // hint footer
        ])
        .split(inner);

    // Search bar
    let search = Line::from(vec![
        Span::styled(" Find: ", Style::default().fg(state.colors.muted)),
        Span::styled(&state.query, Style::default().fg(state.colors.fg)),
        Span::styled(
            "_",
            Style::default()
                .fg(state.colors.accent)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]);
    f.render_widget(
        Paragraph::new(search).style(Style::default().bg(state.colors.bg)),
        chunks[0],
    );

    // Separator
    let sep = Line::from(Span::styled(
        "─".repeat(inner.width as usize),
        Style::default().fg(state.colors.border),
    ));
    f.render_widget(Paragraph::new(sep), chunks[1]);

    // File list — row 0 is always "Create a new table", pinned at the top.
    let total_height = chunks[2].height as usize;
    let mut lines: Vec<Line> = Vec::new();

    let create_cursor = state.cursor == 0;
    let create_style = if create_cursor {
        Style::default()
            .fg(state.colors.cursor_fg)
            .bg(state.colors.cursor_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(state.colors.accent2).add_modifier(Modifier::BOLD)
    };
    lines.push(Line::from(vec![
        Span::styled(if create_cursor { " ▸ " } else { "   " }, create_style),
        Span::styled("✦ Create a new table…", create_style),
        Span::styled(
            "  template · AI · blank",
            if create_cursor {
                Style::default().fg(state.colors.cursor_fg).bg(state.colors.cursor_bg)
            } else {
                Style::default().fg(state.colors.muted)
            },
        ),
    ]));

    let file_area = total_height.saturating_sub(1);
    if filtered.is_empty() {
        if !state.query.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No matching files",
                Style::default().fg(state.colors.muted),
            )));
        }
    } else if file_area > 0 {
        // Center the file cursor in the window where possible, clamped to ends.
        let file_cursor = state.cursor.saturating_sub(1);
        let start = if filtered.len() <= file_area {
            0
        } else {
            file_cursor
                .saturating_sub(file_area / 2)
                .min(filtered.len() - file_area)
        };
        for (offset, path) in filtered.iter().skip(start).take(file_area).enumerate() {
            let idx = start + offset;
            let is_cursor = state.cursor == idx + 1;
            let name = state.display_path(path);
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_uppercase())
                .unwrap_or_else(|| "?".to_string());
            let meta = std::fs::metadata(path);
            let size_str = human_size(meta.as_ref().map(|m| m.len()).unwrap_or(0));
            let when = meta
                .and_then(|m| m.modified())
                .map(relative_time)
                .unwrap_or_default();

            let style = if is_cursor {
                Style::default()
                    .fg(state.colors.cursor_fg)
                    .bg(state.colors.cursor_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(state.colors.fg)
            };

            let prefix = if is_cursor { " > " } else { "   " };
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("[{:<4}] ", ext), Style::default().fg(state.colors.muted)),
                Span::styled(name, style),
                Span::styled(
                    format!("   {:>8}", size_str),
                    Style::default().fg(state.colors.muted),
                ),
                Span::styled(
                    format!("   {:>9}", when),
                    Style::default().fg(state.colors.muted),
                ),
            ]));
        }
    }

    f.render_widget(Paragraph::new(lines), chunks[2]);

    // Footer hints + position
    let pos = if state.cursor == 0 {
        "new  ".to_string()
    } else {
        format!("{}/{}  ", state.cursor, filtered.len())
    };
    let footer = Line::from(vec![
        Span::styled(
            " Type to filter · ↑↓ select · Enter open · Esc quit ",
            Style::default().fg(state.colors.muted),
        ),
        Span::styled(
            format!("· {}", pos),
            Style::default().fg(state.colors.accent2),
        ),
    ]);
    f.render_widget(Paragraph::new(footer), chunks[3]);

    file_area.max(1)
}

/// Compact "time since modified" label, e.g. `2h`, `3d`, `5mo`. Keeps the picker
/// list legible since files are sorted newest-first.
fn relative_time(t: std::time::SystemTime) -> String {
    let secs = match std::time::SystemTime::now().duration_since(t) {
        Ok(d) => d.as_secs(),
        Err(_) => return "now".to_string(), // mtime in the future — clock skew
    };
    if secs < 60 {
        "now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 2_592_000 {
        format!("{}d ago", secs / 86_400)
    } else if secs < 31_536_000 {
        format!("{}mo ago", secs / 2_592_000)
    } else {
        format!("{}y ago", secs / 31_536_000)
    }
}

fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
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
        .split(popup_layout[1])[1]
}
