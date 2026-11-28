use std::collections::VecDeque;

use tokio::time::Instant;

use crate::{application::Application, events::Event, providers, tools};

use crate::tools::ToolCall;

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Message {
    System(String),
    User(String),
    Model {
        content: String,
        thinking: String,
        tool_calls: Vec<ToolCall>,
        provider_items: Vec<serde_json::Value>,
        aborted: bool,
    },
    ToolResult {
        call_id: String,
        result: Result<tools::ToolOutput, String>,
    },
    FailedResponse(String),
}

pub(crate) fn build_chat(prompt: String) -> Vec<Message> {
    vec![Message::System(prompt)]
}

fn append_model_message(application: &mut Application) {
    application.session.messages.push(Message::Model {
        content: String::new(),
        thinking: String::new(),
        tool_calls: Vec::new(),
        provider_items: Vec::new(),
        aborted: false,
    });
}

pub(crate) fn send_message(application: &mut Application, content: String) {
    application.session.turn = application.session.turn.wrapping_add(1);
    application.session.messages.push(Message::User(content));
    application.error = None;
    if let Err(error) = application.session.save(true) {
        application.error = Some(error);
        return;
    }
    application.session.turn_start = application.session.messages.len();
    let messages = application.session.messages.clone();
    append_model_message(application);
    application.response_started = Some(Instant::now());
    start_response(application, messages);
}

fn take_partial_response(application: &mut Application) -> (String, String) {
    let mut partial_thinking = String::new();
    let mut partial_response = String::new();
    for message in application
        .session
        .messages
        .drain(application.session.turn_start..)
    {
        if let Message::Model {
            thinking, content, ..
        } = message
        {
            if !thinking.is_empty() {
                partial_thinking.push_str(&thinking);
                partial_thinking.push('\n');
            }
            partial_response.push_str(&content);
        }
    }
    (partial_thinking, partial_response)
}

fn mark_response_aborted(application: &mut Application) {
    let (partial_thinking, partial_response) = take_partial_response(application);
    application.session.messages.push(Message::Model {
        content: partial_response,
        thinking: partial_thinking.trim_end_matches('\n').to_owned(),
        tool_calls: Vec::new(),
        provider_items: Vec::new(),
        aborted: true,
    });
}

fn start_response(application: &mut Application, messages: Vec<Message>) {
    let mut provider = application.session.provider.clone();
    let mut available_tools =
        tools::describe_tools(application.session.get_context_name() == "read");
    if let Some(connection) = &application.blender_connection {
        available_tools.extend(connection.definitions("blender_"));
    }
    provider.connection.tools = available_tools;
    let selected_model = application.session.get_selected_model();
    let model = selected_model.name.clone();
    let thinking_level = selected_model.thinking_level;
    let event_sender = application.event_sender.clone();
    let turn = application.session.turn;
    application.active_request = Some(tokio::spawn(async move {
        providers::stream_response(
            &provider,
            &model,
            thinking_level,
            &messages,
            turn,
            &event_sender,
        )
        .await;
    }));
}

pub(crate) fn abort_response(application: &mut Application) {
    let Some(request) = application.active_request.take() else {
        return;
    };
    request.abort();
    application.session.turn = application.session.turn.wrapping_add(1);
    application.pending_tool_calls.clear();
    application.response_started = None;
    mark_response_aborted(application);
    if let Err(error) = application.session.save(false) {
        application.error = Some(error);
    }
}

fn start_next_tool(application: &mut Application) {
    let Some(call) = application.pending_tool_calls.pop_front() else {
        let messages = application.session.messages.clone();
        append_model_message(application);
        start_response(application, messages);
        return;
    };
    let turn = application.session.turn;
    let event_sender = application.event_sender.clone();
    let allowed = tools::is_available(&call.name, application.session.get_context_name() == "read");
    let connection = application.blender_connection.clone();
    application.active_request = Some(tokio::spawn(async move {
        let original_name = call.name.strip_prefix("blender_");
        let result = if let (Some(connection), Some(name)) = (
            connection.as_ref(),
            original_name.filter(|name| {
                connection
                    .as_ref()
                    .is_some_and(|connection| connection.contains(name))
            }),
        ) {
            let (content, successful) = connection.call(name, &call.arguments).await;
            if successful {
                Ok(tools::ToolOutput::Text { content })
            } else {
                Err(content)
            }
        } else if allowed {
            tools::execute(&call.name, &call.arguments).await
        } else {
            Err("Error: tool unavailable in read context".to_owned())
        };
        let _ = event_sender.send(Event::ToolCompleted {
            turn,
            call_id: call.id,
            result,
        });
    }));
}

