use shodan::settings::build_settings;

use super::support::{
    LOGIN_PORT, RecordedLlama, TestHome, create_home, read_bottom_line, read_history,
    receive_request, run_test_application, run_test_application_with_configured_clipboard,
    send_response_bytes,
};

#[tokio::test]
async fn user_configures_clipboard_command() {
    let _login_port = LOGIN_PORT.lock().await;
    let home = create_home(serde_json::json!({
        "default_provider": "llama",
        "providers": {"llama": {"address": "http://127.0.0.1:8080", "default_model": "model", "models": [{"name": "model"}]}},
        "clipboard_command": ["sh", "-c", "cat > \"$1\"", "clipboard", "temporary"]
    }));
    let clipboard_path = home.path().join("clipboard");
    let settings_path = home.path().join(".shodan/settings.json");
    let mut configuration: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_path).expect("the settings can be read"))
            .expect("the settings are valid");
    configuration["clipboard_command"][4] = serde_json::json!(clipboard_path);
    std::fs::write(settings_path, configuration.to_string()).expect("the settings can be written");
    let settings = build_settings(
        home.path().to_owned(),
        ["login", "openai"].map(str::to_owned),
    )
    .expect("the settings can be built");
    let mut application = run_test_application_with_configured_clipboard(settings);
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("copied to your clipboard"))
        .await;
    let authorization_url = std::fs::read_to_string(clipboard_path)
        .expect("the command received the authorization URL");
    let url = reqwest::Url::parse(&authorization_url).expect("the copied URL is valid");
    assert_eq!(url.host_str(), Some("auth.openai.com"));
    assert_eq!(
        url.query_pairs()
            .find(|(name, _)| name == "state")
            .expect("the copied URL contains state")
            .1,
        get_login_state(&screen)
    );
}

fn get_login_state(screen: &ratatui::buffer::Buffer) -> String {
    let history: String = read_history(screen)
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let authorization_url = history
        .split('│')
        .find(|line| line.starts_with("https://auth.openai.com/"))
        .expect("the authorization URL is displayed");
    let authorization_url =
        reqwest::Url::parse(authorization_url).expect("the authorization URL is valid");
    authorization_url
        .query_pairs()
        .find(|(name, _)| name == "state")
        .expect("the authorization URL contains state")
        .1
        .into_owned()
}

fn create_openai_home(address: &str) -> TestHome {
    let home = create_home(serde_json::json!({
        "default_provider": "openai",
        "providers": {"openai": {
            "address": address,
            "authentication_address": address,
            "default_model": "gpt-6.1-terra",
            "models": [{"name": "gpt-6.1-terra", "thinking_level": "high"}, {"name": "gpt-6-luna", "thinking_level": "max"}]
        }}
    }));
    std::fs::write(
        home.path().join(".shodan/openai.json"),
        serde_json::json!({
            "type": "oauth", "access": "recorded-access", "refresh": "recorded-refresh",
            "expires": 4_102_444_800_000_u64, "clientId": "recorded-client",
            "scopes": ["chatgpt.tokens.use.direct"]
        })
        .to_string(),
    )
    .expect("the credentials can be written");
    home
}

#[tokio::test]
async fn user_chats_with_default_openai_model() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Greet me");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["model"], "gpt-6.1-terra");
    assert_eq!(request.body["store"], false);
    assert_eq!(request.body["stream"], true);
    assert_eq!(request.body["reasoning"]["effort"], "high");
    send_response_bytes(&request, include_str!("fixtures/openai/chat.txt")).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello, world!"))
        .await;
    application.send_message("What did you just say?");
    let request = receive_request(&mut requests).await;
    let input = request.body["input"]
        .as_array()
        .expect("conversation input is an array");
    assert!(input.iter().any(|message| message["role"] == "assistant"));
    send_response_bytes(&request, include_str!("fixtures/openai/continuation.txt")).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("↑46 ↓16 CH0%"))
        .await;
}

#[tokio::test]
async fn user_cycles_to_luna() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.press_control_key(' ');
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("openai gpt-6-luna max"))
        .await;
    application.send_message("Greet me");
    let request = receive_request(&mut requests).await;
    assert_eq!(request.body["model"], "gpt-6-luna");
    assert_eq!(request.body["reasoning"]["effort"], "max");
    send_response_bytes(&request, include_str!("fixtures/openai/luna.txt")).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello, world!"))
        .await;
}

