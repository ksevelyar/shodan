use eventsource_stream::{Event, Eventsource};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::chats::{Message, Role};

pub enum ProviderEvent {
    ModelSelected(String),
    MessageDelta(String),
    ResponseEnded {
        prompt_tokens: u64,
        completion_tokens: u64,
        cached_tokens: u64,
    },
    Error(String),
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
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<AvailableModel>,
}

#[derive(Deserialize)]
struct AvailableModel {
    id: String,
    status: ModelStatus,
}

#[derive(Deserialize)]
struct ModelStatus {
    value: String,
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

pub async fn select_model(
    client: &reqwest::Client,
    address: &str,
    updates: &mpsc::UnboundedSender<ProviderEvent>,
) {
    let result = async {
        let models = list_models(client, address).await?;
        if let Some(model) = models
            .data
            .iter()
            .find(|model| model.status.value == "loaded")
        {
            return Ok(model.id.clone());
        }

        let model = models
            .data
            .iter()
            .find(|model| model.status.value == "unloaded")
            .ok_or_else(|| "no available models".to_owned())?;
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
                    Some("loaded") => return Ok(model.id.clone()),
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
        Ok(model) => ProviderEvent::ModelSelected(model),
        Err(error) => ProviderEvent::Error(error),
    };
    let _ = updates.send(event);
}

pub async fn stream_response(
    client: &reqwest::Client,
    address: &str,
    model: &str,
    messages: &[Message],
    updates: &mpsc::UnboundedSender<ProviderEvent>,
) {
    let result = async {
        let events = post_messages(client, address, model, messages).await?;
        read_provider_events(events, updates).await
    }
    .await;
    if let Err(error) = result {
        let _ = updates.send(ProviderEvent::Error(error));
    }
}

async fn post_messages(
    client: &reqwest::Client,
    address: &str,
    model: &str,
    messages: &[Message],
) -> Result<impl Stream<Item = Result<Event, String>> + Unpin, String> {
    let messages = messages
        .iter()
        .filter(|message| !message.aborted)
        .map(|message| {
            let role = match message.role {
                Role::System => "system",
                Role::User => "user",
                Role::Model => "assistant",
            };
            serde_json::json!({"role": role, "content": message.content})
        })
        .collect::<Vec<_>>();
    let response = client
        .post(format!("{address}/v1/chat/completions"))
        .json(&serde_json::json!({"model": model, "messages": messages, "stream": true, "stream_options": {"include_usage": true}}))
        .send().await.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| error.to_string())?;

    Ok(response
        .bytes_stream()
        .eventsource()
        .map(|event| event.map_err(|error| error.to_string())))
}

async fn read_provider_events(
    mut events: impl Stream<Item = Result<Event, String>> + Unpin,
    updates: &mpsc::UnboundedSender<ProviderEvent>,
) -> Result<(), String> {
    let mut usage = Usage::default();

    while let Some(event) = events.next().await {
        let event = event?;
        if event.data == "[DONE]" {
            let _ = updates.send(ProviderEvent::ResponseEnded {
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
            if let Some(content) = choice.delta.content {
                let _ = updates.send(ProviderEvent::MessageDelta(content));
            }
        }
    }
    Err("response stream ended before completion".to_owned())
}
