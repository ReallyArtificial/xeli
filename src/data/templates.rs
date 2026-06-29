//! Canonical tracker templates and the common status value-sets.
//!
//! Research finding: most "create a spreadsheet" is really "give me the standard
//! tracker, not a blank grid", and five constrained archetypes — Status,
//! Priority, Owner, Stage/Category, Date — recur in every one. So creation leads
//! with typed templates; the status value-sets people rebuild by hand every time
//! ship as one-keystroke presets.

use crate::data::schema::{
    select_colored, select_from_labels, ColumnSpec, ColumnType, PillColor, TableSchema,
};

pub struct Template {
    pub key: &'static str,
    pub icon: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub filename: &'static str,
    build: fn() -> TableSchema,
}

impl Template {
    pub fn schema(&self) -> TableSchema {
        (self.build)()
    }
}

fn text(name: &str) -> ColumnSpec {
    ColumnSpec {
        name: name.to_string(),
        col_type: ColumnType::Text,
    }
}
fn typed(name: &str, col_type: ColumnType) -> ColumnSpec {
    ColumnSpec {
        name: name.to_string(),
        col_type,
    }
}
fn person(name: &str) -> ColumnSpec {
    typed(name, ColumnType::Person)
}
fn date(name: &str) -> ColumnSpec {
    typed(name, ColumnType::Date)
}
fn currency(name: &str) -> ColumnSpec {
    typed(name, ColumnType::Currency)
}
fn number(name: &str) -> ColumnSpec {
    typed(name, ColumnType::Number)
}

// --- Curated status / priority sets (label, color), with their default index. -

fn status_todo_doing_done() -> ColumnType {
    select_colored(
        &[
            ("Todo", PillColor::Grey),
            ("Doing", PillColor::Blue),
            ("Done", PillColor::Green),
        ],
        0,
    )
}

fn priority_lmh() -> ColumnType {
    select_colored(
        &[
            ("Low", PillColor::Grey),
            ("Medium", PillColor::Yellow),
            ("High", PillColor::Red),
        ],
        1,
    )
}

// --- Templates ---------------------------------------------------------------

fn task_tracker() -> TableSchema {
    TableSchema::new(vec![
        text("task"),
        typed(
            "status",
            select_colored(
                &[
                    ("Not Started", PillColor::Grey),
                    ("In Progress", PillColor::Blue),
                    ("Blocked", PillColor::Red),
                    ("Done", PillColor::Green),
                ],
                0,
            ),
        ),
        typed("priority", priority_lmh()),
        person("owner"),
        date("due"),
        text("notes"),
    ])
}

fn issue_tracker() -> TableSchema {
    TableSchema::new(vec![
        text("title"),
        typed(
            "status",
            select_colored(
                &[
                    ("Open", PillColor::Yellow),
                    ("In Progress", PillColor::Blue),
                    ("Resolved", PillColor::Green),
                    ("Closed", PillColor::Grey),
                ],
                0,
            ),
        ),
        typed(
            "severity",
            select_colored(
                &[
                    ("Low", PillColor::Grey),
                    ("Medium", PillColor::Yellow),
                    ("High", PillColor::Orange),
                    ("Critical", PillColor::Red),
                ],
                1,
            ),
        ),
        person("assignee"),
        date("reported"),
        text("notes"),
    ])
}

fn content_calendar() -> TableSchema {
    TableSchema::new(vec![
        text("title"),
        typed(
            "channel",
            select_from_labels(&["Blog", "Twitter", "LinkedIn", "YouTube", "Newsletter"]),
        ),
        typed(
            "format",
            select_from_labels(&["Article", "Video", "Image", "Thread"]),
        ),
        typed(
            "status",
            select_colored(
                &[
                    ("Draft", PillColor::Grey),
                    ("In Review", PillColor::Yellow),
                    ("Approved", PillColor::Cyan),
                    ("Scheduled", PillColor::Blue),
                    ("Published", PillColor::Green),
                ],
                0,
            ),
        ),
        person("owner"),
        date("publish"),
    ])
}

fn okr_tracker() -> TableSchema {
    TableSchema::new(vec![
        text("objective"),
        text("key_result"),
        person("owner"),
        typed(
            "quarter",
            select_from_labels(&["Q1", "Q2", "Q3", "Q4"]),
        ),
        number("progress"),
        typed(
            "confidence",
            select_colored(
                &[
                    ("Low", PillColor::Red),
                    ("Medium", PillColor::Yellow),
                    ("High", PillColor::Green),
                ],
                2,
            ),
        ),
        typed(
            "status",
            select_colored(
                &[
                    ("On Track", PillColor::Green),
                    ("At Risk", PillColor::Yellow),
                    ("Behind", PillColor::Red),
                ],
                0,
            ),
        ),
    ])
}

fn crm_pipeline() -> TableSchema {
    TableSchema::new(vec![
        text("contact"),
        text("company"),
        typed(
            "stage",
            select_colored(
                &[
                    ("Lead", PillColor::Grey),
                    ("Qualified", PillColor::Cyan),
                    ("Demo", PillColor::Blue),
                    ("Proposal", PillColor::Purple),
                    ("Won", PillColor::Green),
                    ("Lost", PillColor::Red),
                ],
                0,
            ),
        ),
        currency("value"),
        person("owner"),
        text("next_step"),
        date("next_date"),
    ])
}

