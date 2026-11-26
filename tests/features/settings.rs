use shodan::settings::build_settings;

use super::support::{read_bottom_line, run_test_application};

#[test]
fn user_starts_with_invalid_configuration() {
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let directory = home_directory.join(".shodan");
    std::fs::create_dir_all(&directory).expect("the settings directory can be created");
    std::fs::write(directory.join("settings.json"), "invalid JSON")
        .expect("the settings can be written");
    let result = build_settings(home_directory.clone(), Vec::new());
    let Err(error) = result else {
        panic!("invalid settings must fail");
    };
    assert!(matches!(
        error,
        shodan::application::ApplicationError::Configuration(_)
    ));
    assert!(error.to_string().starts_with("configuration: "));
    assert!(error.to_string().contains("settings.json"));
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_starts_with_existing_settings() {
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let directory = home_directory.join(".shodan");
    std::fs::create_dir_all(&directory).expect("the settings directory can be created");
    let address = "http://127.0.0.1:8080";
    let settings = serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": address,
                "default_model": "Inkling-2-Mini-UD-IQ3_XXS",
                "models": [
                    {
                        "name": "Inkling-2-Mini-UD-IQ3_XXS",
                        "thinking_level": "low"
                    }
                ]
            }
        }
    })
    .to_string();
    let path = directory.join("settings.json");
    std::fs::write(&path, &settings).expect("the settings can be written");
    let mut application = run_test_application(
        build_settings(home_directory.clone(), Vec::new()).expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    assert_eq!(
        std::fs::read_to_string(path).expect("the settings are readable"),
        settings
    );
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_starts_with_existing_shodan_directory() {
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let configuration_directory = home_directory.join(".shodan");
    std::fs::create_dir_all(&configuration_directory)
        .expect("the configuration directory can be created");
    let mut application = run_test_application(
        build_settings(home_directory.clone(), Vec::new()).expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    let configuration: serde_json::Value = serde_json::from_slice(
        &std::fs::read(configuration_directory.join("settings.json"))
            .expect("the provider configuration exists"),
    )
    .expect("the provider configuration is JSON");
    assert_eq!(configuration["default_provider"], "llama");
    assert_eq!(
        configuration["providers"]["llama"]["default_model"],
        "Inkling-2-Mini-UD-IQ3_XXS"
    );
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_starts_shodan_for_first_time() {
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home_directory).expect("the home directory can be created");
    let mut application = run_test_application(
        build_settings(home_directory.clone(), Vec::new()).expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| {
            read_bottom_line(screen).contains("llama Inkling-2-Mini-UD-IQ3_XXS low")
        })
        .await;
    let path = home_directory.join(".shodan/settings.json");
    let configuration: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("the provider configuration exists"))
            .expect("the provider configuration is JSON");
    assert_eq!(configuration["default_provider"], "llama");
    assert_eq!(
        configuration["providers"]["llama"]["address"],
        "http://127.0.0.1:8080"
    );
    assert_eq!(
        configuration["providers"]["llama"]["default_model"],
        "Inkling-2-Mini-UD-IQ3_XXS"
    );
    assert_eq!(
        configuration["providers"]["llama"]["models"],
        serde_json::json!([{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}])
    );
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}
