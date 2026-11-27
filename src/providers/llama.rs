use super::ProviderConnection;
use crate::{
    application::ApplicationError,
    chats::Message,
    events::{Event, EventSender},
    models::ThinkingLevel,
    tools::{ToolCall, ToolOutput},
};
use eventsource_stream::{Event as ServerSentEvent, Eventsource};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;

pub(crate) fn get_default_definition() -> serde_json::Value {
    serde_json::json!({
        "address": "http://127.0.0.1:8080",
        "default_model": "Inkling-2-Mini-UD-IQ3_XXS",
        "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}]
    })
}

#[derive(Default, Deserialize)]
struct Usage {
    prompt_tokens: u64,
    completion_tokens: u64,
    prompt_tokens_details: PromptTokensDetails,
}

#[derive(Default, Deserialize)]
struct PromptTokensDetails {
    cached_tokens: u64,
}

#[derive(Deserialize)]
struct ResponseChunk {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolCallFragment>>,
}

#[derive(Deserialize)]
struct ToolCallFragment {
    index: usize,
    id: Option<String>,
    function: Option<FunctionFragment>,
}

#[derive(Deserialize)]
struct FunctionFragment {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Default)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

pub async fn stream_response(
    connection: &ProviderConnection,
    address: &str,
    model: &str,
    thinking_level: Option<ThinkingLevel>,
    messages: &[Message],
    turn: u64,
    event_sender: &EventSender,
) {
    let result = async {
        let events = post_messages(
            connection,
            address,
            model,
            thinking_level,
            messages,
            &connection.tools,
        )
        .await?;
        read_provider_events(events, turn, event_sender).await
    }
    .await;
    if let Err(error) = result {
        let _ = event_sender.send(Event::RequestFailed { turn, error });
    }
}

async fn post_messages(
    connection: &ProviderConnection,
    address: &str,
    model: &str,
    thinking_level: Option<ThinkingLevel>,
    messages: &[Message],
    available_tools: &[serde_json::Value],
) -> Result<impl Stream<Item = Result<ServerSentEvent, ApplicationError>> + Unpin, ApplicationError>
{
    let messages = messages
        .iter()
        .filter_map(|message| match message {
            Message::System(content) => {
                Some(serde_json::json!({"role": "system", "content": content}))
            }
            Message::User(content) => Some(serde_json::json!({"role": "user", "content": content})),
            Message::Model {
                content,
                tool_calls,
                aborted: false,
                ..
            } if tool_calls.is_empty() => {
                Some(serde_json::json!({"role": "assistant", "content": content}))
            }
            Message::Model {
                content,
                tool_calls,
                aborted: false,
                ..
            } => {
                let calls = tool_calls.iter().map(|call| serde_json::json!({"id": call.id, "type": "function", "function": {"name": call.name, "arguments": call.arguments}})).collect::<Vec<_>>();
                let content = if content.is_empty() {
                    None
                } else {
                    Some(content)
                };
                Some(
                    serde_json::json!({"role": "assistant", "content": content, "tool_calls": calls}),
                )
            }
            Message::Model { aborted: true, .. } | Message::FailedResponse(_) => None,
            Message::ToolResult { call_id, result } => {
                let content = match result {
                    Ok(ToolOutput::ReadImage { content, image_url, .. }) => serde_json::json!([
                        {"type": "text", "text": content},
                        {"type": "image_url", "image_url": {"url": image_url}}
                    ]),
                    Ok(output) => serde_json::json!(output.content()),
                    Err(error) => serde_json::json!(error),
                };
                Some(serde_json::json!({"role": "tool", "tool_call_id": call_id, "content": content}))
            }
        })
        .collect::<Vec<_>>();
    let mut request = serde_json::json!({"model": model, "messages": messages, "tools": available_tools, "stream": true, "stream_options": {"include_usage": true}});
    if let Some(thinking_level) = thinking_level {
        request["reasoning_effort"] = serde_json::json!(thinking_level.as_provider_value());
    }
    let path = "/v1/chat/completions";
    let response = connection
        .send(
            connection
                .client
                .post(format!("{address}{path}"))
                .json(&request),
        )
        .await?
        .error_for_status()
        .map_err(|error| {
            ApplicationError::Provider(format!("Provider returned an error: {error}"))
        })?;

    Ok(connection
        .record_stream(response, path)?
        .eventsource()
        .map(|event| {
            event.map_err(|error| match error {
                eventsource_stream::EventStreamError::Transport(error) => error,
                error => {
                    ApplicationError::Provider(format!("Invalid provider event stream: {error}"))
                }
            })
        }))
}

async fn read_provider_events(
    mut events: impl Stream<Item = Result<ServerSentEvent, ApplicationError>> + Unpin,
    turn: u64,
    event_sender: &EventSender,
) -> Result<(), ApplicationError> {
    let mut usage = Usage::default();
    let mut tool_calls = std::collections::BTreeMap::<usize, PendingToolCall>::new();
    let mut finish_reason = None;

    while let Some(event) = events.next().await {
        let event = event?;
        if event.data == "[DONE]" {
            if tool_calls.is_empty() && finish_reason.as_deref() == Some("tool_calls") {
                return Err(ApplicationError::Provider(
                    "model response ended without the requested tool calls".to_owned(),
                ));
            }
            if !tool_calls.is_empty() && finish_reason.as_deref() != Some("tool_calls") {
                return Err(ApplicationError::Provider(
                    "model response ended with incomplete tool calls".to_owned(),
                ));
            }
            for call in tool_calls.into_values() {
                if call.id.is_empty() || call.name.is_empty() || call.arguments.is_empty() {
                    return Err(ApplicationError::Provider(
                        "model response contained an incomplete tool call".to_owned(),
                    ));
                }
                let _ = event_sender.send(Event::ToolCallReceived {
                    turn,
                    call: ToolCall {
                        id: call.id,
                        name: call.name,
                        arguments: call.arguments,
                    },
                });
            }
            let _ = event_sender.send(Event::ResponseEnded {
                turn,
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                cached_tokens: usage.prompt_tokens_details.cached_tokens,
            });
            return Ok(());
        }
        let chunk: ResponseChunk = serde_json::from_str(&event.data).map_err(|error| {
            ApplicationError::Provider(format!("Invalid provider response: {error}"))
        })?;

        if let Some(received_usage) = chunk.usage {
            usage = received_usage;
        }

        for choice in chunk.choices {
            if let Some(reason) = choice.finish_reason {
                finish_reason = Some(reason);
            }
            if let Some(content) = choice.delta.reasoning_content {
                let _ = event_sender.send(Event::ThinkingDelta { turn, content });
            }
            if let Some(content) = choice.delta.content {
                let _ = event_sender.send(Event::MessageDelta { turn, content });
            }
            for fragment in choice.delta.tool_calls.unwrap_or_default() {
                let call = tool_calls.entry(fragment.index).or_default();
                if let Some(id) = fragment.id {
                    call.id.push_str(&id);
                }
                if let Some(function) = fragment.function {
                    if let Some(name) = function.name {
                        call.name.push_str(&name);
                    }
                    if let Some(arguments) = function.arguments {
                        call.arguments.push_str(&arguments);
                    }
                }
            }
        }
    }
    Err(ApplicationError::Provider(
        "response stream ended before completion".to_owned(),
    ))
}
