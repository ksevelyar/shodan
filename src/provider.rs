use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::chats::{Message, Role};

pub enum ResponseUpdate {
    ModelSelected(String),
    Content(String),
    Finished { model: String, usage: Usage },
    Failed(String),
}

#[derive(Default, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub prompt_tokens_details: PromptTokensDetails,
}

#[derive(Default, Deserialize)]
pub struct PromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: u64,
}

#[derive(Deserialize)]
struct ResponseChunk {
    #[serde(default)]
    model: String,
    #[serde(default)]
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
        .get(format!("{}/v1/models", address.trim_end_matches('/')))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

pub async fn select_model(client: &reqwest::Client, address: &str) -> Result<String, String> {
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
        .post(format!("{}/models/load", address.trim_end_matches('/')))
        .json(&serde_json::json!({"model": model.id}))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let loading_timeout = std::time::Duration::from_secs(300);
    tokio::time::timeout(loading_timeout, async {
        loop {
            let models = list_models(client, address).await?;
            let status = models
                .data
                .iter()
                .find(|candidate| candidate.id == model.id)
                .ok_or_else(|| "model disappeared while loading".to_owned())?;
            match status.status.value.as_str() {
                "loaded" => return Ok(model.id.clone()),
                "loading" => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
                _ => return Err(format!("model failed to load: {}", status.status.value)),
            }
        }
    })
    .await
    .map_err(|_| "model loading timed out".to_owned())?
}

pub async fn stream_response(
    client: &reqwest::Client,
    address: &str,
    model: &str,
    messages: &[Message],
    updates: &mpsc::UnboundedSender<ResponseUpdate>,
) -> Result<(), String> {
    let messages = messages
        .iter()
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
        .post(format!("{}/v1/chat/completions", address.trim_end_matches('/')))
        .json(&serde_json::json!({"model": model, "messages": messages, "stream": true, "stream_options": {"include_usage": true}}))
        .send().await.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| error.to_string())?;
    let mut events = response.bytes_stream().eventsource();
    let mut model = String::new();
    let mut usage = Usage::default();
    while let Some(event) = events.next().await {
        let event = event.map_err(|error| error.to_string())?;
        if event.data == "[DONE]" {
            let _ = updates.send(ResponseUpdate::Finished { model, usage });
            return Ok(());
        }
        let chunk: ResponseChunk =
            serde_json::from_str(&event.data).map_err(|error| error.to_string())?;
        if !chunk.model.is_empty() {
            model = chunk.model;
        }
        if let Some(received_usage) = chunk.usage {
            usage = received_usage;
        }
        for choice in chunk.choices {
            if let Some(content) = choice.delta.content {
                let _ = updates.send(ResponseUpdate::Content(content));
            }
        }
    }
    Err("response stream ended before completion".to_owned())
}
