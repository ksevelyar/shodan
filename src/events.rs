use std::{io, time::Duration};

use crossterm::event::Event as TerminalEvent;
use futures_util::Stream;
use ratatui::backend::Backend;
use tokio::sync::mpsc;

use crate::{
    application::{Application, ApplicationError},
    chats::{self, Message},
    interface::{self, Interface},
    tools::{ToolCall, ToolOutput},
};

pub enum Event {
    LoginMessage(String),
    LoginSucceeded,
    LoginFailed(String),
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
    ProviderItemsReceived {
        turn: u64,
        items: Vec<serde_json::Value>,
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
        result: Result<ToolOutput, String>,
    },
    RequestFailed {
        turn: u64,
        error: ApplicationError,
    },
}

pub type Events = mpsc::UnboundedReceiver<Event>;
pub(crate) type EventSender = mpsc::UnboundedSender<Event>;

pub(crate) async fn run_single_prompt_loop(
    application: &mut Application,
    background_events: &mut Events,
) -> Result<String, ApplicationError> {
    while application.response_started.is_some() {
        let request = application
            .active_request
            .as_mut()
            .expect("an active response has an active request");
        let request_identifier = request.id();
        if let Err(error) = request.await {
            if error.is_panic() {
                std::panic::resume_unwind(error.into_panic());
            }
            panic!("active request was cancelled unexpectedly: {error}");
        }
        while let Ok(event) = background_events.try_recv() {
            chats::apply_event(application, event);
            if application
                .active_request
                .as_ref()
                .is_none_or(|request| request.id() != request_identifier)
            {
                break;
            }
        }
        if application.response_started.is_some()
            && application
                .active_request
                .as_ref()
                .is_none_or(|request| request.id() == request_identifier)
        {
            panic!("request ended without completing its work");
        }
    }
    if let Some(error) = application.error.take() {
        return Err(error);
    }
    match application.session.messages.last() {
        Some(Message::Model { content, .. }) => Ok(content.clone()),
        _ => panic!("completed conversation has a final model response"),
    }
}

pub(crate) async fn run_interactive_loop<BackendType, EventSource>(
    application: &mut Application,
    interface: &mut Interface<BackendType, EventSource>,
    events: &mut Events,
) -> Result<(), ApplicationError>
where
    BackendType: Backend,
    BackendType::Error: Send + Sync + 'static,
    EventSource: Stream<Item = io::Result<TerminalEvent>> + Unpin,
{
    loop {
        interface::draw(
            interface,
            &application.session,
            application.error.as_ref(),
            application.response_started,
        )?;
        tokio::select! {
            input = interface::wait_for_input(interface) => {
                let action = interface::handle_input(interface, input?, &application.session.messages);
                if crate::application::handle_action(application, interface, action) {
                    return Ok(());
                }
            }
            event = events.recv() => {
                let event = event.expect("the application holds an event sender");
                let login_succeeded = matches!(event, Event::LoginSucceeded);
                chats::apply_event(application, event);
                if login_succeeded {
                    interface::finish_login(interface);
                }
            }
            () = tokio::time::sleep(Duration::from_millis(80)), if application.response_started.is_some() || interface::is_selection_dragging(interface) => {
                interface::advance_selection_scroll(interface);
            }
        }
    }
}
