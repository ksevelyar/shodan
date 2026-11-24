use serde::Deserialize;

#[derive(Clone, Copy)]
pub enum ThinkingLevel {
    Low,
}

impl ThinkingLevel {
    pub fn as_provider_value(self) -> &'static str {
        match self {
            Self::Low => "low",
        }
    }
}

pub struct SelectedModel {
    pub name: String,
    pub thinking_level: Option<ThinkingLevel>,
}

#[derive(Deserialize)]
pub struct AvailableModel {
    pub id: String,
    pub status: ModelStatus,
}

#[derive(Deserialize)]
pub struct ModelStatus {
    pub value: String,
}

pub fn select_model(models: &[AvailableModel]) -> Option<(&AvailableModel, Option<ThinkingLevel>)> {
    let definitions = [("Qwen3.8-27B-UD-IQ4_XS", ThinkingLevel::Low)];
    for (name, thinking_level) in definitions {
        if let Some(model) = models.iter().find(|model| {
            model.id == name && matches!(model.status.value.as_str(), "loaded" | "unloaded")
        }) {
            return Some((model, Some(thinking_level)));
        }
    }
    models
        .iter()
        .find(|model| model.status.value == "loaded")
        .or_else(|| models.iter().find(|model| model.status.value == "unloaded"))
        .map(|model| (model, None))
}
