use std::io;

use crossterm::event::EventStream;
use ratatui::backend::CrosstermBackend;
use shodan::{application::run, interface::TerminalMode, settings::build_settings};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| io::Error::other("home directory is unavailable"))?;
    let settings = build_settings(home, std::env::args().skip(1))?;
    run(settings, || {
        (
            CrosstermBackend::new(io::stdout()),
            EventStream::new(),
            TerminalMode::Managed,
        )
    })
    .await?;
    Ok(())
}
