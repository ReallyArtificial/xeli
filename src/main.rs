mod ai;
mod app;
mod data;
mod event;
mod handlers;
mod ui;
mod utils;

use anyhow::{Context, Result};
use app::App;
use clap::{Parser, Subcommand};
use crossterm::{
    event::EnableMouseCapture,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use data::engine::DataEngine;
use data::loader;
use event::{AppEvent, EventHandler};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "xeli",
    about = "Excel for the Terminal — interactive TUI spreadsheet with AI-powered queries",
    version,
    after_help = "Examples:\n  xeli data.csv\n  xeli sales.json\n  cat data.csv | xeli\n  xeli data.parquet"
)]
struct Cli {
    /// File to open (CSV, JSON, Parquet, Excel)
    file: Option<String>,

    /// Theme (dracula, nord, catppuccin, tokyo-night, solarized)
    #[arg(short, long, default_value = "dracula")]
    theme: String,

    /// Disable row numbers
    #[arg(long)]
    no_row_numbers: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure xeli settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Create a new spreadsheet — interactive, or from a template / column spec / AI
    New {
        /// Template key: tasks, issues, content, okrs, crm, expenses, quick, blank.
        /// Omit for the interactive picker.
        template: Option<String>,
        /// Define columns inline, e.g. "title status:select(Todo,Doing,Done) due:date"
        #[arg(short, long)]
        columns: Option<String>,
        /// Let AI design the columns from a description
        #[arg(short, long)]
        ai: Option<String>,
        /// Output file name (default derived from the template)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Set an AI API key
    SetKey {
        /// Provider (openai or anthropic)
        provider: String,
        /// API key
        key: String,
    },
    /// Set the AI model to use
    SetModel {
        /// Model name (e.g., gpt-4o-mini, claude-sonnet-4-5-20250929)
        model: String,
    },
    /// Show current configuration
    Show,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Resolve theme up front (used by picker / create screen too)
    let theme = match cli.theme.as_str() {
        "nord" => app::Theme::Nord,
        "catppuccin" => app::Theme::Catppuccin,
        "tokyo-night" | "tokyonight" => app::Theme::TokyoNight,
        "solarized" => app::Theme::Solarized,
        _ => app::Theme::Dracula,
    };

    // Subcommands
    match cli.command {
        Some(Commands::Config { action }) => return handle_config(action),
        Some(Commands::New {
            template,
            columns,
            ai,
            output,
        }) => {
            return run_new_command(template, columns, ai, output, theme, cli.no_row_numbers).await;
        }
        None => {}
    }

    // Determine file path: CLI arg → stdin pipe → file picker in cwd.
    let file_path: String = match cli.file {
        Some(path) => path,
        None => {
            use std::io::IsTerminal;
            if !io::stdin().is_terminal() {
                // Piped input — read it.
                match loader::load_from_stdin() {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Error reading stdin: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // No arg, no pipe — open the file picker in the current directory.
                // With no data files at all, jump straight to creating one.
                let files = loader::list_data_files_in_cwd().unwrap_or_default();
                if files.is_empty() {
                    return run_new_command(None, None, None, None, theme, cli.no_row_numbers).await;
                }
                match run_file_picker(files, &theme)? {
                    Some(ui::file_picker::PickOutcome::Open(path)) => {
                        path.to_string_lossy().to_string()
                    }
                    Some(ui::file_picker::PickOutcome::Create) => {
                        return run_new_command(None, None, None, None, theme, cli.no_row_numbers)
                            .await;
                    }
                    None => return Ok(()), // user cancelled
                }
            }
        }
    };

    // Verify file exists
    if !std::path::Path::new(&file_path).exists() {
        eprintln!("Error: file not found: {}", file_path);
        std::process::exit(1);
    }

    // Detect format
    let format = loader::detect_format(&file_path)
        .with_context(|| format!("Failed to detect format of {}", file_path))?;

    // Initialize DuckDB engine and load data
    let engine = DataEngine::new()?;
    engine
        .load_file(&file_path, format.as_str())
        .with_context(|| format!("Failed to load {}", file_path))?;

    // Create app state
    let mut app = App::new(file_path.clone(), format, engine)?;

    // A `*.xeli.json` sidecar restores typed columns (pills, dropdowns, dates).
    if let Some(schema) = data::schema::TableSchema::load_sidecar(&file_path) {
        app.schema = Some(schema);
        app.auto_size_columns();
    }

    // Apply CLI options
    app.theme = theme;
    if cli.no_row_numbers {
        app.show_row_numbers = false;
    }

    launch_app(app).await
}

/// Terminal setup → event loop → teardown. Shared by the open-file and
/// create-new paths.
async fn launch_app(mut app: App) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    Ok(())
}

/// Resolve a create request (template / DSL / AI / interactive) into a schema +
/// filename, then build the in-memory table and launch the app on it.
async fn run_new_command(
    template: Option<String>,
    columns: Option<String>,
    ai: Option<String>,
    output: Option<String>,
    theme: app::Theme,
    no_row_numbers: bool,
) -> Result<()> {
    use data::schema::TableSchema;

    let (schema, filename) = if let Some(cols) = columns {
        let schema = TableSchema::from_dsl(&cols);
        if schema.columns.is_empty() {
            eprintln!("No columns parsed from --columns. Example:");
            eprintln!("  xeli new --columns \"title status:select(Todo,Doing,Done) due:date\"");
            std::process::exit(1);
        }
        (schema, output.unwrap_or_else(|| "table.csv".to_string()))
    } else if let Some(desc) = ai {
        eprintln!("✨ Designing your table with AI…");
        let schema = ai_generate_schema(&desc).await?;
        (schema, output.unwrap_or_else(|| "table.csv".to_string()))
    } else if let Some(tpl) = template {
        match data::templates::by_key(&tpl) {
            Some(schema) => {
                let fname = data::templates::all()
                    .into_iter()
                    .find(|t| t.key == tpl)
                    .map(|t| t.filename.to_string())
                    .unwrap_or_else(|| "table.csv".to_string());
                (schema, output.unwrap_or(fname))
            }
            None => {
                eprintln!(
                    "Unknown template '{}'.  Try: tasks, issues, content, okrs, crm, expenses, quick, blank",
                    tpl
                );
                std::process::exit(1);
            }
        }
    } else {
        // Interactive create screen.
        match run_create_screen(&theme)? {
            ui::create::CreateChoice::Schema(schema, fname) => (schema, output.unwrap_or(fname)),
            ui::create::CreateChoice::Ai(desc) => {
                eprintln!("✨ Designing your table with AI…");
                let schema = ai_generate_schema(&desc).await?;
                (schema, output.unwrap_or_else(|| "table.csv".to_string()))
            }
            ui::create::CreateChoice::Cancel => return Ok(()),
        }
    };

    launch_new_table(schema, filename, theme, no_row_numbers).await
}

async fn launch_new_table(
    schema: data::schema::TableSchema,
    filename: String,
    theme: app::Theme,
    no_row_numbers: bool,
) -> Result<()> {
    let engine = DataEngine::new()?;
    engine
        .create_table_from_schema(&schema, 5)
        .context("Failed to create the new table")?;

    let format = loader::detect_format(&filename).unwrap_or(loader::FileFormat::Csv);
    let mut app = App::new(filename.clone(), format, engine)?;
    app.schema = Some(schema);
    app.theme = theme;
    app.dirty = true;
    if no_row_numbers {
        app.show_row_numbers = false;
    }
    app.auto_size_columns();
    app.status_message = Some(format!(
        "New table — press Ctrl+S to save {}",
        app.filename()
    ));
    launch_app(app).await
}

async fn ai_generate_schema(description: &str) -> Result<data::schema::TableSchema> {
    let config = ai::config::AiConfig::load();
    if config.openai_api_key.is_none() && config.anthropic_api_key.is_none() {
        anyhow::bail!(
            "AI needs an API key. Set ANTHROPIC_API_KEY / OPENAI_API_KEY, or run: xeli config set-key anthropic <key>"
        );
    }
    let prompt = ai::prompt::build_schema_prompt(description);
    let raw = ai::client::query_schema(&config, &prompt)
        .await
        .context("AI request failed")?;
    data::schema::from_ai_json(&raw).context("Couldn't parse the schema the AI returned")
}

fn run_create_screen(theme: &app::Theme) -> Result<ui::create::CreateChoice> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = ui::create::run(&mut terminal, theme);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_file_picker(
    files: Vec<std::path::PathBuf>,
    theme: &app::Theme,
) -> Result<Option<ui::file_picker::PickOutcome>> {
    // Standalone TUI session — must set up and tear down independently of the
    // main app so a pick-then-quit path leaves the terminal clean.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = ui::file_picker::pick(&mut terminal, files, theme);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut events = EventHandler::new(Duration::from_millis(16));
    let ai_tx = events.sender();

    loop {
        // Update viewport dimensions
        let size = terminal.size()?;
        app.viewport_width = size.width;
        app.viewport_height = size.height;

        // Render
        terminal.draw(|f| {
            ui::render(f, app);
        })?;

        // Handle events
        if let Some(event) = events.next().await {
            match event {
                AppEvent::Key(key) => {
                    handlers::input::handle_key(app, key, &ai_tx);
                }
                AppEvent::Mouse(mouse) => {
                    handlers::input::handle_mouse(app, mouse);
                }
                AppEvent::Resize(w, h) => {
                    app.viewport_width = w;
                    app.viewport_height = h;
                }
                AppEvent::AiResponse(sql) => {
                    handlers::input::handle_ai_response(app, sql);
                }
                AppEvent::AiError(err) => {
                    handlers::input::handle_ai_error(app, err);
                }
                AppEvent::Tick | AppEvent::AiDone => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::SetKey { provider, key } => {
            let mut config = ai::config::AiConfig::load();
            match provider.as_str() {
                "openai" => {
                    config.openai_api_key = Some(key);
                    config.provider = "openai".to_string();
                }
                "anthropic" => {
                    config.anthropic_api_key = Some(key);
                    config.provider = "anthropic".to_string();
                }
                _ => {
                    eprintln!("Unknown provider: {}. Use 'openai' or 'anthropic'.", provider);
                    std::process::exit(1);
                }
            }
            config.save()?;
            println!("API key saved for {} at {:?}", provider, ai::config::AiConfig::config_path());
        }
        ConfigAction::SetModel { model } => {
            let mut config = ai::config::AiConfig::load();
            config.model = Some(model.clone());
            config.save()?;
            println!("Model set to: {}", model);
        }
        ConfigAction::Show => {
            let config = ai::config::AiConfig::load();
            println!("Provider: {}", config.provider);
            println!(
                "OpenAI key: {}",
                config.openai_api_key.as_ref().map(|k| format!("{}...{}", &k[..8.min(k.len())], &k[k.len().saturating_sub(4)..])).unwrap_or_else(|| "(not set)".to_string())
            );
            println!(
                "Anthropic key: {}",
                config.anthropic_api_key.as_ref().map(|k| format!("{}...{}", &k[..8.min(k.len())], &k[k.len().saturating_sub(4)..])).unwrap_or_else(|| "(not set)".to_string())
            );
            println!(
                "Model: {}",
                config.model.unwrap_or_else(|| "(default)".to_string())
            );
            println!("Config file: {:?}", ai::config::AiConfig::config_path());
        }
    }
    Ok(())
}