#[tokio::test]
async fn user_watches_openai_thinking() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Solve the puzzle");
    let request = receive_request(&mut requests).await;
    let response = include_str!("fixtures/openai/thinking.txt");
    let end = response
        .find("event: response.output_text.delta")
        .expect("the response has streamed text");
    send_response_bytes(&request, &response[..end]).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Calculating lamb total"))
        .await;
    send_response_bytes(&request, &response[end..]).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("↑46 ↓96 CH0%"))
        .await;
}

#[tokio::test]
async fn user_continues_after_openai_thinking() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Solve the puzzle");
    let request = receive_request(&mut requests).await;
    send_response_bytes(&request, include_str!("fixtures/openai/thinking.txt")).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("↑46 ↓96 CH0%"))
        .await;
    application.send_message("Continue");
    let request = receive_request(&mut requests).await;
    assert!(
        request.body["input"]
            .as_array()
            .expect("the input is an array")
            .iter()
            .any(|item| item["type"] == "reasoning"
                && item["encrypted_content"]
                    .as_str()
                    .is_some_and(|content| !content.is_empty()))
    );
    send_response_bytes(&request, include_str!("fixtures/openai/continuation.txt")).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("↑79 ↓104 CH0%"))
        .await;
}

#[tokio::test]
async fn user_uses_tool_with_openai() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Read the greeting file");
    let request = receive_request(&mut requests).await;
    send_response_bytes(&request, include_str!("fixtures/openai/tool-call.txt")).await;
    let request = receive_request(&mut requests).await;
    let input = request.body["input"]
        .as_array()
        .expect("conversation input is an array");
    assert!(
        input
            .iter()
            .any(|item| item["type"] == "function_call" && item["name"] == "read")
    );
    assert!(input.iter().any(|item| {
        item["type"] == "function_call_output"
            && item["output"]
                .as_str()
                .is_some_and(|output| output.contains("Hello, Shodan!"))
    }));
    send_response_bytes(&request, include_str!("fixtures/openai/tool-result.txt")).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello, Shodan!"))
        .await;
}

#[tokio::test]
async fn user_views_openai_usage() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Greet me");
    let request = receive_request(&mut requests).await;
    send_response_bytes(&request, include_str!("fixtures/openai/chat.txt")).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("↑13 ↓8 CH0%"))
        .await;
}

#[tokio::test]
async fn user_continues_with_refreshed_credentials() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let path = home.path().join(".shodan/openai.json");
    let mut credential: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("the credentials can be read"))
            .expect("the credentials are valid");
    credential["expires"] = serde_json::json!(0);
    std::fs::write(&path, credential.to_string()).expect("the expired credentials can be written");
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Greet me");
    let request = receive_request(&mut requests).await;
    send_response_bytes(&request, include_str!("fixtures/openai/chat.txt")).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello, world!"))
        .await;
    let credential: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("the credentials can be read"))
            .expect("the credentials are valid");
    let recorded: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/openai/refresh.json"))
            .expect("the recorded response is valid");
    assert_eq!(credential["access"], recorded["access_token"]);
    assert_eq!(credential["refresh"], recorded["refresh_token"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path)
                .expect("the credentials exist")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[tokio::test]
async fn user_aborts_openai_response() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Greet me");
    let request = receive_request(&mut requests).await;
    let response = include_str!("fixtures/openai/chat.txt");
    let end = response
        .find("event: response.output_text.done")
        .expect("the response has streamed text");
    send_response_bytes(&request, &response[..end]).await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello, world!"))
        .await;
    application.press_key(crossterm::event::KeyCode::Esc);
    application
        .wait_for_screen(|screen| read_history(screen).contains("Request was aborted"))
        .await;
    application.send_message("Greet me again");
    let request = receive_request(&mut requests).await;
    send_response_bytes(&request, response).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("↑13 ↓8 CH0%"))
        .await;
}

#[tokio::test]
async fn user_sees_openai_request_error() {
    let (address, mut requests) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Greet me");
    let request = receive_request(&mut requests).await;
    let response = include_str!("fixtures/openai/chat.txt");
    let end = response
        .find("event: response.output_text.done")
        .expect("the response has streamed text");
    send_response_bytes(&request, &response[..end]).await;
    drop(request);
    application
        .wait_for_screen(|screen| super::support::read_screen(screen).contains("Request failed"))
        .await;
    application.send_message("Greet me again");
    let request = receive_request(&mut requests).await;
    send_response_bytes(&request, response).await;
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("↑13 ↓8 CH0%"))
        .await;
}

