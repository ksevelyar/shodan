use std::{error::Error, fmt, io, io::Write};

use crossterm::event::Event;
use futures_util::Stream;
use ratatui::backend::Backend;
use tokio::{sync::mpsc, time::Instant};

use crate::{
    chats,
    events::{self, EventSender},
    interface::{self, build_interface},
    providers,
    session::Session,
    settings::{ApplicationSettings, Mode},
};

pub use crate::{
    events::Events,
    interface::{Interface, TerminalMode},
};

#[derive(Debug)]
pub enum ApplicationError {
    Arguments(String),
    Configuration(String),
    Provider(String),
    Recording(String),
    Terminal(String),
    Output(String),
    Tool(String),
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (category, message) = match self {
            Self::Arguments(message) => ("arguments", message),
            Self::Configuration(message) => ("configuration", message),
            Self::Provider(message) => ("provider", message),
            Self::Recording(message) => ("recording", message),
            Self::Terminal(message) => ("terminal", message),
            Self::Output(message) => ("output", message),
            Self::Tool(message) => ("tool", message),
        };
        write!(formatter, "{category}: {message}")
    }
}

impl Error for ApplicationError {}

pub struct Application {
    pub(crate) session: Session,
    pub(crate) error: Option<ApplicationError>,
    pub(crate) event_sender: EventSender,
    pub(crate) active_request: Option<tokio::task::JoinHandle<()>>,
    pub(crate) response_started: Option<Instant>,
    pub(crate) pending_tool_calls: std::collections::VecDeque<crate::tools::ToolCall>,
    home_directory: std::path::PathBuf,
    pub(crate) mode: Mode,
    pub(crate) pending_authorization: Option<mpsc::UnboundedSender<String>>,
    pub(crate) clipboard_command: Option<Vec<String>>,
    pub(crate) blender_connection: Option<std::sync::Arc<crate::mcp::Connection>>,
    blender_server_command: Option<Vec<String>>,
}

impl Drop for Application {
    fn drop(&mut self) {
        if let Some(request) = &self.active_request {
            request.abort();
        }
    }
}

#[doc = "# Errors\nReturns an error if the provider client cannot be initialized."]
pub fn build(settings: ApplicationSettings) -> Result<(Application, Events), ApplicationError> {
    let mut session = Session::build(settings.provider, settings.session_number)?;
    if matches!(settings.mode, Mode::OpenaiLogin) {
        session.messages.clear();
    }
    let (event_sender, background_events) = mpsc::unbounded_channel();
    let application = Application {
        session,
        error: None,
        event_sender,
        active_request: None,
        response_started: None,
        pending_tool_calls: std::collections::VecDeque::new(),
        home_directory: settings.home_directory,
        clipboard_command: settings.clipboard_command,
        blender_connection: None,
        blender_server_command: settings.blender_server_command,
        mode: settings.mode,
        pending_authorization: None,
    };
    Ok((application, background_events))
}

#[doc = "# Errors\nReturns an error if the application cannot run."]
pub async fn run<BackendType, EventSource>(
    settings: ApplicationSettings,
    create_terminal_resources: impl FnOnce() -> (BackendType, EventSource, TerminalMode),
) -> Result<(), ApplicationError>
where
    BackendType: Backend,
    BackendType::Error: Send + Sync + 'static,
    EventSource: Stream<Item = io::Result<Event>> + Unpin,
{
    let (mut application, mut background_events) = build(settings)?;
    if !matches!(application.mode, Mode::OpenaiLogin)
        && let Some(command) = application.blender_server_command.take()
    {
        match crate::mcp::Connection::connect(&command).await {
            Ok(connection) => application.blender_connection = Some(connection),
            Err(error) => {
                if matches!(application.mode, Mode::SinglePrompt(_)) {
                    eprintln!("MCP connection failed: {error}");
                } else {
                    application.error = Some(ApplicationError::Tool(format!(
                        "MCP connection failed: {error}"
                    )));
                }
            }
        }
    }
    match &application.mode {
        Mode::SinglePrompt(prompt) => {
            let prompt = prompt.clone();
            run_single_prompt(&mut application, &mut background_events, prompt).await
        }
        Mode::Interactive | Mode::OpenaiLogin => {
            run_interactive(
                &mut application,
                &mut background_events,
                create_terminal_resources,
            )
            .await
        }
    }
}

