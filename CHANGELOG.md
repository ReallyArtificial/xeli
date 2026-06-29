# Changelog

All notable changes to xeli are documented here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning is [SemVer](https://semver.org/).

## [Unreleased]

## [0.3.0] — 2026-06-29

### Added
- **Create new spreadsheets — end to end.** xeli can now make a tracker from scratch, not just open existing files.
  - `xeli new` opens an interactive create screen with a live, typed-column preview (status pills and all) before you commit; press `Ctrl+K` there to let AI design the table.
  - `xeli new <template>` scaffolds one of seven canonical trackers — `tasks`, `issues`, `content`, `okrs`, `crm`, `expenses`, `quick` — plus `blank`.
  - `xeli new --columns "title status:select(Todo,Doing,Done)=Todo due:date"` builds a schema from a Rails-style `name:type` mini-DSL (scriptable, CI-friendly).
  - `xeli new --ai "bug tracker for a mobile app with severity and OS"` has the AI design the typed columns.
  - The file picker grows a "✦ Create a new table" row, and an empty folder jumps straight to create.
- **Typed columns, with the status dropdown as the centerpiece.** Columns now carry a semantic type — Text, Number, Currency, Date, Checkbox, Person, Link, and **Select** (a status/enum column that owns its value set, colors, and default). Making one is a single decision (`a` → name → type → preset/custom), not the spreadsheet ritual of free-text column + validation rule + color rules.
  - `Enter`/`i` on a status cell opens a filterable dropdown; `Space` cycles it to the next value in place.
  - Select values render as colored pills, booleans as checkboxes, blanks as empty cells (not `NULL`); numeric columns sort numerically.
  - `b` opens a **kanban board** grouped by a status column; `Enter` on a card jumps to its row.
  - `o` adds a row, `Shift+D` deletes one, `a` adds a typed column (back-filling existing rows with the default).
- **Plain-text source of truth + schema sidecar.** `Ctrl+S` writes the data as plain CSV/JSON/Parquet **plus** a small, git-diffable `*.xeli.json` sidecar that carries the column types, value sets, colors, and defaults. Reopening a file restores the typed editing experience. An unsaved-changes dot (`●`) shows in the header.
- **Excel export with real dropdowns.** Export (`e`) adds an `.xlsx` option that materializes Select columns as genuine, clickable data-validation dropdowns — the offline hand-off for non-terminal teammates.

## [0.2.0] — 2026-06-07

### Added
- **Recursive file picker**: running `xeli` with no args now searches the current directory *and subdirectories* (up to 4 levels, skipping `node_modules`, `target`, `.git`, etc.), shows paths relative to the launch dir, and displays a match count, modified-time column, and `cursor/total` position. Added `PageUp`/`PageDown`/`Home`/`End` and wrap-around navigation.
- **SQL mode discoverability**: `Ctrl+Q` now pre-fills `SELECT * FROM data` and shows the table name + column list (`table: data (…)`) so you no longer have to guess the table name. DuckDB binder errors are trimmed and, for wrong-table/column mistakes, point you at `data` and its columns.
- **Back to the full table**: after an AI/SQL/formula/group-by result, press `Esc` (or `u`, or the new *Back to Table* command) to return to the base table. Structural ops (`s`/`f`/`g`/`c`/`=`/`J`/`v`/`Ctrl+I`) snap back automatically.
- **Export respects the view**: exporting while viewing a query result now writes that result, not the base table.

### Fixed
- **Search now scans the whole table**, not just the loaded 100-row page; results are filter-aware and ordered consistently with the view.
- **Column stats & histograms respect active filters** instead of always reporting on the full table.
- **Group-by** no longer fails on column names containing spaces or parentheses (the aggregate alias is now quoted).
- **Join** no longer errors or produces ambiguous columns when both files share a column name (overlapping right-side columns get a `_2` suffix).
- Editing a cell while viewing a query result is blocked (it could corrupt the wrong base-table row); scrolling a query result past one screen now works.
- Pagination, rowid lookup, and search share one deterministic ordering, so highlights, the cursor, and cell edits always line up.
- Mouse click-to-select no longer lands on the wrong row when the filter bar is visible.
- Guarded a potential panic in filter mode with zero visible columns; removed a redundant re-query while scrolling.

## [0.1.3] — 2026-05-21

### Changed
- Republishing release after the old GitHub repo was deleted and recreated. `@josharsh/xeli@0.1.2` and `josharsh/homebrew-tap` Formula/xeli.rb both pointed to release artifacts that no longer existed. 0.1.3 is byte-identical code, fresh artifacts.
- `dist-workspace.toml`: added `npm` to `publish-jobs` so the npm registry is updated as part of every tagged release.

## [0.1.2] — 2026-05-20

### Added
- **File picker on no-args**: running `xeli` with no file argument (and no piped stdin) now opens a fuzzy-searchable picker listing supported data files in the current directory, sorted by mtime.
- **Inline AI key entry**: pressing `Ctrl+K` with no API key configured opens a two-step inline flow — pick provider (Anthropic / OpenAI), paste key (masked except last 4 chars), Enter saves to `~/.config/xeli/config.toml` and drops you straight into the AI prompt. No restart, no separate `xeli config` command.
- **Persistent key hints**: the status bar is now two lines — mode + transient message on top, always-visible key hints on the bottom (`Ctrl+K AI · Ctrl+Q SQL · / find · f filter …`). Hints are mode-aware.

### Changed
- `dist-workspace.toml`: added `npm-scope = "@josharsh"` so future cargo-dist runs produce a correctly-scoped npm tarball.

## [0.1.1] — 2026-05-20

### Fixed
- **DuckDB panic on group-by, SQL mode, formula bar, AI/join queries**: `engine::execute_query` called `stmt.column_count()` before the prepared statement had been executed, which panics in duckdb-rs 1.10501 with *"The statement was not executed yet."* Switched to the documented pattern (`query()` first, then read column info via `rows.as_ref().column_count()`). Affected every TUI feature that ran ad-hoc SQL.

## [0.1.0] — 2026-04-10

### Added
- Initial release.
- Multi-format loader: CSV, TSV, JSON, JSONL, Parquet, Excel (xlsx).
- Interactive TUI table with vim-style navigation and mouse support.
- Regex search (`/`), visual filter builder (`f`), click-to-sort (`s`).
- AI natural language → SQL via OpenAI or Anthropic (`Ctrl+K`).
- Direct DuckDB SQL mode (`Ctrl+Q`).
- Column statistics (`Ctrl+I`), histograms (`v`), formula bar (`=`), computed columns (`c`).
- Group-by + pivot wizard (`g`), join wizard (`J`).
- Export filtered data to CSV / JSON / Parquet (`e`).
- 5 themes: Dracula, Nord, Catppuccin, Tokyo Night, Solarized.
- Command palette (`Ctrl+P`), full undo stack (`u`).
- Per-platform binaries via cargo-dist: macOS arm64/x86_64, Linux arm64/x86_64.

[Unreleased]: https://github.com/josharsh/xeli/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/josharsh/xeli/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/josharsh/xeli/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/josharsh/xeli/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/josharsh/xeli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/josharsh/xeli/releases/tag/v0.1.0
