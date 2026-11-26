use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    extract::{OriginalUri, State},
    http::StatusCode,
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use crate::{
    application::ApplicationError,
    chats::{Message, ToolCall},
    events::{Event, EventSender},
    models::ThinkingLevel,
};

#[derive(Deserialize, Serialize)]
struct Credential {
    #[serde(rename = "type")]
    kind: String,
    access: String,
    refresh: String,
    expires: u64,
    #[serde(rename = "clientId")]
    client_identifier: String,
    scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id_token: Option<String>,
}

fn get_current_time() -> Result<u64, ApplicationError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ApplicationError::Provider(format!("Cannot read system time: {error}")))?;
    u64::try_from(duration.as_millis())
        .map_err(|error| ApplicationError::Provider(format!("Invalid system time: {error}")))
}

fn read_credential(path: &Path) -> Result<Credential, ApplicationError> {
    let contents = fs::read(path).map_err(|_| {
        ApplicationError::Provider(
            "Cannot read OpenAI credentials. Run shodan login openai".to_owned(),
        )
    })?;
    let credential: Credential = serde_json::from_slice(&contents).map_err(|_| {
        ApplicationError::Provider("Invalid OpenAI credentials. Run shodan login openai".to_owned())
    })?;
    if credential.kind != "oauth"
        || credential.access.is_empty()
        || credential.refresh.is_empty()
        || credential.client_identifier.is_empty()
        || !credential
            .scopes
            .iter()
            .any(|scope| scope == "chatgpt.tokens.use.direct")
    {
        return Err(ApplicationError::Provider(
            "Invalid OpenAI credentials. Run shodan login openai".to_owned(),
        ));
    }
    Ok(credential)
}

