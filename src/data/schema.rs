//! The semantic shape of a table — column names and their *types*.
//!
//! xeli stores created tables as plain VARCHAR columns inside DuckDB (so editing
//! never hits a cast error and the file stays as texty as a CSV). The richer
//! meaning — "this column is a status dropdown of Todo/Doing/Done", "this one is
//! a date" — lives here, in a `TableSchema`, and is persisted next to the data
//! file as a small, git-diffable `*.xeli.json` sidecar. The sidecar is what lets
//! a CSV carry a dropdown: we read it back to restore typed editing, and we use
//! it to materialise real data-validation dropdowns on xlsx export.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A named color for a select value. Resolved against the active theme at render
/// time (see `ui::theme::pill_color`) so pills look right in every theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PillColor {
    Grey,
    Blue,
    Green,
    Yellow,
    Orange,
    Red,
    Purple,
    Pink,
    Cyan,
}

impl PillColor {
    /// The palette in round-robin order — used to auto-color custom value sets.
    pub const PALETTE: [PillColor; 9] = [
        PillColor::Blue,
        PillColor::Green,
        PillColor::Yellow,
        PillColor::Purple,
        PillColor::Pink,
        PillColor::Cyan,
        PillColor::Orange,
        PillColor::Red,
        PillColor::Grey,
    ];
}

/// One allowed value of a Select column: a label plus its pill color.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectValue {
    pub label: String,
    pub color: PillColor,
}

/// The semantic type of a column. `Select` is the centerpiece — one type that
/// carries its own value set, colors, and default, so "make a status dropdown"
/// is a single decision instead of the spreadsheet ritual of free-text column +
/// data-validation rule + N conditional-formatting rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColumnType {
    Text,
    Number,
    Currency,
    Date,
    Bool,
    Person,
    Link,
    Select {
        values: Vec<SelectValue>,
        #[serde(default)]
        default: Option<String>,
    },
}

impl ColumnType {
    /// Short human label shown in the type picker.
    pub fn label(&self) -> &'static str {
        match self {
            ColumnType::Text => "Text",
            ColumnType::Number => "Number",
            ColumnType::Currency => "Currency",
            ColumnType::Date => "Date",
            ColumnType::Bool => "Checkbox",
            ColumnType::Person => "Person",
            ColumnType::Link => "Link",
            ColumnType::Select { .. } => "Select (dropdown)",
        }
    }

    /// A one-glyph hint shown next to the column name / in pickers.
    pub fn glyph(&self) -> &'static str {
        match self {
            ColumnType::Text => "T",
            ColumnType::Number => "#",
            ColumnType::Currency => "$",
            ColumnType::Date => "▦",
            ColumnType::Bool => "☑",
            ColumnType::Person => "@",
            ColumnType::Link => "↗",
            ColumnType::Select { .. } => "◉",
        }
    }

    /// True for the numeric family, so sorting can cast to a number rather than
    /// comparing the stored text lexicographically.
    pub fn is_numeric(&self) -> bool {
        matches!(self, ColumnType::Number | ColumnType::Currency)
    }

    /// The eight pickable base types, in display order. `Select` here is empty —
    /// its values are filled in by the preset/custom step.
    pub fn pickable() -> Vec<ColumnType> {
        vec![
            ColumnType::Select {
                values: Vec::new(),
                default: None,
            },
            ColumnType::Text,
            ColumnType::Number,
            ColumnType::Currency,
            ColumnType::Date,
            ColumnType::Person,
            ColumnType::Bool,
            ColumnType::Link,
        ]
    }

    /// The default value seeded into new/blank rows. Only Select columns carry
    /// one, so a fresh tracker shows status pills immediately.
    pub fn default_value(&self) -> Option<String> {
        match self {
            ColumnType::Select { default, .. } => default.clone(),
            _ => None,
        }
    }

    /// Look up a value's pill, if this is a Select column and the value is one of
    /// its allowed labels.
    pub fn select_value(&self, value: &str) -> Option<&SelectValue> {
        match self {
            ColumnType::Select { values, .. } => values.iter().find(|v| v.label == value),
            _ => None,
        }
    }
}

/// A single column: a name and its semantic type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub name: String,
    #[serde(flatten)]
    pub col_type: ColumnType,
}

/// The full typed shape of a table. Serialized to the sidecar.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TableSchema {
    /// Schema format version, so future readers can migrate.
    #[serde(default = "default_version")]
    pub version: u32,
    pub columns: Vec<ColumnSpec>,
}

fn default_version() -> u32 {
    1
}

impl TableSchema {
    pub fn new(columns: Vec<ColumnSpec>) -> Self {
        TableSchema {
            version: 1,
            columns,
        }
    }

    pub fn col_type(&self, name: &str) -> Option<&ColumnType> {
        self.columns
            .iter()
            .find(|c| c.name == name)
            .map(|c| &c.col_type)
    }

