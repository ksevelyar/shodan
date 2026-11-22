use std::{io, time::Instant};

use tokio::sync::mpsc;

use crate::{
    application::Application,
    provider::{ResponseUpdate, stream_response},
};

#[derive(Clone)]
pub(crate) enum Role {
    System,
    User,
    Model,
}

#[derive(Clone)]
pub(crate) struct Message {
    pub(crate) role: Role,
    pub(crate) content: String,
}

pub type ChatUpdates = mpsc::UnboundedReceiver<ResponseUpdate>;

pub(crate) fn build_chat() -> (
    Vec<Message>,
    mpsc::UnboundedSender<ResponseUpdate>,
    ChatUpdates,
) {
    let (update_sender, updates) = mpsc::unbounded_channel();
    let messages = vec![Message {
        role: Role::System,
        content: "you are helpful assistent".to_owned(),
    }];
    (messages, update_sender, updates)
}

pub(crate) async fn wait_for_update(updates: &mut ChatUpdates) -> io::Result<ResponseUpdate> {
    updates
        .recv()
        .await
        .ok_or_else(|| io::Error::other("model update channel closed"))
}

pub(crate) fn send_message(application: &mut Application, content: String) {
    application.messages.push(Message {
        role: Role::User,
        content,
    });
    let messages = application.messages.clone();
    application.messages.push(Message {
        role: Role::Model,
        content: String::new(),
    });
    application.response_started = Some(Instant::now());
    application.error = None;
    let address = application.server_address.clone();
    let client = application.client.clone();
    let updates = application.update_sender.clone();
    tokio::spawn(async move {
        if let Err(error) = stream_response(&client, &address, &messages, &updates).await {
            let _ = updates.send(ResponseUpdate::Failed(error));
        }
    });
}

pub(crate) fn apply_update(application: &mut Application, update: ResponseUpdate) {
    match update {
        ResponseUpdate::Content(content) => {
            let message = application
                .messages
                .last_mut()
                .expect("an active response has a model message");
            assert!(
                matches!(message.role, Role::Model),
                "an active response belongs to the model"
            );
            message.content.push_str(&content);
        }
        ResponseUpdate::Finished { model, usage } => {
            application.statistics.model = model;
            application.statistics.prompt_tokens += usage.prompt_tokens;
            application.statistics.cached_tokens += usage.prompt_tokens_details.cached_tokens;
            application.statistics.completion_tokens += usage.completion_tokens;
            application.statistics.response_time = application
                .response_started
                .take()
                .map(|started| started.elapsed());
        }
        ResponseUpdate::Failed(error) => {
            application.error = Some(error);
            application.response_started = None;
        }
    }
}