fn write_credential(path: &Path, credential: &impl Serialize) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("credential directory is missing"))?;
    let temporary_path = directory.join(format!(".openai-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(&temporary_path)?;
        serde_json::to_writer_pretty(&mut file, credential)?;
        writeln!(file)?;
        file.sync_all()?;
        fs::rename(&temporary_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

async fn get_access_token(
    connection: &super::ProviderConnection,
    authentication_address: &str,
    home_directory: &Path,
) -> Result<String, ApplicationError> {
    let credential = read_credential(&home_directory.join(".shodan/openai.json"))?;
    if credential.expires > get_current_time()? {
        return Ok(credential.access);
    }
    let connection = connection.clone();
    let authentication_address = authentication_address.to_owned();
    let home_directory = home_directory.to_owned();
    tokio::spawn(async move {
        refresh_access_token(&connection, &authentication_address, &home_directory).await
    })
    .await
    .map_err(|error| {
        ApplicationError::Provider(format!("OpenAI credential task failed: {error}"))
    })?
}

async fn refresh_access_token(
    connection: &super::ProviderConnection,
    authentication_address: &str,
    home_directory: &Path,
) -> Result<String, ApplicationError> {
    let path = home_directory.join(".shodan/openai.json");
    let lock_path = home_directory.join(".shodan/openai.lock");
    let lock = tokio::task::spawn_blocking(move || {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(lock_path)?;
        file.lock()?;
        Ok::<_, io::Error>(file)
    })
    .await
    .map_err(|error| {
        ApplicationError::Provider(format!("Cannot lock OpenAI credentials: {error}"))
    })?
    .map_err(|error| {
        ApplicationError::Provider(format!("Cannot lock OpenAI credentials: {error}"))
    })?;
    let mut credential = read_credential(&path)?;
    if credential.expires <= get_current_time()? {
        let body = reqwest::Url::parse_with_params(
            "http://localhost",
            [
                ("grant_type", "refresh_token"),
                ("client_id", credential.client_identifier.as_str()),
                ("refresh_token", credential.refresh.as_str()),
                ("resource", "https://api.openai.com/v1"),
            ],
        )
        .expect("refresh request parameters form a URL")
        .query()
        .expect("refresh request has parameters")
        .to_owned();
        let response = connection
            .send(
                connection
                    .client
                    .post(format!("{authentication_address}/api/accounts/oauth/token"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(body),
            )
            .await?;
        if !response.status().is_success() {
            return Err(ApplicationError::Provider(format!(
                "OpenAI credential refresh failed ({}). Run shodan login openai",
                response.status()
            )));
        }
        let token: TokenResponse = response.json().await.map_err(|_| {
            ApplicationError::Provider(
                "Invalid OpenAI refresh response. Run shodan login openai".to_owned(),
            )
        })?;
        if token.access_token.is_empty() || token.refresh_token.is_empty() || token.expires_in == 0
        {
            return Err(ApplicationError::Provider(
                "Invalid OpenAI refresh response. Run shodan login openai".to_owned(),
            ));
        }
        credential.access.clone_from(&token.access_token);
        credential.refresh.clone_from(&token.refresh_token);
        credential.scopes = token.scope.split_whitespace().map(str::to_owned).collect();
        credential.expires = get_current_time()?
            .saturating_add(token.expires_in.saturating_mul(1000))
            .saturating_sub(180_000);
        write_credential(&path, &credential).map_err(|error| {
            ApplicationError::Provider(format!("Cannot save refreshed OpenAI credentials: {error}"))
        })?;
        if connection.record {
            record_authentication_response(&token).map_err(|error| {
                ApplicationError::Recording(format!(
                    "Cannot save sanitized authentication response: {error}"
                ))
            })?;
        }
        if !credential
            .scopes
            .iter()
            .any(|scope| scope == "chatgpt.tokens.use.direct")
        {
            return Err(ApplicationError::Provider(
                "OpenAI plan permission is missing. Run shodan login openai".to_owned(),
            ));
        }
    }
    drop(lock);
    Ok(credential.access)
}

fn build_input(messages: &[Message]) -> (String, Vec<Value>) {
    let mut instructions = String::new();
    let mut input = Vec::new();
    for message in messages {
        match message {
            Message::System(content) => instructions.push_str(content),
            Message::User(content) => input.push(json!({"role": "user", "content": content})),
            Message::Model {
                provider_items,
                content,
                tool_calls,
                aborted: false,
                ..
            } => {
                if provider_items.is_empty() {
                    if !content.is_empty() {
                        input.push(json!({"role": "assistant", "content": content}));
                    }
                    for call in tool_calls {
                        input.push(json!({"type": "function_call", "call_id": call.id, "name": call.name, "arguments": call.arguments}));
                    }
                } else {
                    input.extend(provider_items.iter().cloned());
                }
            }
            Message::ToolResult {
                call_id, content, ..
            } => input.push(
                json!({"type": "function_call_output", "call_id": call_id, "output": content}),
            ),
            Message::Model { aborted: true, .. } | Message::FailedResponse(_) => {}
        }
    }
    (instructions, input)
}

fn describe_openai_tools(read_context: bool) -> Vec<Value> {
    crate::tools::describe_tools(read_context).into_iter().map(|tool| {
        let function = &tool["function"];
        json!({"type": "function", "name": function["name"], "description": function["description"], "parameters": function["parameters"], "strict": false})
    }).collect()
}

#[derive(Deserialize)]
struct ResponseUsage {
    input_tokens: u64,
    output_tokens: u64,
    input_tokens_details: InputTokenDetails,
}

#[derive(Deserialize)]
struct InputTokenDetails {
    cached_tokens: u64,
}

#[derive(Deserialize)]
struct CompletedResponse {
    output: Vec<Value>,
    usage: ResponseUsage,
}

#[derive(Deserialize)]
struct FailedResponse {
    error: Option<ProviderError>,
}

#[derive(Deserialize)]
struct ProviderError {
    message: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ResponseEvent {
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta { delta: String },
    #[serde(rename = "response.reasoning_summary_text.delta")]
    ReasoningSummaryDelta { delta: String },
    #[serde(rename = "response.completed")]
    Completed { response: CompletedResponse },
    #[serde(rename = "response.failed")]
    Failed { response: FailedResponse },
    #[serde(rename = "response.incomplete")]
    Incomplete { response: FailedResponse },
    #[serde(rename = "error")]
    Error { message: Option<String> },
    #[serde(other)]
    Other,
}

#[derive(Clone)]
pub(crate) struct OpenaiProvider {
    pub address: String,
    pub authentication_address: String,
    pub home_directory: std::path::PathBuf,
}

pub(crate) async fn stream_response(
    connection: &super::ProviderConnection,
    provider: &OpenaiProvider,
    model: &str,
    thinking_level: Option<ThinkingLevel>,
    messages: &[Message],
    turn: u64,
    event_sender: &EventSender,
) {
    let result = async {
        let token = get_access_token(
            connection,
            &provider.authentication_address,
            &provider.home_directory,
        )
        .await?;
        let (instructions, input) = build_input(messages);
        let mut request = json!({"model": model, "instructions": instructions, "input": input,
            "tools": describe_openai_tools(connection.read_context), "store": false, "stream": true,
            "include": ["reasoning.encrypted_content"]});
        if let Some(thinking_level) = thinking_level {
            request["reasoning"] =
                json!({"effort": thinking_level.as_provider_value(), "summary": "auto"});
        }
        let path = "/v1/responses";
        let response = connection
            .send(
                connection
                    .client
                    .post(format!("{}{path}", provider.address))
                    .bearer_auth(token)
                    .json(&request),
            )
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = if status == reqwest::StatusCode::UNAUTHORIZED {
                ". Run shodan login openai"
            } else {
                ""
            };
            return Err(ApplicationError::Provider(format!(
                "OpenAI returned {status}{detail}"
            )));
        }
        let mut events = connection.record_stream(response, path)?.eventsource();
        while let Some(event) = events.next().await {
            let event = event.map_err(|error| match error {
                eventsource_stream::EventStreamError::Transport(error) => error,
                error => {
                    ApplicationError::Provider(format!("Invalid OpenAI event stream: {error}"))
                }
            })?;
            let event: ResponseEvent = serde_json::from_str(&event.data).map_err(|error| {
                ApplicationError::Provider(format!("Invalid OpenAI response: {error}"))
            })?;
            match event {
                ResponseEvent::OutputTextDelta { delta } => {
                    let _ = event_sender.send(Event::MessageDelta {
                        turn,
                        content: delta,
                    });
                }
                ResponseEvent::ReasoningSummaryDelta { delta } => {
                    let _ = event_sender.send(Event::ThinkingDelta {
                        turn,
                        content: delta,
                    });
                }
                ResponseEvent::Completed { response } => {
                    return finish_response(response, turn, event_sender);
                }
                ResponseEvent::Failed { response } | ResponseEvent::Incomplete { response } => {
                    let message = response
                        .error
                        .and_then(|error| error.message)
                        .unwrap_or_else(|| "OpenAI response did not complete".to_owned());
                    return Err(ApplicationError::Provider(message));
                }
                ResponseEvent::Error { message } => {
                    return Err(ApplicationError::Provider(
                        message.unwrap_or_else(|| "OpenAI response did not complete".to_owned()),
                    ));
                }
                ResponseEvent::Other => {}
            }
        }
        Err(ApplicationError::Provider(
            "OpenAI response stream ended before completion".to_owned(),
        ))
    }
    .await;
    if let Err(error) = result {
        let _ = event_sender.send(Event::RequestFailed { turn, error });
    }
}

fn finish_response(
    response: CompletedResponse,
    turn: u64,
    event_sender: &EventSender,
) -> Result<(), ApplicationError> {
    let calls = response
        .output
        .iter()
        .filter(|item| item["type"] == "function_call")
        .map(|item| {
            let get_field = |name: &str| {
                item[name]
                    .as_str()
                    .filter(|field| !field.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        ApplicationError::Provider(format!("Incomplete OpenAI tool call: {name}"))
                    })
            };
            Ok(ToolCall {
                id: get_field("call_id")?,
                name: get_field("name")?,
                arguments: get_field("arguments")?,
            })
        })
        .collect::<Result<Vec<_>, ApplicationError>>()?;
    for call in calls {
        let _ = event_sender.send(Event::ToolCallReceived { turn, call });
    }
    let _ = event_sender.send(Event::ProviderItemsReceived {
        turn,
        items: response.output,
    });
    let _ = event_sender.send(Event::ResponseEnded {
        turn,
        prompt_tokens: response.usage.input_tokens,
        completion_tokens: response.usage.output_tokens,
        cached_tokens: response.usage.input_tokens_details.cached_tokens,
    });
    Ok(())
}

pub(crate) fn get_default_definition() -> Value {
    json!({
        "default_model": "gpt-6.1-terra",
        "models": [
            {"name": "gpt-6.1-terra", "thinking_level": "high"},
            {"name": "gpt-6-luna", "thinking_level": "max"}
        ]
    })
}

fn generate_random_value() -> String {
    let mut bytes = [0_u8; 32];
    bytes[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    URL_SAFE_NO_PAD.encode(bytes)
}

struct Authorization {
    code: String,
    client_identifier: String,
}

enum LoginError {
    Recoverable(String),
    NonRecoverable(String),
}

fn parse_redirect(
    input: &str,
    expected_state: &str,
    expected_client_identifier: Option<&str>,
) -> Result<Authorization, LoginError> {
    let redirect = reqwest::Url::parse(input.trim()).map_err(|_| {
        LoginError::Recoverable("Paste the full callback URL from the browser".to_owned())
    })?;
    if redirect.scheme() != "http"
        || redirect.host_str() != Some("127.0.0.1")
        || redirect.port() != Some(1455)
        || redirect.path() != "/auth/callback"
    {
        return Err(LoginError::Recoverable(
            "The redirect URL must start with http://127.0.0.1:1455/auth/callback".to_owned(),
        ));
    }
    let parameters = redirect
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    let state = parameters
        .get("state")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| LoginError::Recoverable("Missing OAuth state".to_owned()))?;
    if state.as_ref() != expected_state {
        return Err(LoginError::Recoverable("OAuth state mismatch".to_owned()));
    }
    if let Some(error) = parameters.get("error") {
        return Err(LoginError::NonRecoverable(format!(
            "ChatGPT authorization failed: {error}"
        )));
    }
    let code = parameters
        .get("code")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| LoginError::Recoverable("Missing authorization code".to_owned()))?;
    let returned_client_identifier = parameters
        .get("client_id")
        .filter(|value| !value.trim().is_empty())
        .map(std::convert::AsRef::as_ref);
    if expected_client_identifier.is_some()
        && returned_client_identifier.is_some()
        && expected_client_identifier != returned_client_identifier
    {
        return Err(LoginError::Recoverable(
            "OAuth client ID mismatch".to_owned(),
        ));
    }
    let client_identifier = expected_client_identifier
        .or(returned_client_identifier)
        .ok_or_else(|| LoginError::Recoverable("Missing issued client ID".to_owned()))?;
    Ok(Authorization {
        code: code.to_string(),
        client_identifier: client_identifier.to_owned(),
    })
}

type AuthorizationSender = oneshot::Sender<Result<Authorization, String>>;

#[derive(Clone)]
struct CallbackState {
    expected_state: String,
    expected_client_identifier: Option<String>,
    result: Arc<Mutex<Option<AuthorizationSender>>>,
}

async fn handle_callback(
    State(state): State<CallbackState>,
    OriginalUri(uri): OriginalUri,
) -> (StatusCode, &'static str) {
    let redirect = format!("http://127.0.0.1:1455{uri}");
    match parse_redirect(
        &redirect,
        &state.expected_state,
        state.expected_client_identifier.as_deref(),
    ) {
        Ok(authorization) => {
            if let Some(result) = state
                .result
                .lock()
                .expect("callback state is available")
                .take()
            {
                let _ = result.send(Ok(authorization));
            }
            (
                StatusCode::OK,
                "ChatGPT authentication completed. You can close this window.",
            )
        }
        Err(LoginError::NonRecoverable(error)) => {
            if let Some(result) = state
                .result
                .lock()
                .expect("callback state is available")
                .take()
            {
                let _ = result.send(Err(error));
            }
            (StatusCode::BAD_REQUEST, "ChatGPT authentication failed.")
        }
        Err(LoginError::Recoverable(_)) => {
            (StatusCode::BAD_REQUEST, "ChatGPT authentication failed.")
        }
    }
}

#[derive(Deserialize, Serialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    id_token: String,
    scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_type: Option<String>,
    #[serde(flatten)]
    additional_fields: std::collections::BTreeMap<String, Value>,
}

async fn verify_identity(
    id_token: &str,
    client_identifier: &str,
    nonce: &str,
    connection: &super::ProviderConnection,
    authentication_address: &str,
) -> Result<String, String> {
    let invalid_identity = || "Invalid OpenAI identity".to_owned();
    let segments = id_token.split('.').collect::<Vec<_>>();
    let [header, claims, signature] = segments.as_slice() else {
        return Err(invalid_identity());
    };
    let header: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(header)
            .map_err(|_| invalid_identity())?,
    )
    .map_err(|_| invalid_identity())?;
    if header["alg"] != "RS256" {
        return Err(invalid_identity());
    }
    let identifier = header["kid"].as_str().ok_or_else(invalid_identity)?;
    let response = connection
        .send(
            connection
                .client
                .get(format!("{authentication_address}/.well-known/jwks.json")),
        )
        .await
        .map_err(|error| format!("Cannot verify OpenAI identity: {error}"))?;
    if !response.status().is_success() {
        return Err("Cannot fetch OpenAI identity keys".to_owned());
    }
    let keys: Value = response.json().await.map_err(|_| invalid_identity())?;
    let key = keys["keys"]
        .as_array()
        .and_then(|keys| {
            keys.iter()
                .find(|key| key["kid"] == identifier && key["kty"] == "RSA")
        })
        .ok_or_else(invalid_identity)?;
    let modulus = URL_SAFE_NO_PAD
        .decode(key["n"].as_str().ok_or_else(invalid_identity)?)
        .map_err(|_| invalid_identity())?;
    let exponent = URL_SAFE_NO_PAD
        .decode(key["e"].as_str().ok_or_else(invalid_identity)?)
        .map_err(|_| invalid_identity())?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| invalid_identity())?;
    let signed_content = format!("{}.{}", segments[0], segments[1]);
    ring::signature::RsaPublicKeyComponents {
        n: &modulus,
        e: &exponent,
    }
    .verify(
        &ring::signature::RSA_PKCS1_2048_8192_SHA256,
        signed_content.as_bytes(),
        &signature,
    )
    .map_err(|_| invalid_identity())?;
    let claims: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(claims)
            .map_err(|_| invalid_identity())?,
    )
    .map_err(|_| invalid_identity())?;
    let audience_matches = claims["aud"]
        .as_str()
        .is_some_and(|audience| audience == client_identifier)
        || claims["aud"].as_array().is_some_and(|audiences| {
            audiences
                .iter()
                .any(|audience| audience == client_identifier)
        });
    let now = get_current_time().map_err(|error| error.to_string())? / 1000;
    if claims["iss"] != "https://auth.openai.com"
        || claims["nonce"] != nonce
        || !audience_matches
        || claims["exp"]
            .as_u64()
            .is_none_or(|expiration| expiration <= now)
    {
        return Err(invalid_identity());
    }
    claims["sub"]
        .as_str()
        .filter(|subject| !subject.is_empty())
        .map(str::to_owned)
        .ok_or_else(invalid_identity)
}

