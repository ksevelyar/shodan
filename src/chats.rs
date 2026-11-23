use std::io;

use tokio::{sync::mpsc, time::Instant};

use crate::{
    application::{Application, start_response},
    provider::ProviderEvent,
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
    pub(crate) aborted: bool,
}

pub type ChatUpdates = mpsc::UnboundedReceiver<ProviderEvent>;

pub(crate) fn build_chat() -> (
    Vec<Message>,
    mpsc::UnboundedSender<ProviderEvent>,
    ChatUpdates,
) {
    let (update_sender, updates) = mpsc::unbounded_channel();
    let messages = vec![Message {
        role: Role::System,
        content: "you are helpful assistent".to_owned(),
        aborted: false,
    }];
    (messages, update_sender, updates)
}

pub(crate) async fn wait_for_update(updates: &mut ChatUpdates) -> io::Result<ProviderEvent> {
    updates
        .recv()
        .await
        .ok_or_else(|| io::Error::other("model update channel closed"))
}

pub(crate) fn send_message(application: &mut Application, content: String) {
    application.messages.push(Message {
        role: Role::User,
        content,
        aborted: false,
    });
    let messages = application.messages.clone();
    application.messages.push(Message {
        role: Role::Model,
        content: String::new(),
        aborted: false,
    });
    application.statistics.response_started = Some(Instant::now());
    application.error = None;
    start_response(application, messages);
}

pub(crate) fn mark_response_aborted(application: &mut Application) {
    let message = application
        .messages
        .last_mut()
        .expect("an active response has a model message");
    message.aborted = true;
}

pub(crate) fn apply_update(application: &mut Application, update: ProviderEvent) {
    match update {
        ProviderEvent::ModelSelected(model) => application.statistics.model = model,
        ProviderEvent::MessageDelta(content) => {
            let message = application
                .messages
                .last_mut()
                .expect("an active response has a model message");
            message.content.push_str(&content);
        }
        ProviderEvent::ResponseEnded {
            prompt_tokens,
            completion_tokens,
            cached_tokens,
        } => {
            application.active_request = None;
            application.statistics.prompt_tokens += prompt_tokens;
            application.statistics.cached_tokens += cached_tokens;
            application.statistics.completion_tokens += completion_tokens;
            application.statistics.response_time = application
                .statistics
                .response_started
                .take()
                .map(|started| started.elapsed());
        }
        ProviderEvent::Error(error) => {
            application.active_request = None;
            application.error = Some(error);
            application.statistics.response_started = None;
        }
    }
}