fn expense_tracker() -> TableSchema {
    TableSchema::new(vec![
        date("date"),
        text("description"),
        typed(
            "category",
            select_from_labels(&["Food", "Travel", "Software", "Office", "Other"]),
        ),
        currency("amount"),
        typed(
            "method",
            select_from_labels(&["Card", "Cash", "Transfer"]),
        ),
        text("notes"),
    ])
}

fn quick_tracker() -> TableSchema {
    TableSchema::new(vec![
        text("item"),
        typed("status", status_todo_doing_done()),
        text("notes"),
    ])
}

fn blank() -> TableSchema {
    TableSchema::new(vec![text("column_1"), text("column_2"), text("column_3")])
}

/// All templates, in the order shown in the create screen. Trackers first
/// (that's what people actually want), the lightweight starters last.
pub fn all() -> Vec<Template> {
    vec![
        Template {
            key: "tasks",
            icon: "✅",
            name: "Project / task tracker",
            description: "task · status · priority · owner · due",
            filename: "tasks.csv",
            build: task_tracker,
        },
        Template {
            key: "issues",
            icon: "🐛",
            name: "Issue / bug tracker",
            description: "title · status · severity · assignee · reported",
            filename: "issues.csv",
            build: issue_tracker,
        },
        Template {
            key: "content",
            icon: "🗓",
            name: "Content calendar",
            description: "title · channel · format · status · publish",
            filename: "content.csv",
            build: content_calendar,
        },
        Template {
            key: "okrs",
            icon: "🎯",
            name: "OKR tracker",
            description: "objective · key result · progress · confidence · status",
            filename: "okrs.csv",
            build: okr_tracker,
        },
        Template {
            key: "crm",
            icon: "👥",
            name: "CRM / pipeline",
            description: "contact · company · stage · value · owner",
            filename: "crm.csv",
            build: crm_pipeline,
        },
        Template {
            key: "expenses",
            icon: "💰",
            name: "Expense tracker",
            description: "date · description · category · amount · method",
            filename: "expenses.csv",
            build: expense_tracker,
        },
        Template {
            key: "quick",
            icon: "⚡",
            name: "Quick tracker",
            description: "item · status · notes — the 30-second tracker",
            filename: "tracker.csv",
            build: quick_tracker,
        },
        Template {
            key: "blank",
            icon: "＋",
            name: "Blank table",
            description: "three text columns — start from scratch",
            filename: "table.csv",
            build: blank,
        },
    ]
}

/// Build a template's schema by key (used by `xeli new <key>`). Returns `None`
/// for an unknown key.
pub fn by_key(key: &str) -> Option<TableSchema> {
    all().into_iter().find(|t| t.key == key).map(|t| t.schema())
}

/// The common status value-sets, offered as one-keystroke presets when a user
/// makes a Select column. Each is `(menu_label, ColumnType)`.
pub fn status_presets() -> Vec<(String, ColumnType)> {
    vec![
        ("Todo · Doing · Done".into(), status_todo_doing_done()),
        (
            "Backlog · Todo · In Progress · In Review · Done".into(),
            select_colored(
                &[
                    ("Backlog", PillColor::Grey),
                    ("Todo", PillColor::Cyan),
                    ("In Progress", PillColor::Blue),
                    ("In Review", PillColor::Purple),
                    ("Done", PillColor::Green),
                ],
                0,
            ),
        ),
        (
            "Open · In Progress · Resolved · Closed".into(),
            select_colored(
                &[
                    ("Open", PillColor::Yellow),
                    ("In Progress", PillColor::Blue),
                    ("Resolved", PillColor::Green),
                    ("Closed", PillColor::Grey),
                ],
                0,
            ),
        ),
        (
            "Not Started · On Track · At Risk · Done".into(),
            select_colored(
                &[
                    ("Not Started", PillColor::Grey),
                    ("On Track", PillColor::Green),
                    ("At Risk", PillColor::Yellow),
                    ("Done", PillColor::Blue),
                ],
                0,
            ),
        ),
        ("Low · Medium · High  (priority)".into(), priority_lmh()),
        (
            "Low · Medium · High · Critical".into(),
            select_colored(
                &[
                    ("Low", PillColor::Grey),
                    ("Medium", PillColor::Yellow),
                    ("High", PillColor::Orange),
                    ("Critical", PillColor::Red),
                ],
                1,
            ),
        ),
        (
            "Yes · No".into(),
            select_colored(&[("Yes", PillColor::Green), ("No", PillColor::Red)], 0),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_has_columns_and_unique_keys() {
        let ts = all();
        let mut keys: Vec<&str> = ts.iter().map(|t| t.key).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n, "template keys must be unique");
        for t in &ts {
            assert!(!t.schema().columns.is_empty(), "{} has no columns", t.key);
        }
    }

    #[test]
    fn by_key_resolves_and_rejects() {
        assert!(by_key("tasks").is_some());
        assert!(by_key("nope").is_none());
    }

    #[test]
    fn presets_are_all_selects_with_defaults() {
        for (label, ct) in status_presets() {
            match ct {
                ColumnType::Select { values, default } => {
                    assert!(!values.is_empty(), "{label} empty");
                    assert!(default.is_some(), "{label} has no default");
                }
                _ => panic!("{label} is not a select"),
            }
        }
    }
}