#[tokio::test]
async fn user_sees_authentication_error() {
    let (address, _) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    let path = home.path().join(".shodan/openai.json");
    let mut credential: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("the credentials can be read"))
            .expect("the credentials are valid");
    credential["expires"] = serde_json::json!(0);
    credential["clientId"] = serde_json::json!("invalid-client");
    std::fs::write(path, credential.to_string()).expect("the expired credentials can be written");
    let settings =
        build_settings(home.path().to_owned(), Vec::new()).expect("the settings can be built");
    let mut application = run_test_application(settings);
    application.send_message("Hello");
    application
        .wait_for_screen(|screen| {
            super::support::read_screen(screen).contains("shodan login openai")
        })
        .await;
}

#[tokio::test]
async fn user_sees_invalid_openai_identity() {
    let _login_port = LOGIN_PORT.lock().await;
    let (address, _) = RecordedLlama::new().start_openai().await;
    let home = create_openai_home(&address);
    std::fs::remove_file(home.path().join(".shodan/openai.json"))
        .expect("the credentials can be removed");
    let settings = build_settings(
        home.path().to_owned(),
        ["login", "openai"].map(str::to_owned),
    )
    .expect("the settings can be built");
    let mut application = run_test_application(settings);
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("auth.openai.com"))
        .await;
    let state = get_login_state(&screen);
    application.send_message(&format!("http://127.0.0.1:1455/auth/callback?code=recorded-code&client_id=recorded-client&state={state}"));
    application
        .wait_for_screen(|screen| read_history(screen).contains("Invalid OpenAI identity"))
        .await;
}

