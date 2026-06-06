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

    // Handle config subcommands
    if let Some(Commands::Config { action }) = cli.command {
        return handle_config(action);
    }

    // Resolve theme up front (used by picker too)
    let theme = match cli.theme.as_str() {
        "nord" => app::Theme::Nord,
        "catppuccin" => app::Theme::Catppuccin,
        "tokyo-night" | "tokyonight" => app::Theme::TokyoNight,
        "solarized" => app::Theme::Solarized,
        _ => app::Theme::Dracula,
    };

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
                let files = loader::list_data_files_in_cwd().unwrap_or_default();
                if files.is_empty() {
                    eprintln!("No supported data files found here (searched this folder and subfolders).");
                    eprintln!("Usage:  xeli <file>");
                    eprintln!("        cat data.csv | xeli");
                    std::process::exit(1);
                }
                match run_file_picker(files, &theme)? {
                    Some(path) => path.to_string_lossy().to_string(),
                    None => {
                        // User cancelled.
                        return Ok(());
                    }
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
    let mut app = App::new(file_path, format, engine)?;

    // Apply CLI options
    app.theme = theme;
    if cli.no_row_numbers {
        app.show_row_numbers = false;
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Run event loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
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

fn run_file_picker(
    files: Vec<std::path::PathBuf>,
    theme: &app::Theme,
) -> Result<Option<std::path::PathBuf>> {
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
