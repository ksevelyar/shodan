use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    application::ApplicationError,
    models::Model,
    providers::{self, ProviderSelection as ProviderChoice, ProviderSettings},
    session::Session,
};

#[derive(Clone, Deserialize)]
pub(crate) struct ProxySettings {
    #[serde(deserialize_with = "deserialize_proxy_url")]
    url: reqwest::Url,
    providers: Vec<String>,
}

fn deserialize_proxy_url<'de, Deserializer>(
    deserializer: Deserializer,
) -> Result<reqwest::Url, Deserializer::Error>
where
    Deserializer: serde::Deserializer<'de>,
{
    let url = String::deserialize(deserializer)?;
    reqwest::Url::parse(&url)
        .map_err(|error| serde::de::Error::custom(format!("invalid proxy.url: {error}")))
}

impl ProxySettings {
    pub fn validate(&self) -> io::Result<()> {
        if !matches!(self.url.scheme(), "socks5" | "socks5h")
            || self.url.host_str().is_none()
            || self.url.port().is_none()
        {
            return Err(io::Error::other(
                "proxy.url must be a SOCKS5 URL with a host and port",
            ));
        }
        if self.providers.iter().any(String::is_empty) {
            return Err(io::Error::other(
                "proxy.providers must contain provider names",
            ));
        }
        Ok(())
    }

    pub fn get_for_provider(&self, provider: &str) -> Option<&reqwest::Url> {
        self.providers
            .iter()
            .any(|name| name == provider)
            .then_some(&self.url)
    }
}

#[derive(Deserialize)]
struct ModelConfiguration {
    default_model: String,
    models: Vec<Model>,
}

#[derive(Deserialize)]
struct LlamaConfiguration {
    address: String,
    #[serde(flatten)]
    models: ModelConfiguration,
}

#[derive(Deserialize)]
struct OpenaiConfiguration {
    address: Option<String>,
    authentication_address: Option<String>,
    #[serde(flatten)]
    models: ModelConfiguration,
}

#[derive(Deserialize)]
struct ProvidersConfiguration {
    llama: Option<LlamaConfiguration>,
    openai: Option<OpenaiConfiguration>,
}

#[derive(Deserialize)]
pub(crate) struct Settings {
    default_provider: String,
    providers: ProvidersConfiguration,
    pub proxy: Option<ProxySettings>,
    clipboard_command: Option<Vec<String>>,
}

pub(crate) enum Mode {
    Interactive,
    OpenaiLogin,
    SinglePrompt(String),
}

pub struct ApplicationSettings {
    pub(crate) home_directory: PathBuf,
    pub(crate) provider: ProviderSettings,
    pub(crate) mode: Mode,
    pub(crate) clipboard_command: Option<Vec<String>>,
    pub(crate) session_number: Option<usize>,
    pub(crate) blender_server_command: Option<Vec<String>>,
}

impl ApplicationSettings {
    #[must_use]
    pub fn with_blender_server_command(mut self, command: Option<Vec<String>>) -> Self {
        self.blender_server_command = command;
        self
    }

    #[must_use]
    pub fn with_clipboard_command(mut self, command: Vec<String>) -> Self {
        self.clipboard_command = Some(command);
        self
    }
}

struct ProviderSelection {
    kind: ProviderChoice,
    models: Vec<Model>,
    default_model: String,
}

#[derive(Default)]
struct Arguments {
    provider: Option<String>,
    model: Option<String>,
    record: bool,
    login: bool,
    prompt: Option<String>,
    session_number: Option<usize>,
}

#[doc = "# Errors\nReturns an error if arguments or configuration are invalid."]
pub fn build_settings(
    home_directory: PathBuf,
    arguments: impl IntoIterator<Item = String>,
) -> Result<ApplicationSettings, ApplicationError> {
    let arguments = parse_arguments(arguments)
        .map_err(|error| ApplicationError::Arguments(format!("Invalid arguments: {error}")))?;
    if arguments.login && arguments.prompt.is_some() {
        return Err(ApplicationError::Arguments(
            "login cannot be combined with --prompt".to_owned(),
        ));
    }
    let settings = load_settings(&home_directory).map_err(|error| {
        ApplicationError::Configuration(format!("Failed to load configuration: {error}"))
    })?;
    let saved_selection = arguments
        .session_number
        .map(|number| Session::get_saved_selection(&home_directory, number))
        .transpose()?;
    let provider = if arguments.login {
        Some("openai")
    } else if let Some((provider, _)) = &saved_selection {
        Some(provider.as_str())
    } else {
        arguments.provider.as_deref()
    };
    let selection = settings
        .select_provider(provider, arguments.login)
        .map_err(|error| {
            ApplicationError::Configuration(format!("Invalid provider or model selection: {error}"))
        })?;
    let mode = if arguments.login {
        Mode::OpenaiLogin
    } else if let Some(prompt) = arguments.prompt {
        Mode::SinglePrompt(prompt)
    } else {
        Mode::Interactive
    };
    let application_settings = ApplicationSettings {
        home_directory: home_directory.clone(),
        provider: ProviderSettings {
            home_directory,
            models: selection.models,
            default_model: selection.default_model,
            model_override: saved_selection.map_or(arguments.model, |(_, model)| Some(model)),
            proxy: settings
                .proxy
                .as_ref()
                .and_then(|proxy| proxy.get_for_provider(selection.kind.get_name()))
                .cloned(),
            selection: selection.kind,
            record: arguments.record,
        },
        mode,
        clipboard_command: settings.clipboard_command,
        session_number: arguments.session_number,
        blender_server_command: Some(vec!["blender-mcp-server".to_owned()]),
    };
    Session::select_model_position(&application_settings.provider)?;
    Ok(application_settings)
}

