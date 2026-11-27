mod bash;
mod edit;
mod find;
mod grep;
mod ls;
mod read;
mod search;
mod write;

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::application::ApplicationError;

use serde::Deserialize;
use serde_json::Value;

#[doc = "# Errors\nReturns an error if the command cannot be spawned."]
pub async fn run_bash_command(command: &str) -> Result<String, ApplicationError> {
    bash::run_command(command)
        .await
        .map(|(output, _)| output)
        .map_err(|error| ApplicationError::Tool(format!("Failed to run bash command: {error}")))
}

#[doc = "# Errors\nReturns an error if the directory cannot be listed."]
pub async fn list_directory(path: &Path) -> Result<String, ApplicationError> {
    ls::list_directory(path).await.map_err(|error| {
        ApplicationError::Tool(format!(
            "Failed to list directory {}: {error}",
            path.display()
        ))
    })
}

#[doc = "# Errors\nReturns an error if the file cannot be read or the line range is invalid."]
pub async fn read_file(
    path: &Path,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<String, ApplicationError> {
    read::read_file(path, offset, limit).await.map_err(|error| {
        ApplicationError::Tool(format!("Failed to read file {}: {error}", path.display()))
    })
}

static MUTATION_LOCK: Mutex<()> = Mutex::new(());

struct Cancellation(Arc<AtomicBool>);

impl Drop for Cancellation {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

struct TemporaryFile(PathBuf);

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

async fn run_file_mutation<Output: Send + 'static>(
    operation: impl FnOnce(&AtomicBool) -> Result<Output, String> + Send + 'static,
) -> Result<Output, String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancellation = Cancellation(cancelled.clone());
    tokio::task::spawn_blocking(move || {
        let _guard = MUTATION_LOCK.lock().map_err(|error| error.to_string())?;
        if cancelled.load(Ordering::Relaxed) {
            return Err("Mutation aborted".to_owned());
        }
        operation(&cancelled)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn replace_file_atomically(
    path: &Path,
    content: &str,
    metadata: Option<&fs::Metadata>,
    cancelled: &AtomicBool,
) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("file has no parent directory"))?;
    let temporary_file = TemporaryFile(directory.join(format!(".shodan-{}", uuid::Uuid::new_v4())));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_file.0)?;
    if let Some(metadata) = metadata {
        file.set_permissions(metadata.permissions())?;
    }
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(io::Error::other("Mutation aborted"));
    }
    fs::rename(&temporary_file.0, path)?;
    Ok(())
}

#[derive(Deserialize)]
struct CommandArguments {
    command: String,
}

#[derive(Deserialize)]
struct DirectoryArguments {
    path: String,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ToolOutput {
    Text {
        content: String,
    },
    ReadText {
        path: String,
        content: String,
        offset: usize,
        preceding_content: String,
    },
    ReadImage {
        path: String,
        content: String,
        image_url: String,
    },
    Write {
        path: String,
        content: String,
        file_content: String,
    },
    Edit {
        path: String,
        content: String,
        original: String,
        updated: String,
    },
}

impl ToolOutput {
    pub(crate) fn content(&self) -> &str {
        match self {
            Self::Text { content }
            | Self::ReadText { content, .. }
            | Self::ReadImage { content, .. }
            | Self::Write { content, .. }
            | Self::Edit { content, .. } => content,
        }
    }
}

#[derive(Deserialize)]
struct FileArguments {
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

pub(crate) fn describe_tools(read_context: bool) -> Vec<Value> {
    let mut tools = vec![
        edit::describe(),
        bash::describe(),
        ls::describe(),
        read::describe(),
        grep::describe(),
        find::describe(),
        write::describe(),
    ];
    tools.retain(|tool| {
        is_available(
            tool["function"]["name"].as_str().unwrap_or_default(),
            read_context,
        )
    });
    tools
}

pub(crate) fn is_available(name: &str, read_context: bool) -> bool {
    !read_context || !matches!(name, "edit" | "write")
}

pub(crate) async fn execute(name: &str, arguments: &str) -> Result<ToolOutput, String> {
    if name == "bash" {
        let arguments: CommandArguments =
            parse_arguments(arguments).map_err(|error| format!("Error: {error}"))?;
        let (content, successful) = bash::run_command(&arguments.command)
            .await
            .map_err(|error| format!("Error: tool: Failed to run bash command: {error}"))?;
        return if successful {
            Ok(ToolOutput::Text { content })
        } else {
            Err(content)
        };
    }
    execute_call(name, arguments)
        .await
        .map_err(|error| format!("Error: {error}"))
}

async fn execute_call(name: &str, arguments: &str) -> Result<ToolOutput, String> {
    match name {
        "grep" => Ok(ToolOutput::Text {
            content: grep::search(parse_arguments(arguments)?).await?,
        }),
        "find" => Ok(ToolOutput::Text {
            content: find::find(parse_arguments(arguments)?).await?,
        }),
        "write" => {
            let (path, file_content) = write::write_file(parse_arguments(arguments)?).await?;
            Ok(ToolOutput::Write {
                content: format!("Successfully wrote {path}"),
                path,
                file_content,
            })
        }
        "edit" => {
            let (path, original, updated) = edit::edit_file(parse_arguments(arguments)?).await?;
            let diff = similar::TextDiff::from_lines(&original, &updated)
                .unified_diff()
                .header(&path, &path)
                .to_string();
            Ok(ToolOutput::Edit {
                content: format!("Successfully edited {path}.\n{diff}"),
                path,
                original,
                updated,
            })
        }
        "read" => {
            let arguments: FileArguments = parse_arguments(arguments)?;
            let path = Path::new(&arguments.path);
            let image = read::read_image(path).await.map_err(|error| {
                format!("tool: Failed to read file {}: {error}", arguments.path)
            })?;
            if let Some((content, image_url)) = image {
                Ok(ToolOutput::ReadImage {
                    path: arguments.path,
                    content,
                    image_url,
                })
            } else {
                let (content, preceding_content) =
                    read::read_file_with_context(path, arguments.offset, arguments.limit)
                        .await
                        .map_err(|error| {
                            format!("tool: Failed to read file {}: {error}", arguments.path)
                        })?;
                Ok(ToolOutput::ReadText {
                    path: arguments.path,
                    content,
                    offset: arguments.offset.unwrap_or(1),
                    preceding_content,
                })
            }
        }
        "ls" => {
            let arguments: DirectoryArguments = parse_arguments(arguments)?;
            let content = list_directory(Path::new(&arguments.path))
                .await
                .map_err(|error| error.to_string())?;
            Ok(ToolOutput::Text { content })
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}

fn parse_arguments<Arguments: serde::de::DeserializeOwned>(
    arguments: &str,
) -> Result<Arguments, String> {
    serde_json::from_str(arguments).map_err(|error| error.to_string())
}