fn finish_login(application: &mut Application) {
    application.active_request = None;
    application.pending_authorization = None;
    application.session.clear_messages();
    application.mode = crate::settings::Mode::Interactive;
}

fn save_provider_items(application: &mut Application, items: Vec<serde_json::Value>) {
    if let Some(Message::Model { provider_items, .. }) = application.session.messages.last_mut() {
        *provider_items = items;
    }
}

fn save_session(application: &mut Application) {
    if let Err(error) = application.session.save(false) {
        application.error = Some(error);
    }
}

fn handle_failed_request(
    application: &mut Application,
    error: crate::application::ApplicationError,
) {
    application.active_request = None;
    application.error = Some(error);
    application.response_started = None;
    let (partial_thinking, partial_response) = take_partial_response(application);
    application
        .session
        .messages
        .push(Message::FailedResponse(format!(
            "{partial_thinking}{partial_response}"
        )));
    save_session(application);
}

pub(crate) fn apply_event(application: &mut Application, event: Event) {
    match event {
        Event::LoginMessage(message) => application
            .session
            .messages
            .push(Message::FailedResponse(message)),
        Event::LoginSucceeded => finish_login(application),
        Event::LoginFailed(error) => {
            application.active_request = None;
            application.pending_authorization = None;
            application
                .session
                .messages
                .push(Message::FailedResponse(error));
        }
        Event::MessageDelta { turn, content }
            if turn == application.session.turn && application.response_started.is_some() =>
        {
            let Some(Message::Model {
                content: message, ..
            }) = application.session.messages.last_mut()
            else {
                return;
            };
            message.push_str(&content);
        }
        Event::ThinkingDelta { turn, content }
            if turn == application.session.turn && application.response_started.is_some() =>
        {
            let Some(Message::Model { thinking, .. }) = application.session.messages.last_mut()
            else {
                return;
            };
            thinking.push_str(&content);
        }
        Event::ToolCallReceived { turn, call }
            if turn == application.session.turn && application.response_started.is_some() =>
        {
            let Some(Message::Model { tool_calls, .. }) = application.session.messages.last_mut()
            else {
                return;
            };
            tool_calls.push(call);
        }
        Event::ProviderItemsReceived { turn, items }
            if turn == application.session.turn && application.response_started.is_some() =>
        {
            save_provider_items(application, items);
        }
        Event::ResponseEnded {
            turn,
            prompt_tokens,
            completion_tokens,
            cached_tokens,
        } if turn == application.session.turn && application.response_started.is_some() => {
            application.active_request = None;
            application.session.statistics.prompt_tokens += prompt_tokens;
            application.session.statistics.cached_tokens += cached_tokens;
            application.session.statistics.completion_tokens += completion_tokens;
            let Some(Message::Model { tool_calls, .. }) = application.session.messages.last()
            else {
                save_session(application);
                return;
            };
            if tool_calls.is_empty() {
                application.session.statistics.response_time = application
                    .response_started
                    .take()
                    .map(|started| started.elapsed());
                save_session(application);
            } else {
                application.pending_tool_calls = VecDeque::from(tool_calls.clone());
                save_session(application);
                start_next_tool(application);
            }
        }
        Event::ToolCompleted {
            turn,
            call_id,
            result,
        } if turn == application.session.turn && application.response_started.is_some() => {
            application.active_request = None;
            application
                .session
                .messages
                .push(Message::ToolResult { call_id, result });
            save_session(application);
            start_next_tool(application);
        }
        Event::RequestFailed { turn, error }
            if turn == application.session.turn && application.response_started.is_some() =>
        {
            handle_failed_request(application, error);
        }
        _ => {}
    }
}
