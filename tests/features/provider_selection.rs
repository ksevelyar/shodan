use serde_json::{Value, json};
use shodan::settings::build_settings;

use super::support::{read_bottom_line, run_test_application};

fn create_configuration(default_provider: &str) -> (std::path::PathBuf, String) {
    let home = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let directory = home.join(".shodan");
    std::fs::create_dir_all(&directory).expect("the configuration directory can be created");
    let configuration = json!({
        "default_provider": default_provider,
        "providers": {
            "llama": {"address": "http://127.0.0.1:8080", "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}]},
            "openai": {"default_model": "gpt-6.1-terra", "models": [{"name": "gpt-6.1-terra", "thinking_level": "high"}, {"name": "gpt-6-luna", "thinking_level": "max"}]}
        }
    }).to_string();
    std::fs::write(directory.join("settings.json"), &configuration)
        .expect("the configuration can be written");
    (home, configuration)
}

fn assert_preserved(home: &std::path::Path, configuration: &str) {
    let actual = std::fs::read_to_string(home.join(".shodan/settings.json"))
        .expect("the configuration is readable");
    assert_eq!(actual, configuration);
}

#[tokio::test]
async fn user_starts_with_existing_configuration() {
    let (home, configuration) = create_configuration("openai");
    let path = home.join(".shodan/settings.json");
    let mut value: Value = serde_json::from_str(&configuration).expect("the configuration is JSON");
    value["providers"]
        .as_object_mut()
        .expect("providers is an object")
        .remove("llama");
    let configuration = value.to_string();
    std::fs::write(path, &configuration).expect("the configuration can be written");
    let mut application = run_test_application(
        build_settings(home.clone(), Vec::new()).expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("openai gpt-6.1-terra high"))
        .await;
    assert_preserved(&home, &configuration);
    drop(application);
    std::fs::remove_dir_all(home).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_selects_provider() {
    let (home, configuration) = create_configuration("llama");
    let mut application = run_test_application(
        build_settings(home.clone(), ["--provider", "openai"].map(str::to_owned))
            .expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("openai gpt-6.1-terra high"))
        .await;
    assert_preserved(&home, &configuration);
    drop(application);
    std::fs::remove_dir_all(home).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_selects_model() {
    let (home, configuration) = create_configuration("llama");
    let mut application = run_test_application(
        build_settings(
            home.clone(),
            ["--provider", "openai", "--model", "gpt-6-luna"].map(str::to_owned),
        )
        .expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("openai gpt-6-luna max"))
        .await;
    assert_preserved(&home, &configuration);
    drop(application);
    std::fs::remove_dir_all(home).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_cycles_through_configured_openai_models() {
    let (home, configuration) = create_configuration("openai");
    let mut application = run_test_application(
        build_settings(home.clone(), Vec::new()).expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("openai gpt-6.1-terra high"))
        .await;
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("openai gpt-6-luna max"))
        .await;
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("openai gpt-6.1-terra high"))
        .await;
    assert_preserved(&home, &configuration);
    drop(application);
    std::fs::remove_dir_all(home).expect("the home directory can be removed");
}

fn assert_selection_error(provider: Option<&str>, model: Option<&str>, expected: &str) {
    let (home, configuration) = create_configuration("llama");
    let arguments = provider
        .into_iter()
        .flat_map(|value| ["--provider", value])
        .chain(model.into_iter().flat_map(|value| ["--model", value]))
        .map(str::to_owned);
    let result = build_settings(home.clone(), arguments);
    let Err(error) = result else {
        panic!("the unavailable selection must fail")
    };
    assert!(matches!(
        error,
        shodan::application::ApplicationError::Configuration(_)
    ));
    assert!(error.to_string().contains(expected), "{error}");
    assert_preserved(&home, &configuration);
    std::fs::remove_dir_all(home).expect("the home directory can be removed");
}

#[test]
fn user_selects_unavailable_provider() {
    assert_selection_error(Some("unavailable"), None, "unavailable");
}

#[test]
fn user_selects_unavailable_model() {
    assert_selection_error(Some("openai"), Some("missing-model"), "missing-model");
}
