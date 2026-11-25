pub(crate) mod llama;
pub(crate) mod openai;

use std::io::Write;

use futures_util::{Stream, StreamExt};

use crate::application::ApplicationError;

pub(crate) async fn stream_response(
    provider: &Provider,
    model: &str,
    thinking_level: Option<crate::models::ThinkingLevel>,
    messages: &[crate::chats::Message],
    turn: u64,
    event_sender: &crate::events::EventSender,
) {
    match &provider.kind {
        ProviderKind::Llama { address } => {
            llama::stream_response(
                &provider.connection,
                address,
                model,
                thinking_level,
                messages,
                turn,
                event_sender,
            )
            .await;
        }
        ProviderKind::Openai(openai) => {
            openai::stream_response(
                &provider.connection,
                openai,
                model,
                thinking_level,
                messages,
                turn,
                event_sender,
            )
            .await;
        }
    }
}

fn build_client(proxy: Option<&reqwest::Url>) -> Result<reqwest::Client, ApplicationError> {
    let mut builder = reqwest::Client::builder().no_proxy();
    if let Some(proxy) = proxy {
        let proxy = reqwest::Proxy::all(proxy.clone()).map_err(|error| {
            ApplicationError::Provider(format!("Failed to initialize provider client: {error}"))
        })?;
        builder = builder.proxy(proxy);
    }
    builder.build().map_err(|error| {
        ApplicationError::Provider(format!("Failed to initialize provider client: {error}"))
    })
}

#[derive(Clone)]
pub(crate) enum ProviderSelection {
    Llama {
        address: String,
    },
    Openai {
        address: String,
        authentication_address: String,
    },
}

impl ProviderSelection {
    pub fn get_name(&self) -> &'static str {
        match self {
            Self::Llama { .. } => "llama",
            Self::Openai { .. } => "openai",
        }
    }
}

pub(crate) struct ProviderSettings {
    pub home_directory: std::path::PathBuf,
    pub selection: ProviderSelection,
    pub models: Vec<crate::models::Model>,
    pub default_model: String,
    pub model_override: Option<String>,
    pub proxy: Option<reqwest::Url>,
    pub record: bool,
}

#[derive(Clone)]
pub(crate) struct ProviderConnection {
    client: reqwest::Client,
    proxy: Option<reqwest::Url>,
    record: bool,
    provider_name: &'static str,
}

#[derive(Clone)]
pub(crate) struct Provider {
    pub models: Vec<crate::models::Model>,
    kind: ProviderKind,
    pub(crate) connection: ProviderConnection,
}

#[derive(Clone)]
enum ProviderKind {
    Llama { address: String },
    Openai(openai::OpenaiProvider),
}

impl Provider {
    pub(crate) fn get_openai_authentication_address(&self) -> Option<&str> {
        match &self.kind {
            ProviderKind::Openai(provider) => Some(&provider.authentication_address),
            ProviderKind::Llama { .. } => None,
        }
    }

    pub(crate) fn get_name(&self) -> &'static str {
        match self.kind {
            ProviderKind::Llama { .. } => "llama",
            ProviderKind::Openai(_) => "openai",
        }
    }
}

pub(crate) fn build(settings: ProviderSettings) -> Result<Provider, ApplicationError> {
    let connection = ProviderConnection {
        client: build_client(settings.proxy.as_ref())?,
        proxy: settings.proxy,
        record: settings.record,
        provider_name: settings.selection.get_name(),
    };
    let kind = match settings.selection {
        ProviderSelection::Llama { address } => ProviderKind::Llama { address },
        ProviderSelection::Openai {
            address,
            authentication_address,
        } => ProviderKind::Openai(openai::OpenaiProvider {
            address,
            authentication_address,
            home_directory: settings.home_directory,
        }),
    };
    Ok(Provider {
        models: settings.models,
        kind,
        connection,
    })
}

struct Recording {
    file: std::fs::File,
    path: std::path::PathBuf,
}

impl Recording {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ApplicationError> {
        self.file.write_all(bytes).map_err(|error| {
            ApplicationError::Recording(format!(
                "Failed to write recording {}: {error}",
                self.path.display()
            ))
        })
    }
}

impl ProviderConnection {
    async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, ApplicationError> {
        request.send().await.map_err(|error| {
            let message = if self.proxy.is_some() {
                "Request through the configured proxy failed"
            } else {
                "Provider request failed"
            };
            ApplicationError::Provider(format!("{message}: {error}"))
        })
    }

    fn record_stream(
        &self,
        response: reqwest::Response,
        path: &str,
    ) -> Result<
        impl Stream<Item = Result<impl AsRef<[u8]> + use<>, ApplicationError>> + use<>,
        ApplicationError,
    > {
        let mut recording = self.create_recording(path, "txt")?;
        Ok(response.bytes_stream().map(move |chunk| {
            let chunk = chunk.map_err(|error| {
                ApplicationError::Provider(format!("Failed to read provider response: {error}"))
            })?;
            if let Some(recording) = &mut recording {
                recording.write_all(&chunk)?;
            }
            Ok(chunk)
        }))
    }

    fn create_recording(
        &self,
        path: &str,
        extension: &str,
    ) -> Result<Option<Recording>, ApplicationError> {
        if !self.record {
            return Ok(None);
        }
        let directory = std::path::Path::new("recordings")
            .join(self.provider_name)
            .join(path.trim_matches('/').replace('/', "-"));
        std::fs::create_dir_all(&directory).map_err(|error| {
            ApplicationError::Recording(format!(
                "Failed to create recording directory {}: {error}",
                directory.display()
            ))
        })?;
        let filename = format!("{}.{extension}", uuid::Uuid::new_v4());
        let path = directory.join(filename);
        let file = std::fs::File::create(&path).map_err(|error| {
            ApplicationError::Recording(format!(
                "Failed to create recording {}: {error}",
                path.display()
            ))
        })?;
        Ok(Some(Recording { file, path }))
    }
}
