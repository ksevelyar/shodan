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

pub(crate) const PREFERRED_MODELS: [(&str, ThinkingLevel); 2] = [
    ("Inkling-2-Mini-UD-IQ3_XXS", ThinkingLevel::Low),
    ("Qwen3.8-27B-UD-IQ4_XS", ThinkingLevel::Low),
];

pub fn select_model(models: &[AvailableModel]) -> Option<(&AvailableModel, Option<ThinkingLevel>)> {
    for (name, thinking_level) in PREFERRED_MODELS {
        if let Some(model) = find_selectable_model(models, name) {
            return Some((model, Some(thinking_level)));
        }
    }
    models
        .iter()
        .find(|model| model.status.value == "loaded")
        .or_else(|| models.iter().find(|model| model.status.value == "unloaded"))
        .map(|model| (model, None))
}

pub(crate) fn cycle_model<'a>(
    current_model: &str,
    models: &'a [AvailableModel],
) -> Option<(&'a AvailableModel, Option<ThinkingLevel>)> {
    let current_position = PREFERRED_MODELS
        .iter()
        .position(|(name, _)| *name == current_model)
        .unwrap_or(PREFERRED_MODELS.len() - 1);
    let (name, thinking_level) = PREFERRED_MODELS[(current_position + 1) % PREFERRED_MODELS.len()];
    let model = find_selectable_model(models, name)?;
    Some((model, Some(thinking_level)))
}

fn find_selectable_model<'a>(
    models: &'a [AvailableModel],
    name: &'a str,
) -> Option<&'a AvailableModel> {
    models.iter().find(|model| {
        model.id == name && matches!(model.status.value.as_str(), "loaded" | "unloaded")
    })
}
