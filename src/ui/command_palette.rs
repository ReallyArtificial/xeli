use crate::app::App;
use crate::ui::theme::ThemeColors;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
    pub shortcut: &'static str,
}

pub fn get_commands() -> Vec<Command> {
    vec![
        Command { name: "Search", description: "Regex search across all columns", shortcut: "/" },
        Command { name: "Filter", description: "Add a column filter", shortcut: "f" },
        Command { name: "Clear Filters", description: "Remove all active filters", shortcut: "F" },
        Command { name: "Sort", description: "Sort by current column", shortcut: "s" },
        Command { name: "AI Query", description: "Natural language data query", shortcut: "Ctrl+K" },
        Command { name: "SQL Query", description: "Direct SQL query", shortcut: "Ctrl+Q" },
        Command { name: "Export", description: "Export data to file", shortcut: "e" },
        Command { name: "Column Stats", description: "Show statistics for current column", shortcut: "Ctrl+I" },
        Command { name: "Edit Cell", description: "Edit current cell value", shortcut: "i" },
        Command { name: "Cell Detail", description: "View full cell content", shortcut: "Enter" },
        Command { name: "Toggle Row Numbers", description: "Show/hide row numbers", shortcut: "r" },
        Command { name: "Cycle Theme", description: "Switch color theme", shortcut: "t" },
        Command { name: "Hide Column", description: "Hide current column", shortcut: "-" },
        Command { name: "Show All Columns", description: "Unhide all columns", shortcut: "+" },
        Command { name: "Sparkline", description: "Show histogram/sparkline for column", shortcut: "v" },
        Command { name: "Formula Bar", description: "Evaluate expression on data", shortcut: "=" },
        Command { name: "Add Row", description: "Append a blank row to fill in", shortcut: "o" },
        Command { name: "Add Column", description: "Add a typed column (text, date, status dropdown…)", shortcut: "a" },
        Command { name: "Delete Row", description: "Delete the current row", shortcut: "Shift+D" },
        Command { name: "Board View", description: "Kanban grouped by a status column", shortcut: "b" },
        Command { name: "Save", description: "Write data + schema sidecar to disk", shortcut: "Ctrl+S" },
        Command { name: "Computed Column", description: "Add a derived column", shortcut: "c" },
        Command { name: "Group By", description: "Pivot / group-by wizard", shortcut: "g" },
        Command { name: "Join", description: "Join with another file", shortcut: "J" },
        Command { name: "Back to Table", description: "Leave query result, show the full data table", shortcut: "Esc" },
        Command { name: "Undo", description: "Restore previous view", shortcut: "u" },
        Command { name: "Go to Top", description: "Jump to first row", shortcut: "gg" },
        Command { name: "Go to Bottom", description: "Jump to last row", shortcut: "G" },
        Command { name: "Help", description: "Show keybinding help", shortcut: "?" },
        Command { name: "Quit", description: "Exit xeli", shortcut: "q" },
    ]
}

pub fn filtered_commands(query: &str) -> Vec<&'static Command> {
    let commands = get_commands();
    // Leak to get 'static — this is fine for a CLI tool
    let commands: &'static [Command] = Box::leak(commands.into_boxed_slice());

    if query.is_empty() {
        return commands.iter().collect();
    }

    let query_lower = query.to_lowercase();
    commands
        .iter()
        .filter(|cmd| {
            cmd.name.to_lowercase().contains(&query_lower)
                || cmd.description.to_lowercase().contains(&query_lower)
        })
        .collect()
}

pub fn render(f: &mut Frame, app: &App, colors: &ThemeColors) {
    let area = super::centered_rect(50, 60, f.area());

    let block = Block::default()
        .title(" Command Palette ")
        .title_style(
            Style::default()
                .fg(colors.accent)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colors.border))
        .style(Style::default().bg(colors.bg));

    let commands = filtered_commands(&app.command_query);

    let mut lines = vec![
        Line::from(vec![
            Span::styled(" > ", Style::default().fg(colors.accent)),
            Span::styled(&app.command_query, Style::default().fg(colors.fg)),
            Span::styled(
                "_",
                Style::default()
                    .fg(colors.accent)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
        Line::from(""),
    ];

    let max_items = (area.height as usize).saturating_sub(5);

    for (i, cmd) in commands.iter().take(max_items).enumerate() {
        let is_selected = i == app.command_selected;

        let style = if is_selected {
            Style::default()
                .fg(colors.cursor_fg)
                .bg(colors.cursor_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.fg)
        };

        let shortcut_style = if is_selected {
            Style::default()
                .fg(colors.cursor_fg)
                .bg(colors.cursor_bg)
        } else {
            Style::default().fg(colors.muted)
        };

        lines.push(Line::from(vec![
            Span::styled(if is_selected { " > " } else { "   " }, style),
            Span::styled(cmd.name, style),
            Span::styled(
                format!("  {}", cmd.description),
                if is_selected {
                    Style::default()
                        .fg(colors.cursor_fg)
                        .bg(colors.cursor_bg)
                        .add_modifier(Modifier::DIM)
                } else {
                    Style::default().fg(colors.muted)
                },
            ),
            Span::styled(format!("  [{}]", cmd.shortcut), shortcut_style),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}
