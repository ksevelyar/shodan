mod storage;

use crate::{
    application::ApplicationError,
    chats::{Message, build_chat},
    models::Model,
    providers::{self, Provider, ProviderSettings},
};

const CONTEXT_NAMES: [&str; 4] = ["read", "edit", "review", "blank"];

pub(crate) struct Session {
    pub provider: Provider,
    pub messages: Vec<Message>,
    pub turn: u64,
    pub turn_start: usize,
    selected_model_position: usize,
    context_position: usize,
    home_directory: std::path::PathBuf,
    storage_path: std::path::PathBuf,
    saved: bool,
}

impl Session {
    pub(crate) fn build(
        settings: ProviderSettings,
        session_number: Option<usize>,
    ) -> Result<Self, ApplicationError> {
        let selected_model_position = Self::select_model_position(&settings)?;
        let home_directory = settings.home_directory.clone();
        let provider = providers::build(settings)?;
        let directory = storage::get_directory(&home_directory).map_err(|error| {
            ApplicationError::Configuration(format!("Failed to locate sessions: {error}"))
        })?;
        let session_path = if let Some(number) = session_number {
            storage::select_session(&directory, number).map_err(|error| {
                ApplicationError::Configuration(format!("Failed to select session: {error}"))
            })?
        } else {
            directory.join(format!("{}.jsonl", uuid::Uuid::new_v4()))
        };
        for context in CONTEXT_NAMES.iter().filter(|context| **context != "blank") {
            let path = home_directory.join(".shodan").join(format!("{context}.md"));
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path);
        }
        let mut session = Self {
            provider,
            messages: Vec::new(),
            turn: 0,
            turn_start: 0,
            selected_model_position,
            context_position: 0,
            home_directory,
            storage_path: session_path,
            saved: session_number.is_some(),
        };
        if session_number.is_some() {
            let snapshot = storage::read_session(&session.storage_path).map_err(|error| {
                ApplicationError::Configuration(format!(
                    "Failed to read session {}: {error}",
                    session.storage_path.display()
                ))
            })?;
            session.context_position = CONTEXT_NAMES
                .iter()
                .position(|context| *context == snapshot.context)
                .ok_or_else(|| {
                    ApplicationError::Configuration(format!(
                        "Unknown session context: {}",
                        snapshot.context
                    ))
                })?;
            if session.provider.get_name() != snapshot.provider {
                return Err(ApplicationError::Configuration(format!(
                    "Session requires provider {}",
                    snapshot.provider
                )));
            }
            session.selected_model_position = session
                .provider
                .models
                .iter()
                .position(|model| model.name == snapshot.model)
                .ok_or_else(|| {
                    ApplicationError::Configuration(format!(
                        "Session model is not configured: {}",
                        snapshot.model
                    ))
                })?;
            session.messages = snapshot.messages;
        } else {
            session.clear_messages();
        }
        Ok(session)
    }

    pub(crate) fn save(&mut self, create: bool) -> Result<(), ApplicationError> {
        if !self.saved && !create {
            return Ok(());
        }
        let snapshot = storage::SessionSnapshot {
            context: self.get_context_name().to_owned(),
            model: self.get_selected_model().name.clone(),
            provider: self.provider.get_name().to_owned(),
            messages: self.messages.clone(),
        };
        storage::write_session(&self.storage_path, &snapshot).map_err(|error| {
            ApplicationError::Output(format!(
                "Failed to save session {}: {error}",
                self.storage_path.display()
            ))
        })?;
        self.saved = true;
        Ok(())
    }

    pub(crate) fn get_saved_selection(
        home_directory: &std::path::Path,
        number: usize,
    ) -> Result<(String, String), ApplicationError> {
        let directory = storage::get_directory(home_directory).map_err(|error| {
            ApplicationError::Configuration(format!("Failed to locate sessions: {error}"))
        })?;
        let path = storage::select_session(&directory, number).map_err(|error| {
            ApplicationError::Configuration(format!("Failed to select session: {error}"))
        })?;
        let snapshot = storage::read_session(&path).map_err(|error| {
            ApplicationError::Configuration(format!(
                "Failed to read session {}: {error}",
                path.display()
            ))
        })?;
        Ok((snapshot.provider, snapshot.model))
    }

    pub(crate) fn select_model_position(
        settings: &ProviderSettings,
    ) -> Result<usize, ApplicationError> {
        let model = settings
            .model_override
            .as_deref()
            .unwrap_or(&settings.default_model);
        settings
            .models
            .iter()
            .position(|configured_model| configured_model.name == model)
            .ok_or_else(|| {
                let provider = settings.selection.get_name();
                ApplicationError::Configuration(format!(
                    "Invalid provider or model selection: model not configured for {provider}: {model}"
                ))
            })
    }

    pub(crate) fn clear_messages(&mut self) {
        self.messages = build_chat(self.get_prompt());
        self.turn_start = 0;
    }

    pub(crate) fn get_context_name(&self) -> &'static str {
        CONTEXT_NAMES[self.context_position]
    }

    fn get_prompt(&self) -> String {
        let context = self.get_context_name();
        if context == "blank" {
            return String::new();
        }
        let context_path = self
            .home_directory
            .join(".shodan")
            .join(format!("{context}.md"));
        let context_prompt = std::fs::read_to_string(context_path).unwrap_or_default();
        let project_prompt = std::fs::read_to_string("AGENTS.md").unwrap_or_default();
        [context_prompt, project_prompt]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn cycle_context(&mut self, new_session: bool) {
        self.context_position = (self.context_position + 1) % CONTEXT_NAMES.len();
        if !new_session {
            let prompt = self.get_prompt();
            if let Some(message) = self.messages.first_mut() {
                *message = Message::System(prompt);
            }
            return;
        }
        let last_response = self
            .messages
            .iter()
            .rev()
            .find_map(|message| match message {
                Message::Model {
                    content,
                    aborted: false,
                    tool_calls,
                    ..
                } if tool_calls.is_empty() && !content.is_empty() => Some(content.clone()),
                _ => None,
            });
        self.storage_path = self
            .storage_path
            .with_file_name(format!("{}.jsonl", uuid::Uuid::new_v4()));
        self.saved = false;
        self.clear_messages();
        if let Some(content) = last_response {
            self.messages.push(Message::Model {
                content,
                thinking: String::new(),
                tool_calls: Vec::new(),
                provider_items: Vec::new(),
                aborted: false,
            });
        }
    }

    pub(crate) fn get_selected_model(&self) -> &Model {
        &self.provider.models[self.selected_model_position]
    }

    pub(crate) fn cycle_model(&mut self) {
        self.selected_model_position =
            (self.selected_model_position + 1) % self.provider.models.len();
    }
}
