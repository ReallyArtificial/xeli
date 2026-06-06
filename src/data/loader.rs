use anyhow::{bail, Result};
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum FileFormat {
    Csv,
    Tsv,
    Json,
    JsonLines,
    Parquet,
    Excel,
}

impl FileFormat {
    pub fn as_str(&self) -> &str {
        match self {
            FileFormat::Csv => "csv",
            FileFormat::Tsv => "tsv",
            FileFormat::Json => "json",
            FileFormat::JsonLines => "jsonl",
            FileFormat::Parquet => "parquet",
            FileFormat::Excel => "xlsx",
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            FileFormat::Csv | FileFormat::Tsv => "CSV",
            FileFormat::Json | FileFormat::JsonLines => "JSON",
            FileFormat::Parquet => "PRQ",
            FileFormat::Excel => "XLS",
        }
    }
}

pub fn detect_format(path: &str) -> Result<FileFormat> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    if let Some(ext) = &ext {
        match ext.as_str() {
            "csv" => return Ok(FileFormat::Csv),
            "tsv" | "tab" => return Ok(FileFormat::Tsv),
            "json" => return Ok(FileFormat::Json),
            "jsonl" | "ndjson" => return Ok(FileFormat::JsonLines),
            "parquet" | "pq" => return Ok(FileFormat::Parquet),
            "xlsx" | "xls" => return Ok(FileFormat::Excel),
            _ => {}
        }
    }

    // Try magic bytes
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 8];
    let n = file.read(&mut buf)?;

    if n >= 4 && &buf[0..4] == b"PAR1" {
        return Ok(FileFormat::Parquet);
    }
    if n >= 4 && &buf[0..4] == [0x50, 0x4B, 0x03, 0x04] {
        return Ok(FileFormat::Excel);
    }

    // Check if it looks like JSON
    let mut content = String::new();
    file = std::fs::File::open(path)?;
    file.read_to_string(&mut content)?;
    let trimmed = content.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        if trimmed.contains('\n') && !trimmed.starts_with('[') {
            return Ok(FileFormat::JsonLines);
        }
        return Ok(FileFormat::Json);
    }

    // Check for TSV vs CSV
    let first_line = content.lines().next().unwrap_or("");
    if first_line.contains('\t') && !first_line.contains(',') {
        return Ok(FileFormat::Tsv);
    }

    // Default to CSV
    Ok(FileFormat::Csv)
}

fn is_supported_ext(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    matches!(
        ext.as_deref(),
        Some("csv") | Some("tsv") | Some("tab") | Some("json") | Some("jsonl")
        | Some("ndjson") | Some("parquet") | Some("pq") | Some("xlsx") | Some("xls")
    )
}

/// List supported data files under the current working directory, recursing up
/// to a few levels deep, sorted by most-recently-modified first. Hidden entries
/// (dotfiles) and heavy build/dependency directories are skipped so the picker
/// stays fast and relevant.
pub fn list_data_files_in_cwd() -> Result<Vec<std::path::PathBuf>> {
    list_data_files_under(&std::env::current_dir()?)
}

fn list_data_files_under(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    const MAX_DEPTH: usize = 4;
    const MAX_FILES: usize = 2000;
    const SKIP_DIRS: &[&str] = &[
        "node_modules", "target", "__pycache__", "venv", ".venv",
        "dist", "build", ".git", "vendor", ".next",
    ];

    let mut entries: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue, // unreadable dir (permissions) — skip silently
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if name.starts_with('.') {
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if file_type.is_dir() {
                if depth < MAX_DEPTH && !SKIP_DIRS.contains(&name) {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if !is_supported_ext(&path) {
                continue;
            }
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            entries.push((path, mtime));
        }
        if entries.len() >= MAX_FILES {
            break;
        }
    }

    entries.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(entries.into_iter().map(|(p, _)| p).collect())
}

pub fn load_from_stdin() -> Result<String> {
    use std::io::{self, IsTerminal};

    if io::stdin().is_terminal() {
        bail!("No file specified and stdin is a terminal. Usage: xeli <file>");
    }

    let mut content = String::new();
    io::stdin().read_to_string(&mut content)?;

    // Write to temp file
    let tmp = std::env::temp_dir().join("xeli_stdin.csv");
    std::fs::write(&tmp, &content)?;

    Ok(tmp.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_nested_data_files_and_skips_noise() {
        // Build an isolated temp tree: a top-level CSV, a nested Parquet, a
        // dotfile, an unsupported file, and a file buried inside node_modules.
        let root = std::env::temp_dir().join("xeli_picker_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();

        std::fs::write(root.join("top.csv"), "a,b\n1,2\n").unwrap();
        std::fs::write(root.join("data/sales.parquet"), "PAR1").unwrap();
        std::fs::write(root.join(".hidden.csv"), "x\n").unwrap();
        std::fs::write(root.join("notes.txt"), "ignore me\n").unwrap();
        std::fs::write(root.join("node_modules/pkg/dep.csv"), "x\n").unwrap();

        let found = list_data_files_under(&root).unwrap();
        let names: Vec<String> = found
            .iter()
            .map(|p| p.strip_prefix(&root).unwrap().to_string_lossy().to_string())
            .collect();

        assert!(names.contains(&"top.csv".to_string()), "got: {:?}", names);
        assert!(
            names.contains(&"data/sales.parquet".to_string()),
            "nested file missing: {:?}",
            names
        );
        assert!(!names.iter().any(|n| n.contains(".hidden")), "dotfile leaked");
        assert!(!names.iter().any(|n| n.ends_with(".txt")), "unsupported leaked");
        assert!(
            !names.iter().any(|n| n.contains("node_modules")),
            "node_modules not skipped"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