    /// Where the sidecar lives for a given data file: a sibling `<stem>.xeli.json`.
    /// e.g. `tasks.csv` → `tasks.xeli.json`.
    pub fn sidecar_path(data_path: &str) -> PathBuf {
        Path::new(data_path).with_extension("xeli.json")
    }

    pub fn load_sidecar(data_path: &str) -> Option<TableSchema> {
        let p = Self::sidecar_path(data_path);
        let content = std::fs::read_to_string(&p).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save_sidecar(&self, data_path: &str) -> anyhow::Result<PathBuf> {
        let p = Self::sidecar_path(data_path);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&p, json)?;
        Ok(p)
    }

    /// Parse a Rails-style `name:type` mini-DSL into a schema. Scriptable creation:
    ///
    ///   `title status:select(Todo,Doing,Done)=Todo owner:person due:date done:bool`
    ///
    /// - A bare token (`title`) is a Text column.
    /// - `name:type` sets the type; unknown/blank types fall back to Text.
    /// - `select(a,b,c)` (aliases: `status`, `enum`) lists the allowed values,
    ///   auto-colored; an optional `=value` sets the default.
    pub fn from_dsl(spec: &str) -> TableSchema {
        let mut columns = Vec::new();
        for token in spec.split_whitespace() {
            if token.is_empty() {
                continue;
            }
            let (name, rest) = match token.split_once(':') {
                Some((n, r)) => (n, r),
                None => (token, ""),
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let col_type = parse_dsl_type(rest);
            columns.push(ColumnSpec {
                name: name.to_string(),
                col_type,
            });
        }
        TableSchema::new(columns)
    }
}

/// Build a `Select` type from labels, auto-coloring them and defaulting to the
/// first value. The single helper behind every preset and the custom-values path.
pub fn select_from_labels(labels: &[&str]) -> ColumnType {
    let values: Vec<SelectValue> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| SelectValue {
            label: l.trim().to_string(),
            color: PillColor::PALETTE[i % PillColor::PALETTE.len()],
        })
        .collect();
    let default = values.first().map(|v| v.label.clone());
    ColumnType::Select { values, default }
}

/// Like `select_from_labels` but with explicit colors (used by curated presets
/// where the colors carry meaning — green Done, red Critical, etc.).
pub fn select_colored(pairs: &[(&str, PillColor)], default_idx: usize) -> ColumnType {
    let values: Vec<SelectValue> = pairs
        .iter()
        .map(|(l, c)| SelectValue {
            label: l.to_string(),
            color: *c,
        })
        .collect();
    let default = values.get(default_idx).map(|v| v.label.clone());
    ColumnType::Select { values, default }
}

/// Map a type *name* (+ optional values/default) to a `ColumnType`. Shared by
/// the DSL and the AI JSON parser so both accept the same vocabulary.
pub fn column_type_named(kind: &str, values: &[String], default: Option<&str>) -> ColumnType {
    match kind.trim().to_lowercase().as_str() {
        "select" | "status" | "enum" | "dropdown" => {
            let labels: Vec<&str> = values.iter().map(|s| s.as_str()).collect();
            if labels.is_empty() {
                return ColumnType::Text;
            }
            let mut ct = select_from_labels(&labels);
            if let (ColumnType::Select { default: d, .. }, Some(req)) = (&mut ct, default) {
                if labels.iter().any(|l| *l == req) {
                    *d = Some(req.to_string());
                }
            }
            ct
        }
        "number" | "num" | "int" | "integer" | "float" => ColumnType::Number,
        "currency" | "money" | "amount" | "price" => ColumnType::Currency,
        "date" | "day" | "datetime" => ColumnType::Date,
        "bool" | "boolean" | "checkbox" | "check" => ColumnType::Bool,
        "person" | "owner" | "user" | "assignee" => ColumnType::Person,
        "link" | "url" | "href" => ColumnType::Link,
        _ => ColumnType::Text,
    }
}

/// Parse the JSON an AI returns for the create flow into a schema. Lenient: it
/// strips code fences and slices to the outermost `{...}` so a chatty model
/// still parses.
pub fn from_ai_json(raw: &str) -> anyhow::Result<TableSchema> {
    #[derive(Deserialize)]
    struct AiCol {
        name: String,
        #[serde(rename = "type", default)]
        kind: String,
        #[serde(default)]
        values: Vec<String>,
        #[serde(default)]
        default: Option<String>,
    }
    #[derive(Deserialize)]
    struct AiSchema {
        columns: Vec<AiCol>,
    }

    let cleaned = raw.trim();
    let start = cleaned.find('{');
    let end = cleaned.rfind('}');
    let json = match (start, end) {
        (Some(s), Some(e)) if e >= s => &cleaned[s..=e],
        _ => anyhow::bail!("AI did not return a JSON object"),
    };

    let parsed: AiSchema = serde_json::from_str(json)?;
    let columns: Vec<ColumnSpec> = parsed
        .columns
        .into_iter()
        .filter(|c| !c.name.trim().is_empty())
        .map(|c| ColumnSpec {
            name: c.name.trim().to_string(),
            col_type: column_type_named(&c.kind, &c.values, c.default.as_deref()),
        })
        .collect();
    if columns.is_empty() {
        anyhow::bail!("AI returned no columns");
    }
    Ok(TableSchema::new(columns))
}