#[tokio::test]
async fn user_starts_openai_login_again() {
    let _login_port = LOGIN_PORT.lock().await;
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home_directory).expect("the home directory can be created");
    let settings = build_settings(
        home_directory.clone(),
        ["login", "openai"].map(str::to_owned),
    )
    .expect("the settings can be built");
    let path = home_directory.join(".shodan/settings.json");
    let original_settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("the settings can be read"))
            .expect("the settings are valid JSON");
    let mut identifiers = Vec::new();
    for settings in [
        settings,
        build_settings(
            home_directory.clone(),
            ["login", "openai"].map(str::to_owned),
        )
        .expect("the settings can be built"),
    ] {
        let mut application = run_test_application(settings);
        let screen = application
            .wait_for_screen(|screen| read_history(screen).contains("auth.openai.com"))
            .await;
        let history: String = read_history(&screen)
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let authorization_url = history
            .split('│')
            .find(|line| line.starts_with("https://auth.openai.com/"))
            .expect("the authorization URL is displayed");
        let authorization_url =
            reqwest::Url::parse(authorization_url).expect("the authorization URL is valid");
        let identifier = authorization_url
            .query_pairs()
            .find(|(name, _)| name == "ext_agent_host_id")
            .expect("the authorization URL identifies the installation")
            .1
            .into_owned();
        identifiers.push(identifier);
        let state = get_login_state(&screen);
        application.send_message(&format!(
            "http://127.0.0.1:1455/auth/callback?error=access_denied&state={state}"
        ));
        application
            .wait_for_screen(|screen| read_history(screen).contains("access_denied"))
            .await;
        drop(application);
    }
    let mut saved_settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("the settings can be read"))
            .expect("the settings are valid JSON");
    let identifier = saved_settings
        .as_object_mut()
        .expect("the settings are an object")
        .remove("device_id")
        .expect("the device identifier is saved");
    let identifier = identifier
        .as_str()
        .expect("the device identifier is a string");
    uuid::Uuid::parse_str(identifier).expect("the device identifier is a UUID");
    assert_eq!(identifiers[0], format!("urn:uuid:{identifier}"));
    assert_eq!(identifiers[1], identifiers[0]);
    assert_eq!(saved_settings, original_settings);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_sees_login_error() {
    let _login_port = LOGIN_PORT.lock().await;
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home_directory).expect("the home directory can be created");
    let mut application = run_test_application(
        build_settings(
            home_directory.clone(),
            ["login".to_owned(), "openai".to_owned()],
        )
        .expect("the settings can be built"),
    );
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("auth.openai.com"))
        .await;
    let state = get_login_state(&screen);
    application.send_message(&format!(
        "http://127.0.0.1:1455/auth/callback?error=access_denied&state={state}"
    ));
    application
        .wait_for_screen(|screen| {
            let history = read_history(screen);
            history.contains("ChatGPT authorization failed: access_denied")
        })
        .await;
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_sees_unavailable_callback_error() {
    let _login_port = LOGIN_PORT.lock().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:1455")
        .await
        .expect("the callback port can be occupied");
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home_directory).expect("the home directory can be created");
    let mut application = run_test_application(
        build_settings(
            home_directory.clone(),
            ["login", "openai"].map(str::to_owned),
        )
        .expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_history(screen).contains("Cannot start login callback"))
        .await;
    drop(application);
    drop(listener);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_continues_login_after_invalid_redirect_url() {
    let _login_port = LOGIN_PORT.lock().await;
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home_directory).expect("the home directory can be created");
    let mut application = run_test_application(
        build_settings(
            home_directory.clone(),
            ["login", "openai"].map(str::to_owned),
        )
        .expect("the settings can be built"),
    );
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("auth.openai.com"))
        .await;
    let state = get_login_state(&screen);
    application.send_message("invalid");
    application
        .wait_for_screen(|screen| read_history(screen).contains("Paste the full callback URL"))
        .await;
    application.send_message(&format!(
        "http://127.0.0.1:1455/auth/callback?error=access_denied&state={state}"
    ));
    application
        .wait_for_screen(|screen| {
            read_history(screen).contains("authorization failed: access_denied")
        })
        .await;
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_receives_callback_after_invalid_redirect_url() {
    let _login_port = LOGIN_PORT.lock().await;
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home_directory).expect("the home directory can be created");
    let mut application = run_test_application(
        build_settings(
            home_directory.clone(),
            ["login", "openai"].map(str::to_owned),
        )
        .expect("the settings can be built"),
    );
    let screen = application
        .wait_for_screen(|screen| read_history(screen).contains("auth.openai.com"))
        .await;
    let state = get_login_state(&screen);
    application.send_message("invalid");
    application
        .wait_for_screen(|screen| read_history(screen).contains("Paste the full callback URL"))
        .await;
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("the callback client can be built")
        .get(format!(
            "http://127.0.0.1:1455/auth/callback?error=access_denied&state={state}"
        ))
        .send()
        .await
        .expect("the browser reaches the callback");
    application
        .wait_for_screen(|screen| {
            read_history(screen).contains("authorization failed: access_denied")
        })
        .await;
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_starts_openai_login_with_initial_model() {
    let _login_port = LOGIN_PORT.lock().await;
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home_directory).expect("the home directory can be created");
    let mut application = run_test_application(
        build_settings(
            home_directory.clone(),
            ["login".to_owned(), "openai".to_owned()],
        )
        .expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| {
            read_history(screen).contains("auth.openai.com")
                && read_bottom_line(screen).contains("openai gpt-6.1-terra high")
        })
        .await;
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_starts_openai_login_with_configured_model() {
    let _login_port = LOGIN_PORT.lock().await;
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let configuration_directory = home_directory.join(".shodan");
    std::fs::create_dir_all(&configuration_directory)
        .expect("the configuration directory can be created");
    let configuration = serde_json::json!({
        "default_provider": "llama",
        "providers": {
            "llama": {
                "address": "http://127.0.0.1:8080",
                "default_model": "Inkling-2-Mini-UD-IQ3_XXS",
                "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}]
            },
            "openai": {
                "default_model": "gpt-6-luna",
                "models": [{"name": "gpt-6-luna", "thinking_level": "max"}]
            }
        }
    });
    std::fs::write(
        configuration_directory.join("settings.json"),
        configuration.to_string(),
    )
    .expect("the provider configuration can be written");
    let mut application = run_test_application(
        build_settings(
            home_directory.clone(),
            ["login".to_owned(), "openai".to_owned()],
        )
        .expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| {
            read_history(screen).contains("auth.openai.com")
                && read_bottom_line(screen).contains("openai gpt-6-luna max")
        })
        .await;
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}
