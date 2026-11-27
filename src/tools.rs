mod bash;
mod edit;
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

#[derive(Deserialize)]
struct CommandArguments {
    command: String,
}

#[derive(Deserialize)]
struct DirectoryArguments {
    path: String,
}

#[derive(Clone, PartialEq)]
pub enum ToolDisplay {
    Read(ReadDisplay),
    Edit(EditDisplay),
    Bash { successful: bool },
}

#[derive(Clone, PartialEq)]
pub struct EditDisplay {
    pub path: String,
    pub original: String,
    pub updated: String,
}

#[derive(Clone, PartialEq)]
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

pub(crate) fn describe_tools(read_context: bool) -> Vec<Value> {
    let mut tools = vec![
        json!({"type":"function","function":{"name":"edit","description":"Edit one existing file with exact, unique, non-overlapping replacements matched against the original file. Include all changes to the file in one call.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path"},"edits":{"type":"array","minItems":1,"items":{"type":"object","properties":{"oldText":{"type":"string","description":"Nonempty text to replace, unique in the original file"},"newText":{"type":"string","description":"Replacement text"}},"required":["oldText","newText"]}}},"required":["path","edits"]}}}),
        json!({"type":"function","function":{"name":"bash","description":"Run a bash command","parameters":{"type":"object","properties":{"command":{"type":"string","description":"Command to run"}},"required":["command"]}}}),
        json!({"type":"function","function":{"name":"ls","description":"list directory content","parameters":{"type":"object","properties":{"path":{"type":"string","description":"Directory path"}},"required":["path"]}}}),
        json!({"type":"function","function":{"name":"read","description":"Read a text file","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path"},"offset":{"type":"integer","description":"First line, starting at 1"},"limit":{"type":"integer","description":"Maximum number of lines"}},"required":["path"]}}}),
    ];
    if read_context {
        tools.retain(|tool| tool["function"]["name"] != "edit");
    }
    tools
}

pub(crate) async fn execute(name: &str, arguments: &str) -> (String, Option<ToolDisplay>) {
    if name == "edit" {
        let result = match serde_json::from_str::<edit::EditArguments>(arguments) {
            Ok(arguments) => edit::edit_file(arguments).await,
            Err(error) => Err(error.to_string()),
        };
        return match result {
            Ok(display) => {
                let diff = similar::TextDiff::from_lines(&display.original, &display.updated)
                    .unified_diff()
                    .header(&display.path, &display.path)
                    .to_string();
                (
                    format!("Successfully edited {}.\n{diff}", display.path),
                    Some(ToolDisplay::Edit(display)),
                )
            }
            Err(error) => (format!("Error: {error}"), None),
        };
    }
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
                    Some(ToolDisplay::Read(ReadDisplay {
                        path: arguments.path,
                        offset: arguments.offset.unwrap_or(1),
                        preceding_content,
                    })),
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
    if name == "bash" {
        let result = match serde_json::from_str::<CommandArguments>(arguments) {
            Ok(arguments) => bash::run_command(&arguments.command)
                .await
                .map_err(|error| format!("tool: Failed to run bash command: {error}")),
            Err(error) => Err(error.to_string()),
        };
        return match result {
            Ok((output, successful)) => (output, Some(ToolDisplay::Bash { successful })),
            Err(error) => (
                format!("Error: {error}"),
                Some(ToolDisplay::Bash { successful: false }),
            ),
        };
    }
    let result = match name {
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
