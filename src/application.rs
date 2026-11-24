use std::{io, time::Duration};

use crossterm::event::Event;
use futures_util::Stream;
use ratatui::backend::Backend;
use tokio::{sync::mpsc, time::Instant};

use crate::{
    chats::{self, Message, build_chat},
    events::{self, EventSender},
    interface::{self, build_interface},
    provider,
};

pub use crate::{
    events::Events,
    interface::{Interface, TerminalMode},
};

#[derive(Default)]
pub(crate) struct Statistics {
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
    pub model: String,
    pub response_time: Option<Duration>,
    pub response_started: Option<Instant>,
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
    pub(crate) event_sender: EventSender,
    pub(crate) active_request: Option<tokio::task::JoinHandle<()>>,
    pub(crate) turn: u64,
    pub(crate) turn_start: usize,
    pub(crate) pending_tool_calls: std::collections::VecDeque<crate::chats::ToolCall>,
}

#[doc = "# Errors\nReturns an error if the terminal cannot be initialized."]
pub fn build_application<BackendType: Backend, EventSource>(
    server_address: String,
    backend: BackendType,
    events: EventSource,
    terminal_mode: TerminalMode,
) -> io::Result<(Application, Interface<BackendType, EventSource>, Events)>
where
    BackendType::Error: Send + Sync + 'static,
{
    let messages = build_chat();
    let (event_sender, background_events) = mpsc::unbounded_channel();
    let interface = build_interface(backend, events, terminal_mode)?;
    let application = Application {
        server_address,
        client: reqwest::Client::new(),
        messages,
        statistics: Statistics::default(),
        error: None,
        event_sender,
        active_request: None,
        turn: 0,
        turn_start: 0,
        pending_tool_calls: std::collections::VecDeque::new(),
        input: String::new(),
        scroll_from_bottom: 0,
        maximum_scroll: 0,
    };
    Ok((application, interface, background_events))
}

#[doc = "# Errors\nReturns an error if terminal input or rendering fails."]
pub async fn run_application<BackendType, EventSource>(
    mut application: Application,
    mut interface: Interface<BackendType, EventSource>,
    mut background_events: Events,
) -> io::Result<()>
where
    BackendType: Backend,
    BackendType::Error: Send + Sync + 'static,
    EventSource: Stream<Item = io::Result<Event>> + Unpin,
{
    let client = application.client.clone();
    let address = application.server_address.clone();
    let event_sender = application.event_sender.clone();
    tokio::spawn(async move {
        crate::provider::select_model(&client, &address, &event_sender).await;
    });
    let result =
        events::run_event_loop(&mut application, &mut interface, &mut background_events).await;
    if let Some(request) = application.active_request.take() {
        request.abort();
    }
    let restoration = interface::restore_terminal(&mut interface);
    result.and(restoration)
}

pub(crate) fn start_response(application: &mut Application, messages: Vec<Message>) {
    let client = application.client.clone();
    let address = application.server_address.clone();
    let model = application.statistics.model.clone();
    let event_sender = application.event_sender.clone();
    let turn = application.turn;
    application.active_request = Some(tokio::spawn(async move {
        provider::stream_response(&client, &address, &model, &messages, turn, &event_sender).await;
    }));
}

pub(crate) fn abort_response(application: &mut Application) {
    let Some(request) = application.active_request.take() else {
        return;
    };
    request.abort();
    application.turn = application.turn.wrapping_add(1);
    application.pending_tool_calls.clear();
    application.statistics.response_started = None;
    chats::mark_response_aborted(application);
}
