use std::path::PathBuf;
use std::sync::mpsc;

use clap::Parser;
use crossterm::event::{Event, KeyEventKind};

use claude_seer::app::{Action, AppState, SideEffect};
use claude_seer::source::DataSource;
use claude_seer::source::filesystem::FilesystemSource;
use claude_seer::tui::Tui;
use claude_seer::tui::event::{AppEvent, map_key_to_action, spawn_event_reader};
use claude_seer::tui::ui;

/// claude-seer: TUI for visualizing Claude Code session data
#[derive(Parser, Debug)]
#[command(name = "claude-seer", version, about)]
struct Cli {
    /// Path to the Claude projects directory
    /// [default: ~/.claude/projects/]
    #[arg(long, short = 'p')]
    path: Option<PathBuf>,

    /// Path to the log file for tracing output
    /// [default: /tmp/claude-seer.log]
    #[arg(long)]
    log_file: Option<PathBuf>,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    let projects_path = match cli.path {
        Some(path) => path,
        None => dirs::home_dir()
            .ok_or_else(|| miette::miette!("could not determine home directory"))?
            .join(".claude/projects"),
    };

    // Set up tracing to file.
    let log_file_path = cli
        .log_file
        .unwrap_or_else(|| PathBuf::from("/tmp/claude-seer.log"));

    // Initialize tracing — write to log file if possible, fall back to stderr.
    match std::fs::File::create(&log_file_path) {
        Ok(file) => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .with_writer(file)
                .with_ansi(false)
                .init();
        }
        Err(_) => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .with_writer(std::io::stderr)
                .init();
        }
    }

    tracing::info!("starting claude-seer");
    tracing::info!("projects path: {}", projects_path.display());

    // Initialize app state.
    let mut app = AppState::new();

    // Initialize terminal.
    let mut tui = Tui::new().map_err(|e| miette::miette!("failed to initialize terminal: {e}"))?;
    tui.terminal
        .clear()
        .map_err(|e| miette::miette!("failed to clear terminal: {e}"))?;

    // Set up event channel.
    let (tx, rx) = mpsc::channel::<AppEvent>();

    // Spawn crossterm event reader thread.
    spawn_event_reader(tx.clone());

    // Spawn background session loading.
    {
        let tx = tx.clone();
        let path = projects_path.clone();
        std::thread::spawn(move || {
            let source = FilesystemSource::new(path);
            let result = source.list_sessions();
            let _ = tx.send(AppEvent::SessionsLoaded(result));
        });
    }

    // Main event loop.
    loop {
        // Render.
        tui.terminal
            .draw(|frame| {
                ui::render(frame, &app);
            })
            .map_err(|e| miette::miette!("render error: {e}"))?;

        // Wait for next event.
        let event = match rx.recv() {
            Ok(event) => event,
            Err(_) => break, // Channel closed.
        };

        // Map event to action.
        let action = match event {
            AppEvent::Terminal(Event::Key(key)) => {
                // Only handle key press events (not release/repeat).
                if key.kind == KeyEventKind::Press {
                    map_key_to_action(key, app.show_help)
                } else {
                    None
                }
            }
            AppEvent::Terminal(Event::Resize(w, h)) => Some(Action::Resize(w, h)),
            AppEvent::Terminal(_) => None,
            AppEvent::SessionsLoaded(Ok(summaries)) => Some(Action::SessionsLoaded(summaries)),
            AppEvent::SessionsLoaded(Err(err)) => Some(Action::LoadError(err.to_string())),
            AppEvent::Tick => None,
        };

        // Process action.
        if let Some(action) = action {
            let effect = app.handle_action(action);

            // Execute side effects.
            match effect {
                Some(SideEffect::Exit) => break,
                Some(SideEffect::LoadSession(_id)) => {
                    // Will be implemented in M3 (conversation viewer).
                }
                Some(SideEffect::LoadSessionList) => {
                    let tx = tx.clone();
                    let path = projects_path.clone();
                    std::thread::spawn(move || {
                        let source = FilesystemSource::new(path);
                        let result = source.list_sessions();
                        let _ = tx.send(AppEvent::SessionsLoaded(result));
                    });
                }
                None => {}
            }
        }
    }

    // Terminal is restored by Tui::drop.
    tracing::info!("claude-seer exiting");

    Ok(())
}
