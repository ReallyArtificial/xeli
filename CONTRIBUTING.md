# Contributing to xeli

Thanks for being here. xeli is a small project run by one person; PRs and issues land faster than you'd expect.

## Quick ways to help

- **File a bug** with the steps that broke it + the file format you were using
- **Suggest a keybinding** — there's still room in the modal grammar
- **Try it on your data** and tell us what was missing
- **Star the repo** if it saved you a Google Sheets tab

## Building from source

```bash
git clone https://github.com/ReallyArtificial/xeli
cd xeli
cargo build --release
./target/release/xeli examples/employees.csv
```

Requires Rust 1.74+. The duckdb-bundled feature compiles DuckDB from source on first build — expect ~3 min.

## Running tests

```bash
# Note: release mode is required on macOS due to a libduckdb-sys/ar issue in debug builds.
cargo test --release
```

## Code layout

```
src/
├── main.rs            # CLI, terminal setup, run loop
├── app.rs             # global app state, modes, view stack
├── data/
│   ├── engine.rs      # DuckDB wrapper — load, query, transforms
│   ├── loader.rs      # file format detection, stdin handling, file picker scan
│   └── export.rs      # CSV / JSON / Parquet export
├── ai/
│   ├── client.rs      # Anthropic + OpenAI HTTP
│   ├── prompt.rs      # NL → SQL prompt template
│   └── config.rs      # ~/.config/xeli/config.toml
├── handlers/
│   ├── input.rs       # keyboard + mouse, per-mode dispatch
│   └── command.rs     # command palette wiring
└── ui/                # one file per render target (table, status, ai_bar, etc.)
```

## Open help-wanted

These are things I'd love a PR for. Hit me on Twitter ([@josharsh](https://twitter.com/josharsh)) before doing big work so we don't duplicate.

1. **Windows support** — currently broken because the bundled DuckDB build doesn't play well with MSVC. If you have ideas, please open an issue.
2. **Ollama backend** for the AI bar — local model option for users who don't want to send queries to OpenAI/Anthropic.
3. **Streaming for files larger than memory** — DuckDB can read Parquet in chunks; xeli currently loads everything.
4. **More themes** — drop one in `src/ui/theme.rs`.

## Style

- `cargo fmt` before pushing
- `cargo clippy --release -- -D warnings` should pass (a couple of pre-existing dead-code warnings are OK)
- Commits: `xeli: <short summary>` for behavior changes, `chore:`, `docs:`, `test:` for the rest
- Keep PRs scoped — one thing per PR is easier to review than five

## Code of conduct

Be decent. If something feels off, email engineering@spotlyte.live and I'll handle it.
