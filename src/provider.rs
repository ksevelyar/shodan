use crate::{
    chats::{Message, ToolCall},
    events::{Event, EventSender},
    models::{self, AvailableModel, SelectedModel, ThinkingLevel},
    tools,
};
use eventsource_stream::{Event as ServerSentEvent, Eventsource};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;

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

#[derive(Deserialize)]
struct ModelList {
    data: Vec<AvailableModel>,
}

async fn list_models(client: &reqwest::Client, address: &str) -> Result<ModelList, String> {
    client
        .get(format!("{address}/v1/models"))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

pub async fn select_model(client: &reqwest::Client, address: &str, event_sender: &EventSender) {
    let result = async {
        let models = list_models(client, address).await?;
        let (model, thinking_level) =
            models::select_model(&models.data).ok_or_else(|| "no available models".to_owned())?;
        let selected_model = SelectedModel {
            name: model.id.clone(),
            thinking_level,
        };
        if model.status.value == "loaded" {
            return Ok(selected_model);
        }
        client
            .post(format!("{address}/models/load"))
            .json(&serde_json::json!({"model": model.id}))
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        let loading_timeout = std::time::Duration::from_secs(60);

        tokio::time::timeout(loading_timeout, async {
            loop {
                let models = list_models(client, address).await?;
                let status = models
                    .data
                    .iter()
                    .find(|candidate| candidate.id == model.id)
                    .map(|candidate| candidate.status.value.as_str());

                match status {
                    Some("loaded") => return Ok(selected_model),
                    Some("loading") | None => {
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    }
                    Some(status) => return Err(format!("model failed to load: {status}")),
                }
            }
        })
        .await
        .map_err(|_| "model loading timed out".to_owned())?
    }
    .await;
    let event = match result {
        Ok(model) => Event::ModelSelected(model),
        Err(error) => Event::ModelSelectionFailed(error),
    };
    let _ = event_sender.send(event);
}

pub async fn stream_response(
    client: &reqwest::Client,
    address: &str,
    model: &str,
    thinking_level: Option<ThinkingLevel>,
    messages: &[Message],
    turn: u64,
    event_sender: &EventSender,
) {
    let result = async {
        let events = post_messages(client, address, model, thinking_level, messages).await?;
        read_provider_events(events, turn, event_sender).await
    }
    .await;
    if let Err(error) = result {
        let _ = event_sender.send(Event::RequestFailed { turn, error });
    }
}

async fn post_messages(
    client: &reqwest::Client,
    address: &str,
    model: &str,
    thinking_level: Option<ThinkingLevel>,
    messages: &[Message],
) -> Result<impl Stream<Item = Result<ServerSentEvent, String>> + Unpin, String> {
    let messages = messages
        .iter()
        .filter_map(|message| match message {
            Message::System(content) => {
                Some(serde_json::json!({"role": "system", "content": content}))
            }
            Message::User(content) => Some(serde_json::json!({"role": "user", "content": content})),
            Message::Model { content, tool_calls, aborted: false, .. } if tool_calls.is_empty() => Some(serde_json::json!({"role": "assistant", "content": content})),
            Message::Model { content, tool_calls, aborted: false, .. } => {
                let calls = tool_calls.iter().map(|call| serde_json::json!({"id": call.id, "type": "function", "function": {"name": call.name, "arguments": call.arguments}})).collect::<Vec<_>>();
                let content = if content.is_empty() { None } else { Some(content) };
                Some(serde_json::json!({"role": "assistant", "content": content, "tool_calls": calls}))
            }
            Message::Model { aborted: true, .. } | Message::FailedResponse(_) => None,
            Message::ToolResult { call_id, content } => Some(serde_json::json!({"role":"tool", "tool_call_id":call_id, "content":content})),
        })
        .collect::<Vec<_>>();
    let mut request = serde_json::json!({"model": model, "messages": messages, "tools": tools::describe_tools(), "stream": true, "stream_options": {"include_usage": true}});
    if let Some(thinking_level) = thinking_level {
        request["reasoning_effort"] = serde_json::json!(thinking_level.as_provider_value());
    }
    let response = client
        .post(format!("{address}/v1/chat/completions"))
        .json(&request)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;

    Ok(response
        .bytes_stream()
        .eventsource()
        .map(|event| event.map_err(|error| error.to_string())))
}

async fn read_provider_events(
    mut events: impl Stream<Item = Result<ServerSentEvent, String>> + Unpin,
    turn: u64,
    event_sender: &EventSender,
) -> Result<(), String> {
    let mut usage = Usage::default();
    let mut tool_calls = std::collections::BTreeMap::<usize, PendingToolCall>::new();
    let mut finish_reason = None;

    while let Some(event) = events.next().await {
        let event = event?;
        if event.data == "[DONE]" {
            if tool_calls.is_empty() && finish_reason.as_deref() == Some("tool_calls") {
                return Err("model response ended without the requested tool calls".to_owned());
            }
            if !tool_calls.is_empty() && finish_reason.as_deref() != Some("tool_calls") {
                return Err("model response ended with incomplete tool calls".to_owned());
            }
            for call in tool_calls.into_values() {
                if call.id.is_empty() || call.name.is_empty() || call.arguments.is_empty() {
                    return Err("model response contained an incomplete tool call".to_owned());
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
        let chunk: ResponseChunk =
            serde_json::from_str(&event.data).map_err(|error| error.to_string())?;

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
    Err("response stream ended before completion".to_owned())
}
