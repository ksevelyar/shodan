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
}

impl Session {
    pub(crate) fn build(settings: ProviderSettings) -> Result<Self, ApplicationError> {
        let selected_model_position = Self::select_model_position(&settings)?;
        let provider = providers::build(settings)?;
        Ok(Self {
            provider,
            messages: build_chat(),
            turn: 0,
            turn_start: 0,
            selected_model_position,
        })
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
        self.messages = build_chat();
        self.turn_start = 0;
    }

    pub(crate) fn get_selected_model(&self) -> &Model {
        &self.provider.models[self.selected_model_position]
    }

    pub(crate) fn cycle_model(&mut self) {
        self.selected_model_position =
            (self.selected_model_position + 1) % self.provider.models.len();
    }
}
