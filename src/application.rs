use std::{
    io,
    time::{Duration, Instant},
};

use crossterm::event::Event;
use futures_util::Stream;
use ratatui::backend::Backend;
use tokio::sync::mpsc;

use crate::{
    chats::{self, Message, build_chat},
    interface::{self, build_interface},
    provider::ResponseUpdate,
};

pub use crate::{
    chats::ChatUpdates,
    interface::{Interface, TerminalMode},
};

#[derive(Default)]
pub(crate) struct Statistics {
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
    pub model: String,
    pub response_time: Option<Duration>,
}

pub struct Application {
    pub(crate) input: String,
    pub(crate) scroll_from_bottom: usize,
    pub(crate) maximum_scroll: usize,
    pub(crate) server_address: String,
    pub(crate) client: reqwest::Client,
    pub(crate) messages: Vec<Message>,
    pub(crate) statistics: Statistics,
    pub(crate) error: Option<String>,
    pub(crate) update_sender: mpsc::UnboundedSender<ResponseUpdate>,
    pub(crate) response_started: Option<Instant>,
}

#[doc = "# Errors\nReturns an error if the terminal cannot be initialized."]
pub fn build_application<BackendType: Backend, EventSource>(
    server_address: String,
    backend: BackendType,
    events: EventSource,
    terminal_mode: TerminalMode,
) -> io::Result<(
    Application,
    Interface<BackendType, EventSource>,
    ChatUpdates,
)>
where
    BackendType::Error: Send + Sync + 'static,
{
    let (messages, update_sender, updates) = build_chat();
    let interface = build_interface(backend, events, terminal_mode)?;
    let application = Application {
        server_address,
        client: reqwest::Client::new(),
        messages,
        statistics: Statistics::default(),
        error: None,
        update_sender,
        response_started: None,
        input: String::new(),
        scroll_from_bottom: 0,
        maximum_scroll: 0,
    };
    Ok((application, interface, updates))
}

#[doc = "# Errors\nReturns an error if terminal input or rendering fails."]
pub async fn run_application<BackendType, EventSource>(
    mut application: Application,
    mut interface: Interface<BackendType, EventSource>,
    mut updates: ChatUpdates,
) -> io::Result<()>
where
    BackendType: Backend,
    BackendType::Error: Send + Sync + 'static,
    EventSource: Stream<Item = io::Result<Event>> + Unpin,
{
    let result = run_event_loop(&mut application, &mut interface, &mut updates).await;
    let restoration = interface::restore_terminal(&mut interface);
    result.and(restoration)
}

async fn run_event_loop<BackendType, EventSource>(
    application: &mut Application,
    interface: &mut Interface<BackendType, EventSource>,
    updates: &mut ChatUpdates,
) -> io::Result<()>
where
    BackendType: Backend,
    BackendType::Error: Send + Sync + 'static,
    EventSource: Stream<Item = io::Result<Event>> + Unpin,
{
    loop {
        interface::draw(interface, application)?;
        tokio::select! {
            input = interface::wait_for_input(interface) => {
                if interface::handle_input(application, input?).is_break() {
                    return Ok(());
                }
            }
            update = chats::wait_for_update(updates) => {
                chats::apply_update(application, update?);
            }
        }
    }
}
