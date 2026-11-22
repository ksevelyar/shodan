use std::io;

use crossterm::event::EventStream;
use ratatui::backend::CrosstermBackend;
use shodan::application::{TerminalMode, build_application, run_application};

#[tokio::main]
async fn main() -> io::Result<()> {
    let server_address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_owned());
    let (application, interface, updates) = build_application(
        server_address,
        CrosstermBackend::new(io::stdout()),
        EventStream::new(),
        TerminalMode::Managed,
    )?;
    run_application(application, interface, updates).await
}
