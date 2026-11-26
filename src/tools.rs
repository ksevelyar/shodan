mod bash;
mod ls;
mod read;

use std::path::Path;

use crate::application::ApplicationError;

use serde::Deserialize;
use serde_json::{Value, json};

#[doc = "# Errors\nReturns an error if the command cannot be spawned."]
pub async fn run_bash_command(command: &str) -> Result<String, ApplicationError> {
    bash::run_command(command)
        .await
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

#[derive(Deserialize)]
struct CommandArguments {
    command: String,
}

#[derive(Deserialize)]
struct DirectoryArguments {
    path: String,
}

#[derive(Clone)]
pub struct ReadDisplay {
    pub path: String,
    pub offset: usize,
    pub preceding_content: String,
}

#[derive(Deserialize)]
struct FileArguments {
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

pub(crate) fn describe_tools() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"bash","description":"Run a bash command","parameters":{"type":"object","properties":{"command":{"type":"string","description":"Command to run"}},"required":["command"]}}}),
        json!({"type":"function","function":{"name":"ls","description":"list directory content","parameters":{"type":"object","properties":{"path":{"type":"string","description":"Directory path"}},"required":["path"]}}}),
        json!({"type":"function","function":{"name":"read","description":"Read a text file","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path"},"offset":{"type":"integer","description":"First line, starting at 1"},"limit":{"type":"integer","description":"Maximum number of lines"}},"required":["path"]}}}),
    ]
}

pub(crate) async fn execute(name: &str, arguments: &str) -> (String, Option<ReadDisplay>) {
    if name == "read" {
        return match serde_json::from_str::<FileArguments>(arguments) {
            Ok(arguments) => match read::read_file_with_context(
                Path::new(&arguments.path),
                arguments.offset,
                arguments.limit,
            )
            .await
            {
                Ok((content, preceding_content)) => (
                    content,
                    Some(ReadDisplay {
                        path: arguments.path,
                        offset: arguments.offset.unwrap_or(1),
                        preceding_content,
                    }),
                ),
                Err(error) => (
                    format!(
                        "Error: tool: Failed to read file {}: {error}",
                        arguments.path
                    ),
                    None,
                ),
            },
            Err(error) => (format!("Error: {error}"), None),
        };
    }
    let result = match name {
        "bash" => match serde_json::from_str::<CommandArguments>(arguments) {
            Ok(arguments) => run_bash_command(&arguments.command)
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        },
        "ls" => match serde_json::from_str::<DirectoryArguments>(arguments) {
            Ok(arguments) => list_directory(Path::new(&arguments.path))
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        },
        _ => Err(format!("unknown tool: {name}")),
    };
    (
        result.unwrap_or_else(|error| format!("Error: {error}")),
        None,
    )
}
