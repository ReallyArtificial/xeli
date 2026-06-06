use crate::app::{App, AiKeyStage, AppMode, ComputedColumnStage, GroupByStage, JoinStage};
use crate::ui::theme::ThemeColors;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render(f: &mut Frame, area: Rect, app: &App, colors: &ThemeColors) {
    // Two rows: line 0 = mode + transient message, line 1 = persistent key hints.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    render_mode_line(f, rows[0], app, colors);
    render_hints_line(f, rows[1], app, colors);
}

fn render_mode_line(f: &mut Frame, area: Rect, app: &App, colors: &ThemeColors) {
    let mode_span = mode_badge(&app.mode, colors);

    let trailing = if let Some(err) = &app.error_message {
        Span::styled(format!(" {} ", err), Style::default().fg(colors.error))
    } else if let Some(msg) = &app.status_message {
        Span::styled(format!(" {} ", msg), Style::default().fg(colors.success))
    } else if app.ai_loading {
        Span::styled(
            " AI thinking... ",
            Style::default()
                .fg(colors.purple)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        // Quick mode-specific context summary on the right of the badge.
        Span::styled(mode_context(app), Style::default().fg(colors.muted))
    };

    let line = Line::from(vec![mode_span, trailing]);
    let paragraph = Paragraph::new(line).style(Style::default().bg(colors.header_bg));
    f.render_widget(paragraph, area);
}

fn render_hints_line(f: &mut Frame, area: Rect, app: &App, colors: &ThemeColors) {
    let hints = key_hints(app);
    let line = Line::from(Span::styled(hints, Style::default().fg(colors.muted)));
    let paragraph = Paragraph::new(line).style(Style::default().bg(colors.bg));
    f.render_widget(paragraph, area);
}

fn mode_badge(mode: &AppMode, colors: &ThemeColors) -> Span<'static> {
    let (label, bg) = match mode {
        AppMode::Normal => (" NORMAL ", colors.green),
        AppMode::Search => (" SEARCH ", colors.yellow),
        AppMode::Filter => (" FILTER ", colors.pink),
        AppMode::AiQuery => (" AI ", colors.purple),
        AppMode::AiKeySetup => (" AI SETUP ", colors.purple),
        AppMode::SqlQuery => (" SQL ", colors.cyan),
        AppMode::CellEdit => (" EDIT ", colors.warning),
        AppMode::Command => (" CMD ", colors.accent),
        AppMode::Sparkline => (" CHART ", colors.cyan),
        AppMode::FormulaBar => (" FORMULA ", colors.green),
        AppMode::ComputedColumn => (" +COL ", colors.green),
        AppMode::GroupBy => (" GROUP ", colors.purple),
        AppMode::Join => (" JOIN ", colors.purple),
        AppMode::CellDetail => (" CELL ", colors.accent),
        AppMode::ColumnStats => (" STATS ", colors.accent),
        AppMode::Export => (" EXPORT ", colors.accent),
        AppMode::Help => (" HELP ", colors.accent),
    };
    Span::styled(
        label,
        Style::default().fg(colors.bg).bg(bg).add_modifier(Modifier::BOLD),
    )
}

fn mode_context(app: &App) -> String {
    // Quick context shown next to the mode badge when nothing else is happening.
    let pos = format!(
        " R{}/{}  C{}/{} ",
        app.cursor_row + 1,
        app.filtered_rows.max(app.total_rows).max(1),
        app.cursor_col + 1,
        app.visible_columns().len().max(1),
    );
    let theme = format!(" {} ", app.theme.name());
    format!("{}{}", pos, theme)
}

fn key_hints(app: &App) -> String {
    match &app.mode {
        AppMode::Normal if app.viewing_query_result => {
            " Viewing query result · Esc or u: back to full table · e export · / find · t theme · q quit ".to_string()
        }
        AppMode::Normal => {
            " Ctrl+K AI · Ctrl+Q SQL · / find · f filter · s sort · g group · J join · = formula · e export · t theme · ? help · q quit ".to_string()
        }
        AppMode::Search => " Type to search (regex)  Enter:confirm  n/N:next/prev  Esc:cancel ".to_string(),
        AppMode::Filter => " Build filter step-by-step  Enter:next  Tab:next field  Esc:cancel ".to_string(),
        AppMode::AiQuery => " Ask in plain English  Enter:send  Esc:cancel ".to_string(),
        AppMode::AiKeySetup => match &app.ai_key_stage {
            AiKeyStage::PickProvider => " j/k:choose provider  Enter:next  Esc:cancel ".to_string(),
            AiKeyStage::EnterKey => " Paste API key  Enter:save  Esc:cancel ".to_string(),
        },
        AppMode::SqlQuery => {
            let cols = if app.data_columns.len() > 8 {
                format!("{}, …", app.data_columns[..8].join(", "))
            } else {
                app.data_columns.join(", ")
            };
            format!(" table: data ({})  ·  Enter:run  Up/Down:history  Esc:cancel ", cols)
        }
        AppMode::CellDetail => " Esc:close ".to_string(),
        AppMode::CellEdit => " ←→:move cursor  Enter:save  Esc:cancel ".to_string(),
        AppMode::ColumnStats => " Esc:close ".to_string(),
        AppMode::Export => " j/k:format  Enter:export  Esc:cancel ".to_string(),
        AppMode::Command => " Type to fuzzy-search commands  Enter:run  Esc:cancel ".to_string(),
        AppMode::Help => " Esc:close · q:close ".to_string(),
        AppMode::Sparkline => " Esc:close ".to_string(),
        AppMode::FormulaBar => " e.g. SUM(price)  price * qty  Enter:evaluate  Esc:cancel ".to_string(),
        AppMode::ComputedColumn => match &app.computed_col_stage {
            ComputedColumnStage::EnterName => " Name the column  Enter:next  Esc:cancel ".to_string(),
            ComputedColumnStage::EnterExpression => " Expression  Enter:apply  Shift+Tab:back  Esc:cancel ".to_string(),
        },
        AppMode::GroupBy => match &app.groupby_stage {
            GroupByStage::SelectGroupCols => " Space:toggle  j/k:move  Enter:next  Esc:cancel ".to_string(),
            GroupByStage::SelectAggregation => " h/l:function  j/k:column  Enter:apply  Esc:cancel ".to_string(),
        },
        AppMode::Join => match &app.join_stage {
            JoinStage::EnterPath => " Enter file path  Enter:load  Esc:cancel ".to_string(),
            JoinStage::SelectJoinType => " j/k:type  Enter:next  Esc:cancel ".to_string(),
            JoinStage::SelectColumns => " Tab:switch side  j/k:pick column  Enter:join  Esc:cancel ".to_string(),
        },
    }
}
