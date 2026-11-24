use std::collections::VecDeque;

use tokio::time::Instant;

use crate::{
    application::{Application, start_response},
    events::Event,
    tools,
};

#[derive(Clone)]
pub struct ToolCall {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Clone)]
pub(crate) enum Message {
    System(String),
    User(String),
    Model {
        content: String,
        thinking: String,
        tool_calls: Vec<ToolCall>,
        aborted: bool,
    },
    ToolResult {
        call_id: String,
        content: String,
    },
    FailedResponse(String),
}

pub(crate) fn build_chat() -> Vec<Message> {
    vec![Message::System("Answer directly and accurately using the fewest words needed. For practical requests, give specific, actionable steps. Omit introductions, summaries, repetition, and unsolicited advice. Add detail only when needed or requested.".to_owned())]
}

fn append_model_message(application: &mut Application) {
    application.messages.push(Message::Model {
        content: String::new(),
        thinking: String::new(),
        tool_calls: Vec::new(),
        aborted: false,
    });
}

pub(crate) fn send_message(application: &mut Application, content: String) {
    application.turn = application.turn.wrapping_add(1);
    application.messages.push(Message::User(content));
    application.turn_start = application.messages.len();
    let messages = application.messages.clone();
    append_model_message(application);
    application.statistics.response_started = Some(Instant::now());
    application.error = None;
    start_response(application, messages);
}

fn take_partial_response(application: &mut Application) -> String {
    let mut partial_response = String::new();
    for message in application.messages.drain(application.turn_start..) {
        if let Message::Model {
            thinking, content, ..
        } = message
        {
            if !thinking.is_empty() {
                partial_response.push_str("Thinking:\n");
                partial_response.push_str(&thinking);
                partial_response.push('\n');
            }
            partial_response.push_str(&content);
        }
    }
    partial_response
}

pub(crate) fn mark_response_aborted(application: &mut Application) {
    let partial_response = take_partial_response(application);
    application.messages.push(Message::Model {
        content: partial_response,
        thinking: String::new(),
        tool_calls: Vec::new(),
        aborted: true,
    });
}

fn start_next_tool(application: &mut Application) {
    let Some(call) = application.pending_tool_calls.pop_front() else {
        let messages = application.messages.clone();
        append_model_message(application);
        start_response(application, messages);
        return;
    };
    let turn = application.turn;
    let event_sender = application.event_sender.clone();
    application.active_request = Some(tokio::spawn(async move {
        let content = tools::execute(&call.name, &call.arguments).await;
        let _ = event_sender.send(Event::ToolCompleted {
            turn,
            call_id: call.id,
            content,
        });
    }));
}

pub(crate) fn apply_event(application: &mut Application, event: Event) {
    match event {
        Event::ModelSelected(model) => application.statistics.model = model,
        Event::ModelSelectionFailed(error) => application.error = Some(error),
        Event::MessageDelta { turn, content }
            if turn == application.turn && application.statistics.response_started.is_some() =>
        {
            let Some(Message::Model {
                content: message, ..
            }) = application.messages.last_mut()
            else {
                return;
            };
            message.push_str(&content);
        }
        Event::ThinkingDelta { turn, content }
            if turn == application.turn && application.statistics.response_started.is_some() =>
        {
            let Some(Message::Model { thinking, .. }) = application.messages.last_mut() else {
                return;
            };
            thinking.push_str(&content);
        }
        Event::ToolCallReceived { turn, call }
            if turn == application.turn && application.statistics.response_started.is_some() =>
        {
            let Some(Message::Model { tool_calls, .. }) = application.messages.last_mut() else {
                return;
            };
            tool_calls.push(call);
        }
        Event::ResponseEnded {
            turn,
            prompt_tokens,
            completion_tokens,
            cached_tokens,
        } if turn == application.turn && application.statistics.response_started.is_some() => {
            application.active_request = None;
            application.statistics.prompt_tokens += prompt_tokens;
            application.statistics.cached_tokens += cached_tokens;
            application.statistics.completion_tokens += completion_tokens;
            let Some(Message::Model { tool_calls, .. }) = application.messages.last() else {
                return;
            };
            if tool_calls.is_empty() {
                application.statistics.response_time = application
                    .statistics
                    .response_started
                    .take()
                    .map(|started| started.elapsed());
            } else {
                application.pending_tool_calls = VecDeque::from(tool_calls.clone());
                start_next_tool(application);
            }
        }
        Event::ToolCompleted {
            turn,
            call_id,
            content,
        } if turn == application.turn && application.statistics.response_started.is_some() => {
            application.active_request = None;
            application
                .messages
                .push(Message::ToolResult { call_id, content });
            start_next_tool(application);
        }
        Event::RequestFailed { turn, error }
            if turn == application.turn && application.statistics.response_started.is_some() =>
        {
            application.active_request = None;
            application.error = Some(error);
            application.statistics.response_started = None;
            let partial_response = take_partial_response(application);
            application
                .messages
                .push(Message::FailedResponse(partial_response));
        }
        _ => {}
    }
}