async fn run_single_prompt(
    application: &mut Application,
    background_events: &mut Events,
    prompt: String,
) -> Result<(), ApplicationError> {
    chats::send_message(application, prompt);
    let result = events::run_single_prompt_loop(application, background_events).await;
    if let Some(request) = application.active_request.take() {
        request.abort();
    }
    let response = result?;
    let mut output = io::stdout().lock();
    output
        .write_all(response.as_bytes())
        .map_err(|error| ApplicationError::Output(format!("Failed to write response: {error}")))?;
    if !response.ends_with('\n') {
        writeln!(output).map_err(|error| {
            ApplicationError::Output(format!("Failed to write response: {error}"))
        })?;
    }
    Ok(())
}

async fn run_interactive<BackendType, EventSource>(
    application: &mut Application,
    background_events: &mut Events,
    create_terminal_resources: impl FnOnce() -> (BackendType, EventSource, TerminalMode),
) -> Result<(), ApplicationError>
where
    BackendType: Backend,
    BackendType::Error: Send + Sync + 'static,
    EventSource: Stream<Item = io::Result<Event>> + Unpin,
{
    let (backend, events, terminal_mode) = create_terminal_resources();
    let mut interface = build_interface(
        backend,
        events,
        terminal_mode,
        application.clipboard_command.clone(),
    )?;
    let result = async {
        if matches!(application.mode, Mode::OpenaiLogin) {
            let (sender, input) = mpsc::unbounded_channel();
            application.pending_authorization = Some(sender);
            let home_directory = application.home_directory.clone();
            let clipboard_command = application.clipboard_command.clone();
            let event_sender = application.event_sender.clone();
            let connection = application.session.provider.connection.clone();
            let authentication_address = application
                .session
                .provider
                .get_openai_authentication_address()
                .expect("OpenAI login selects the OpenAI provider")
                .to_owned();
            application.active_request = Some(tokio::spawn(async move {
                let result = providers::openai::perform_login(
                    &home_directory,
                    input,
                    &event_sender,
                    &connection,
                    &authentication_address,
                    clipboard_command.as_deref(),
                )
                .await;
                let event = match result {
                    Ok(()) => events::Event::LoginSucceeded,
                    Err(error) => events::Event::LoginFailed(error),
                };
                let _ = event_sender.send(event);
            }));
        }
        events::run_interactive_loop(application, &mut interface, background_events).await
    }
    .await;
    if let Some(request) = application.active_request.take() {
        request.abort();
    }
    let restoration = interface::restore_terminal(&mut interface);
    result.and(restoration)
}

pub(crate) fn handle_action<BackendType: Backend, EventSource>(
    application: &mut Application,
    interface: &mut Interface<BackendType, EventSource>,
    action: Option<interface::Action>,
) -> bool {
    match action {
        Some(interface::Action::Exit) => return true,
        Some(interface::Action::Abort) => chats::abort_response(application),
        Some(interface::Action::Submit) => {
            if application.pending_authorization.is_some() {
                if let Some(input) = interface::take_input(interface)
                    && let Some(sender) = &application.pending_authorization
                {
                    let _ = sender.send(input);
                }
            } else if !matches!(application.mode, Mode::OpenaiLogin)
                && application.response_started.is_none()
                && let Some(input) = interface::take_input(interface)
            {
                chats::send_message(application, input);
            }
        }
        Some(interface::Action::SwitchContext { new_session })
            if !matches!(application.mode, Mode::OpenaiLogin) =>
        {
            chats::abort_response(application);
            application.session.cycle_context(new_session);
            interface::reset_view(interface, new_session);
            application.error = application.session.save(false).err();
        }
        Some(interface::Action::SwitchModel) => {
            application.session.cycle_model();
            if application.response_started.is_none()
                && let Err(error) = application.session.save(false)
            {
                application.error = Some(error);
            }
        }
        _ => {}
    }
    false
}
