mod bash;
mod edit;
mod find;
mod grep;
mod ls;
mod read;
mod search;
mod write;

use std::path::Path;

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

#[derive(Deserialize)]
struct CommandArguments {
    command: String,
}

#[derive(Deserialize)]
struct DirectoryArguments {
    path: String,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ToolDisplay {
    Read(ReadDisplay),
    Write(WriteDisplay),
    Edit(EditDisplay),
    Bash { successful: bool },
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EditDisplay {
    pub path: String,
    pub original: String,
    pub updated: String,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReadDisplay {
    pub path: String,
    pub offset: usize,
    pub preceding_content: String,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WriteDisplay {
    pub path: String,
    pub content: String,
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

pub(crate) async fn execute(name: &str, arguments: &str) -> (String, Option<ToolDisplay>) {
    match execute_call(name, arguments).await {
        Ok(result) => result,
        Err(error) => (
            format!("Error: {error}"),
            (name == "bash").then_some(ToolDisplay::Bash { successful: false }),
        ),
    }
}

async fn execute_call(
    name: &str,
    arguments: &str,
) -> Result<(String, Option<ToolDisplay>), String> {
    match name {
        "grep" => Ok((grep::search(parse_arguments(arguments)?).await?, None)),
        "find" => Ok((find::find(parse_arguments(arguments)?).await?, None)),
        "write" => {
            let display = write::write_file(parse_arguments(arguments)?).await?;
            Ok((
                format!("Successfully wrote {}", display.path),
                Some(ToolDisplay::Write(display)),
            ))
        }
        "edit" => {
            let display = edit::edit_file(parse_arguments(arguments)?).await?;
            let diff = similar::TextDiff::from_lines(&display.original, &display.updated)
                .unified_diff()
                .header(&display.path, &display.path)
                .to_string();
            Ok((
                format!("Successfully edited {}.\n{diff}", display.path),
                Some(ToolDisplay::Edit(display)),
            ))
        }
        "read" => {
            let arguments: FileArguments = parse_arguments(arguments)?;
            let (content, preceding_content) = read::read_file_with_context(
                Path::new(&arguments.path),
                arguments.offset,
                arguments.limit,
            )
            .await
            .map_err(|error| format!("tool: Failed to read file {}: {error}", arguments.path))?;
            Ok((
                content,
                Some(ToolDisplay::Read(ReadDisplay {
                    path: arguments.path,
                    offset: arguments.offset.unwrap_or(1),
                    preceding_content,
                })),
            ))
        }
        "bash" => {
            let arguments: CommandArguments = parse_arguments(arguments)?;
            let (output, successful) = bash::run_command(&arguments.command)
                .await
                .map_err(|error| format!("tool: Failed to run bash command: {error}"))?;
            Ok((output, Some(ToolDisplay::Bash { successful })))
        }
        "ls" => {
            let arguments: DirectoryArguments = parse_arguments(arguments)?;
            let output = list_directory(Path::new(&arguments.path))
                .await
                .map_err(|error| error.to_string())?;
            Ok((output, None))
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}

fn parse_arguments<Arguments: serde::de::DeserializeOwned>(
    arguments: &str,
) -> Result<Arguments, String> {
    serde_json::from_str(arguments).map_err(|error| error.to_string())
}