fn parse_dsl_type(rest: &str) -> ColumnType {
    let rest = rest.trim();
    if rest.is_empty() {
        return ColumnType::Text;
    }

    // Split off `(args)` and a trailing `=default`.
    let (base, args, default) = if let Some(open) = rest.find('(') {
        let base = &rest[..open];
        let after = &rest[open + 1..];
        if let Some(close) = after.find(')') {
            let args = &after[..close];
            let tail = after[close + 1..].trim();
            let default = tail.strip_prefix('=').map(|d| d.trim().to_string());
            (base.trim(), Some(args), default)
        } else {
            (base.trim(), Some(after), None)
        }
    } else if let Some((base, def)) = rest.split_once('=') {
        (base.trim(), None, Some(def.trim().to_string()))
    } else {
        (rest, None, None)
    };

    match base.to_lowercase().as_str() {
        "select" | "status" | "enum" | "dropdown" => {
            let labels: Vec<&str> = args
                .unwrap_or("")
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if labels.is_empty() {
                return ColumnType::Text;
            }
            let mut ct = select_from_labels(&labels);
            if let (ColumnType::Select { default: d, .. }, Some(req)) = (&mut ct, default) {
                // Honor an explicit default if it's a real value.
                if labels.iter().any(|l| *l == req) {
                    *d = Some(req);
                }
            }
            ct
        }
        "number" | "num" | "int" | "integer" | "float" => ColumnType::Number,
        "currency" | "money" | "amount" | "price" => ColumnType::Currency,
        "date" | "day" => ColumnType::Date,
        "bool" | "boolean" | "checkbox" | "check" | "done" => ColumnType::Bool,
        "person" | "owner" | "user" | "assignee" | "who" => ColumnType::Person,
        "link" | "url" | "href" => ColumnType::Link,
        _ => ColumnType::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsl_parses_types_and_select_with_default() {
        let s = TableSchema::from_dsl("title status:select(Todo,Doing,Done)=Doing owner:person due:date amount:currency done:bool");
        let names: Vec<&str> = s.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["title", "status", "owner", "due", "amount", "done"]);

        assert_eq!(s.col_type("title"), Some(&ColumnType::Text));
        assert_eq!(s.col_type("owner"), Some(&ColumnType::Person));
        assert_eq!(s.col_type("due"), Some(&ColumnType::Date));
        assert_eq!(s.col_type("amount"), Some(&ColumnType::Currency));
        assert_eq!(s.col_type("done"), Some(&ColumnType::Bool));

        match s.col_type("status").unwrap() {
            ColumnType::Select { values, default } => {
                let labels: Vec<&str> = values.iter().map(|v| v.label.as_str()).collect();
                assert_eq!(labels, vec!["Todo", "Doing", "Done"]);
                assert_eq!(default.as_deref(), Some("Doing"));
            }
            other => panic!("expected select, got {:?}", other),
        }
    }

    #[test]
    fn dsl_bare_token_is_text() {
        let s = TableSchema::from_dsl("name");
        assert_eq!(s.columns.len(), 1);
        assert_eq!(s.col_type("name"), Some(&ColumnType::Text));
    }

    #[test]
    fn sidecar_roundtrips() {
        let schema = TableSchema::new(vec![
            ColumnSpec { name: "task".into(), col_type: ColumnType::Text },
            ColumnSpec {
                name: "status".into(),
                col_type: select_from_labels(&["Todo", "Done"]),
            },
        ]);
        let dir = std::env::temp_dir().join("xeli_sidecar_test");
        std::fs::create_dir_all(&dir).unwrap();
        let data_path = dir.join("t.csv");
        let dp = data_path.to_str().unwrap();

        let written = schema.save_sidecar(dp).unwrap();
        assert!(written.ends_with("t.xeli.json"), "got {written:?}");

        let loaded = TableSchema::load_sidecar(dp).unwrap();
        assert_eq!(loaded, schema);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn select_value_lookup() {
        let ct = select_from_labels(&["Open", "Closed"]);
        assert_eq!(ct.select_value("Open").map(|v| v.label.as_str()), Some("Open"));
        assert!(ct.select_value("Nope").is_none());
        assert_eq!(ct.default_value().as_deref(), Some("Open"));
    }
}
