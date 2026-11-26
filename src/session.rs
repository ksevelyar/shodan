use crate::{
    application::ApplicationError,
    chats::{Message, build_chat},
    models::Model,
    providers::{self, Provider, ProviderSettings},
};

pub(crate) struct Session {
    pub provider: Provider,
    pub messages: Vec<Message>,
    pub turn: u64,
    pub turn_start: usize,
    selected_model_position: usize,
    context_position: usize,
    home_directory: std::path::PathBuf,
}

impl Session {
    pub(crate) fn build(settings: ProviderSettings) -> Result<Self, ApplicationError> {
        let selected_model_position = Self::select_model_position(&settings)?;
        let home_directory = settings.home_directory.clone();
        let provider = providers::build(settings)?;
        let mut session = Self {
            provider,
            messages: Vec::new(),
            turn: 0,
            turn_start: 0,
            selected_model_position,
            context_position: 0,
            home_directory,
        };
        session.clear_messages();
        Ok(session)
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
        ["read", "edit", "review", "blank"][self.context_position]
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

    pub(crate) fn cycle_context(&mut self) {
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
        self.context_position = (self.context_position + 1) % 4;
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
