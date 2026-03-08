use std::io;

use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::{CrosstermBackend, Terminal},
    widgets::Paragraph,
};

fn main() -> miette::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("starting claude-seer");

    io::stdout().execute(EnterAlternateScreen).unwrap();
    enable_raw_mode().unwrap();

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout())).unwrap();
    terminal.clear().unwrap();

    loop {
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(
                    Paragraph::new("Hello, claude-seer! Press 'q' to quit."),
                    area,
                );
            })
            .unwrap();

        if let Event::Key(key) = event::read().unwrap()
            && key.code == KeyCode::Char('q')
        {
            break;
        }
    }

    disable_raw_mode().unwrap();
    io::stdout().execute(LeaveAlternateScreen).unwrap();

    Ok(())
}