async fn exchange_authorization(
    authorization: Authorization,
    verifier: &str,
    nonce: &str,
    expected_subject: Option<&str>,
    connection: &super::ProviderConnection,
    authentication_address: &str,
) -> Result<Value, String> {
    let resource = "https://api.openai.com/v1";
    let redirect = "http://127.0.0.1:1455/auth/callback";
    let body = reqwest::Url::parse_with_params(
        "http://localhost",
        [
            ("grant_type", "authorization_code"),
            ("client_id", authorization.client_identifier.as_str()),
            ("code", authorization.code.as_str()),
            ("code_verifier", verifier),
            ("redirect_uri", redirect),
            ("resource", resource),
        ],
    )
    .map_err(|error| error.to_string())?
    .query()
    .expect("token request has parameters")
    .to_owned();
    let response = connection
        .client
        .post(format!("{authentication_address}/api/accounts/oauth/token"))
        .header("accept", "application/json")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|error| {
            if connection.proxy.is_some() {
                format!("Token request failed through the configured proxy: {error}")
            } else {
                format!("Token request failed: {error}")
            }
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "Token request failed with status {}",
            response.status()
        ));
    }
    let token: TokenResponse = response
        .json()
        .await
        .map_err(|_| "Invalid token response".to_owned())?;
    let scopes = token.scope.split_whitespace().collect::<Vec<_>>();
    if token.access_token.trim().is_empty()
        || token.refresh_token.trim().is_empty()
        || token.id_token.trim().is_empty()
        || token.expires_in == 0
        || !scopes.contains(&"chatgpt.tokens.use.direct")
    {
        return Err("Invalid token response or missing direct token scope".to_owned());
    }
    let subject = verify_identity(
        &token.id_token,
        &authorization.client_identifier,
        nonce,
        connection,
        authentication_address,
    )
    .await?;
    if expected_subject.is_some_and(|expected| expected != subject) {
        return Err("Invalid OpenAI identity: account does not match the saved account".to_owned());
    }
    if connection.record {
        record_authentication_response(&token)
            .map_err(|error| format!("Cannot save sanitized authentication response: {error}"))?;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let expires = now
        .saturating_add(u128::from(token.expires_in) * 1000)
        .saturating_sub(180_000);
    Ok(
        json!({"type": "oauth", "access": token.access_token, "refresh": token.refresh_token, "expires": expires, "clientId": authorization.client_identifier, "scopes": scopes, "subject": subject, "id_token": token.id_token}),
    )
}