fn parse_arguments(arguments: impl IntoIterator<Item = String>) -> io::Result<Arguments> {
    let mut parsed = Arguments::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "login" => {
                let provider = arguments.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "login requires a provider")
                })?;
                if provider != "openai" {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unknown login provider: {provider}"),
                    ));
                }
                parsed.login = true;
            }
            "--provider" => {
                parsed.provider = Some(arguments.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--provider requires a value")
                })?);
            }
            "--model" => {
                parsed.model = Some(arguments.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--model requires a value")
                })?);
            }
            "--prompt" => {
                parsed.prompt = Some(arguments.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--prompt requires a value")
                })?);
            }
            "--session" => {
                let value = arguments.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--session requires a number")
                })?;
                parsed.session_number = Some(value.parse().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid session number: {value}"),
                    )
                })?);
            }
            "--record" => parsed.record = true,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {argument}"),
                ));
            }
        }
    }
    Ok(parsed)
}

impl Settings {
    fn select_provider(
        &self,
        provider: Option<&str>,
        login: bool,
    ) -> io::Result<ProviderSelection> {
        let default_openai = if login && self.providers.openai.is_none() {
            Some(
                serde_json::from_value::<OpenaiConfiguration>(
                    providers::openai::get_default_definition(),
                )
                .map_err(io::Error::other)?,
            )
        } else {
            None
        };
        let provider = provider.unwrap_or(&self.default_provider);
        let (kind, configuration) = match provider {
            "llama" => {
                let configuration = self.providers.llama.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("provider not configured: {provider}"),
                    )
                })?;
                (
                    ProviderChoice::Llama {
                        address: configuration.address.clone(),
                    },
                    &configuration.models,
                )
            }
            "openai" => {
                let configuration = self
                    .providers
                    .openai
                    .as_ref()
                    .or(default_openai.as_ref())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("provider not configured: {provider}"),
                        )
                    })?;
                (
                    ProviderChoice::Openai {
                        address: configuration
                            .address
                            .clone()
                            .unwrap_or_else(|| "https://api.openai.com".to_owned()),
                        authentication_address: configuration
                            .authentication_address
                            .clone()
                            .unwrap_or_else(|| "https://auth.openai.com".to_owned()),
                    },
                    &configuration.models,
                )
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("provider not configured or supported: {provider}"),
                ));
            }
        };
        Ok(ProviderSelection {
            kind,
            models: configuration.models.clone(),
            default_model: configuration.default_model.clone(),
        })
    }
}

fn create_configuration(home_directory: &Path) -> io::Result<()> {
    let directory = home_directory.join(".shodan");
    fs::create_dir_all(&directory)?;
    let path = directory.join("settings.json");
    let configuration = json!({
        "default_provider": "llama",
        "proxy": {"url": "socks5h://127.0.0.1:2081", "providers": ["openai"]},
        "providers": {"llama": providers::llama::get_default_definition()}
    });
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            serde_json::to_writer_pretty(&mut file, &configuration)?;
            writeln!(file)?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

fn read_configuration(path: &Path) -> io::Result<Value> {
    let contents = fs::read(path)?;
    let configuration: Value = serde_json::from_slice(&contents)
        .map_err(|error| invalid_configuration(path, &error.to_string()))?;
    if !configuration.is_object() {
        return Err(invalid_configuration(path, "expected a JSON object"));
    }
    Ok(configuration)
}

fn invalid_configuration(path: &Path, detail: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("Invalid configuration {}: {detail}", path.display()),
    )
}

pub(crate) fn load_settings(home_directory: &Path) -> io::Result<Settings> {
    create_configuration(home_directory)?;
    let path = home_directory.join(".shodan/settings.json");
    let configuration = read_configuration(&path)?;
    let settings: Settings = serde_json::from_value(configuration)
        .map_err(|error| invalid_configuration(&path, &error.to_string()))?;
    if let Some(proxy) = &settings.proxy {
        proxy
            .validate()
            .map_err(|error| invalid_configuration(&path, &error.to_string()))?;
    }
    Ok(settings)
}

pub(crate) fn get_device_identifier(directory: &Path) -> io::Result<String> {
    let path = directory.join("settings.json");
    let mut configuration = read_configuration(&path)?;
    if let Some(identifier) = configuration.get("device_id") {
        let identifier = identifier
            .as_str()
            .ok_or_else(|| invalid_configuration(&path, "device_id must be a UUID"))?;
        uuid::Uuid::parse_str(identifier)
            .map_err(|_| invalid_configuration(&path, "device_id must be a UUID"))?;
        return Ok(identifier.to_owned());
    }
    let identifier = uuid::Uuid::new_v4().to_string();
    configuration["device_id"] = json!(identifier);
    let mut file = fs::File::create(path)?;
    serde_json::to_writer_pretty(&mut file, &configuration)?;
    writeln!(file)?;
    Ok(identifier)
}

pub(crate) fn add_provider(directory: &Path, provider: &str, definition: Value) -> io::Result<()> {
    let path = directory.join("settings.json");
    let mut configuration = read_configuration(&path)?;
    let providers = configuration
        .get_mut("providers")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid_configuration(&path, "providers must be an object"))?;
    if !providers.contains_key(provider) {
        providers.insert(provider.to_owned(), definition);
        let mut file = fs::File::create(path)?;
        serde_json::to_writer_pretty(&mut file, &configuration)?;
        writeln!(file)?;
    }
    Ok(())
}
