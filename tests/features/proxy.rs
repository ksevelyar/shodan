use std::time::Duration;

use serde_json::json;
use shodan::settings::build_settings;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::support::{
    LOGIN_PORT, RecordedLlama, read_bottom_line, read_history, receive_request,
    run_test_application, send_response_bytes,
};

#[tokio::test]
async fn user_starts_with_default_proxy() {
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home_directory).expect("the home directory can be created");
    let mut application = run_test_application(
        build_settings(home_directory.clone(), Vec::new()).expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    let configuration: serde_json::Value = serde_json::from_slice(
        &std::fs::read(home_directory.join(".shodan/settings.json"))
            .expect("the provider configuration exists"),
    )
    .expect("the provider configuration is JSON");
    assert_eq!(configuration["proxy"]["url"], "socks5h://127.0.0.1:2081");
    assert_eq!(configuration["proxy"]["providers"], json!(["openai"]));
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_connects_to_local_server_while_openai_uses_proxy() {
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let configuration_directory = home_directory.join(".shodan");
    std::fs::create_dir_all(&configuration_directory)
        .expect("the configuration directory can be created");
    let (address, mut requests) = RecordedLlama::new().start().await;
    let configuration = json!({"proxy": {"url": "socks5h://127.0.0.1:1", "providers": ["openai"]}, "default_provider": "llama", "providers": {"llama": {"address": address, "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}]}}});
    std::fs::write(
        configuration_directory.join("settings.json"),
        configuration.to_string(),
    )
    .expect("the configuration can be written");
    let mut application = run_test_application(
        build_settings(home_directory.clone(), Vec::new()).expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    send_response_bytes(
        &request,
        include_str!("fixtures/llama/chat/inkling-hello-low.txt"),
    )
    .await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello! How can I help you today?"))
        .await;
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_connects_to_llama_through_proxy() {
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let configuration_directory = home_directory.join(".shodan");
    std::fs::create_dir_all(&configuration_directory)
        .expect("the configuration directory can be created");
    let (address, mut requests) = RecordedLlama::new().start().await;
    let server_address: std::net::SocketAddr = address
        .trim_start_matches("http://")
        .parse()
        .expect("the llama server address is valid");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the proxy binds to a local port");
    let proxy_address = listener.local_addr().expect("the proxy has an address");
    let (connections, mut received_connections) = tokio::sync::mpsc::unbounded_channel();
    let proxy = tokio::spawn(async move {
        while let Ok((mut connection, _)) = listener.accept().await {
            let connections = connections.clone();
            tokio::spawn(async move {
                let mut greeting = [0; 2];
                connection
                    .read_exact(&mut greeting)
                    .await
                    .expect("the proxy receives a greeting");
                let mut methods = vec![0; usize::from(greeting[1])];
                connection
                    .read_exact(&mut methods)
                    .await
                    .expect("the proxy receives authentication methods");
                connection
                    .write_all(&[5, 0])
                    .await
                    .expect("the proxy accepts unauthenticated connections");
                let mut request = [0; 10];
                connection
                    .read_exact(&mut request)
                    .await
                    .expect("the proxy receives the destination");
                assert_eq!(&request[..4], &[5, 1, 0, 1]);
                assert_eq!(&request[4..8], &[127, 0, 0, 1]);
                assert_eq!(
                    u16::from_be_bytes([request[8], request[9]]),
                    server_address.port()
                );
                let mut server = tokio::net::TcpStream::connect(server_address)
                    .await
                    .expect("the proxy connects to llama");
                connection
                    .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
                    .await
                    .expect("the proxy accepts the request");
                connections
                    .send(())
                    .expect("the test receives the proxy connection");
                let _ = tokio::io::copy_bidirectional(&mut connection, &mut server).await;
            });
        }
    });
    let configuration = json!({"proxy": {"url": format!("socks5h://{proxy_address}"), "providers": ["llama"]}, "default_provider": "llama", "providers": {"llama": {"address": address, "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}]}}});
    std::fs::write(
        configuration_directory.join("settings.json"),
        configuration.to_string(),
    )
    .expect("the configuration can be written");
    let mut application = run_test_application(
        build_settings(home_directory.clone(), Vec::new()).expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    send_response_bytes(
        &request,
        include_str!("fixtures/llama/chat/inkling-hello-low.txt"),
    )
    .await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello! How can I help you today?"))
        .await;
    received_connections
        .recv()
        .await
        .expect("llama was contacted through the proxy");
    drop(application);
    proxy.abort();
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

fn create_openai_proxy_configuration(address: std::net::SocketAddr) -> serde_json::Value {
    json!({"proxy": {"url": format!("socks5h://{address}"), "providers": ["openai"]}, "default_provider": "llama", "providers": {"llama": {"address": "http://127.0.0.1:8080", "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}]}}})
}

#[tokio::test]
async fn user_sees_proxy_connection_error() {
    let _login_port = LOGIN_PORT.lock().await;
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let configuration_directory = home_directory.join(".shodan");
    std::fs::create_dir_all(&configuration_directory)
        .expect("the configuration directory can be created");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the proxy binds to a local port");
    let address = listener.local_addr().expect("the proxy has an address");
    std::fs::write(
        configuration_directory.join("settings.json"),
        create_openai_proxy_configuration(address).to_string(),
    )
    .expect("the configuration can be written");
    let proxy = tokio::spawn(async move {
        let (mut connection, _) = listener
            .accept()
            .await
            .expect("Shodan connects to the proxy");
        let mut greeting = [0; 2];
        connection
            .read_exact(&mut greeting)
            .await
            .expect("the proxy receives a greeting");
        assert_eq!(greeting[0], 5);
        let mut methods = vec![0; usize::from(greeting[1])];
        connection
            .read_exact(&mut methods)
            .await
            .expect("the proxy receives authentication methods");
        connection
            .write_all(&[5, 0])
            .await
            .expect("the proxy accepts unauthenticated connections");
        let mut request = [0; 5];
        connection
            .read_exact(&mut request)
            .await
            .expect("the proxy receives the destination");
        assert_eq!(&request[..4], &[5, 1, 0, 3]);
        let mut hostname = vec![0; usize::from(request[4])];
        connection
            .read_exact(&mut hostname)
            .await
            .expect("the proxy receives the hostname");
        let mut port = [0; 2];
        connection
            .read_exact(&mut port)
            .await
            .expect("the proxy receives the port");
        connection
            .write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0])
            .await
            .expect("the proxy reports a connection error");
        (hostname, u16::from_be_bytes(port))
    });
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
    let state = authorization_url
        .query_pairs()
        .find(|(name, _)| name == "state")
        .expect("the authorization URL has a state")
        .1;
    application.send_message(&format!(
        "http://127.0.0.1:1455/auth/callback?code=example&client_id=example&state={state}"
    ));
    let (hostname, port) = tokio::time::timeout(Duration::from_secs(5), proxy)
        .await
        .expect("Shodan contacts the proxy")
        .expect("the proxy completes");
    assert_eq!(hostname, b"auth.openai.com");
    assert_eq!(port, 443);
    application
        .wait_for_screen(|screen| {
            let history = read_history(screen);
            history.contains("Token request failed through the configured proxy")
        })
        .await;
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[tokio::test]
async fn user_connects_with_existing_configuration() {
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let configuration_directory = home_directory.join(".shodan");
    std::fs::create_dir_all(&configuration_directory)
        .expect("the configuration directory can be created");
    let (address, mut requests) = RecordedLlama::new().start().await;
    let configuration = json!({"default_provider": "llama", "providers": {"llama": {"address": address, "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}]}}}).to_string();
    let path = configuration_directory.join("settings.json");
    std::fs::write(&path, &configuration).expect("the configuration can be written");
    let mut application = run_test_application(
        build_settings(home_directory.clone(), Vec::new()).expect("the settings can be built"),
    );
    application
        .wait_for_screen(|screen| read_bottom_line(screen).contains("Inkling-2-Mini-UD-IQ3_XXS"))
        .await;
    application.send_message("Hello");
    let request = receive_request(&mut requests).await;
    send_response_bytes(
        &request,
        include_str!("fixtures/llama/chat/inkling-hello-low.txt"),
    )
    .await;
    application
        .wait_for_screen(|screen| read_history(screen).contains("Hello! How can I help you today?"))
        .await;
    assert_eq!(
        std::fs::read_to_string(path).expect("the configuration is readable"),
        configuration
    );
    drop(application);
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}

#[test]
fn user_starts_with_invalid_proxy_settings() {
    let home_directory = std::env::temp_dir().join(format!("shodan-home-{}", uuid::Uuid::new_v4()));
    let configuration_directory = home_directory.join(".shodan");
    std::fs::create_dir_all(&configuration_directory)
        .expect("the configuration directory can be created");
    let path = configuration_directory.join("settings.json");
    std::fs::write(
        &path,
        json!({"proxy": {"url": "invalid", "providers": ["openai"]}, "default_provider": "llama", "providers": {"llama": {"address": "http://127.0.0.1:8080", "default_model": "Inkling-2-Mini-UD-IQ3_XXS", "models": [{"name": "Inkling-2-Mini-UD-IQ3_XXS", "thinking_level": "low"}]}}}).to_string(),
    )
    .expect("the configuration can be written");
    let result = build_settings(home_directory.clone(), Vec::new());
    let Err(error) = result else {
        panic!("invalid proxy configuration must fail");
    };
    assert!(error.to_string().contains("proxy"));
    assert!(error.to_string().contains("settings.json"));
    std::fs::remove_dir_all(home_directory).expect("the home directory can be removed");
}