fn generate_sanitized_token(token: &str) -> String {
    token
        .split('.')
        .map(|_| generate_random_value())
        .collect::<Vec<_>>()
        .join(".")
}

fn record_authentication_response(token: &TokenResponse) -> io::Result<()> {
    let directory = Path::new("recordings/openai/auth-token");
    fs::create_dir_all(directory)?;
    let filename = format!("{}.json", uuid::Uuid::new_v4());
    let mut file = fs::File::create(directory.join(filename))?;
    let mut response = serde_json::to_value(token)?;
    response["access_token"] = json!(generate_sanitized_token(&token.access_token));
    response["refresh_token"] = json!(generate_sanitized_token(&token.refresh_token));
    response["id_token"] = json!(generate_sanitized_token(&token.id_token));
    serde_json::to_writer_pretty(&mut file, &response)?;
    writeln!(file)
}

fn save_login(directory: &Path, credential: &Value) -> io::Result<()> {
    write_credential(&directory.join("openai.json"), credential)?;
    crate::settings::add_provider(directory, "openai", get_default_definition())
}

fn build_authorization_url(
    identifier: &str,
    saved_client_identifier: Option<&str>,
    state: &str,
    challenge: &str,
    nonce: &str,
) -> Result<String, String> {
    let host = format!("urn:uuid:{identifier}");
    let mut parameters = vec![
        (
            "client_id",
            saved_client_identifier.unwrap_or("dynamic_agent_client"),
        ),
        ("ext_agent_host_id", host.as_str()),
        ("response_type", "code"),
        ("redirect_uri", "http://127.0.0.1:1455/auth/callback"),
        ("resource", "https://api.openai.com/v1"),
        (
            "scope",
            "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct",
        ),
        ("state", state),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("nonce", nonce),
    ];
    if saved_client_identifier.is_none() {
        parameters.push(("agent_name_hint", "Shodan"));
    }
    reqwest::Url::parse_with_params("https://auth.openai.com/api/accounts/authorize", parameters)
        .map(|url| url.to_string())
        .map_err(|error| error.to_string())
}

