use std::{io, time::Duration};

use crossterm::event::Event as TerminalEvent;
use futures_util::Stream;
use ratatui::backend::Backend;
use tokio::sync::mpsc;

use crate::{
    application::Application,
    chats::{self, ToolCall},
    interface::{self, Interface},
};

pub enum Event {
    ModelSelected(String),
    MessageDelta {
        turn: u64,
        content: String,
    },
    ThinkingDelta {
        turn: u64,
        content: String,
    },
    ToolCallReceived {
        turn: u64,
        call: ToolCall,
    },
    ResponseEnded {
        turn: u64,
        prompt_tokens: u64,
        completion_tokens: u64,
        cached_tokens: u64,
    },
    ToolCompleted {
        turn: u64,
        call_id: String,
        content: String,
    },
    RequestFailed {
        turn: u64,
        error: String,
    },
    ModelSelectionFailed(String),
}

pub type Events = mpsc::UnboundedReceiver<Event>;
pub(crate) type EventSender = mpsc::UnboundedSender<Event>;

pub(crate) async fn run_event_loop<BackendType, EventSource>(
    application: &mut Application,
    interface: &mut Interface<BackendType, EventSource>,
    events: &mut Events,
) -> io::Result<()>
where
    BackendType: Backend,
    BackendType::Error: Send + Sync + 'static,
    EventSource: Stream<Item = io::Result<TerminalEvent>> + Unpin,
{
    loop {
        interface::draw(interface, application)?;
        tokio::select! {
            input = interface::wait_for_input(interface) => {
                if interface::handle_input(application, input?).is_break() {
                    return Ok(());
                }
            }
            event = events.recv() => {
                let event = event.ok_or_else(|| io::Error::other("event channel closed"))?;
                chats::apply_event(application, event);
            }
            () = tokio::time::sleep(Duration::from_millis(80)), if application.statistics.response_started.is_some() => {}
        }
    }
}