pub(crate) async fn perform_login(
    home_directory: &Path,
    mut input: mpsc::UnboundedReceiver<String>,
    events: &EventSender,
    connection: &super::ProviderConnection,
    authentication_address: &str,
    clipboard_command: Option<&[String]>,
) -> Result<(), String> {
    let directory = home_directory.join(".shodan");
    let saved_credential = read_credential(&directory.join("openai.json")).ok();
    let saved_client_identifier = saved_credential
        .as_ref()
        .map(|credential| credential.client_identifier.as_str());
    let identifier =
        crate::settings::get_device_identifier(&directory).map_err(|error| error.to_string())?;
    let verifier = generate_random_value();
    let verifier_digest = ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(verifier_digest.as_ref());
    let state = generate_random_value();
    let nonce = generate_random_value();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:1455")
        .await
        .map_err(|error| format!("Cannot start login callback on port 1455: {error}"))?;
    let (result, callback) = oneshot::channel();
    let callback_state = CallbackState {
        expected_state: state.clone(),
        expected_client_identifier: saved_client_identifier.map(str::to_owned),
        result: Arc::new(Mutex::new(Some(result))),
    };
    let router = Router::new()
        .route("/auth/callback", get(handle_callback))
        .with_state(callback_state);
    let server = axum::serve(listener, router);
    let authorization_url = build_authorization_url(
        &identifier,
        saved_client_identifier,
        &state,
        &challenge,
        &nonce,
    )?;
    let copied = crate::interface::copy_to_clipboard(&authorization_url, clipboard_command).await;
    let mut message = format!(
        "{authorization_url}\n\n│ Complete sign-in on this machine, or paste the redirect URL below and press Enter."
    );
    if copied {
        message.push_str("\n│ The authorization URL was copied to your clipboard.");
    }
    let _ = events.send(Event::LoginMessage(message));
    let mut callback = callback;
    let mut server = Box::pin(std::future::IntoFuture::into_future(server));
    let authorization = loop {
        tokio::select! {
            result = &mut callback => {
                break result.map_err(|_| "Login callback closed".to_owned())??;
            }
            redirect = input.recv() => {
                let redirect = redirect.ok_or("Login input closed")?;
                match parse_redirect(&redirect, &state, saved_client_identifier) {
                    Ok(authorization) => break authorization,
                    Err(LoginError::Recoverable(error)) => {
                        let _ = events.send(Event::LoginMessage(error));
                    }
                    Err(LoginError::NonRecoverable(error)) => return Err(error),
                }
            }
            result = &mut server => {
                let error = result.err().map_or_else(
                    || "server closed".to_owned(),
                    |error| error.to_string(),
                );
                return Err(format!("Login callback failed: {error}"));
            }
        }
    };
    let credential = exchange_authorization(
        authorization,
        &verifier,
        &nonce,
        saved_credential
            .as_ref()
            .and_then(|credential| credential.subject.as_deref()),
        connection,
        authentication_address,
    )
    .await?;
    save_login(&directory, &credential).map_err(|error| error.to_string())?;
    Ok(())
}
