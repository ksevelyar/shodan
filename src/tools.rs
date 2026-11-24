mod ls;
mod read;

use std::{io, path::Path};

use serde::Deserialize;
use serde_json::{Value, json};

#[doc = "# Errors\nReturns an error if the directory cannot be listed."]
pub async fn list_directory(path: &Path) -> io::Result<String> {
    ls::list_directory(path).await
}

#[doc = "# Errors\nReturns an error if the file cannot be read or the line range is invalid."]
pub async fn read_file(
    path: &Path,
    offset: Option<usize>,
    limit: Option<usize>,
) -> io::Result<String> {
    read::read_file(path, offset, limit).await
}

#[derive(Deserialize)]
struct DirectoryArguments {
    path: String,
}

#[derive(Deserialize)]
struct FileArguments {
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

pub(crate) fn describe_tools() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"ls","description":"list directory content","parameters":{"type":"object","properties":{"path":{"type":"string","description":"Directory path"}},"required":["path"]}}}),
        json!({"type":"function","function":{"name":"read","description":"Read a text file","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path"},"offset":{"type":"integer","description":"First line, starting at 1"},"limit":{"type":"integer","description":"Maximum number of lines"}},"required":["path"]}}}),
    ]
}

pub(crate) async fn execute(name: &str, arguments: &str) -> String {
    let result = match name {
        "ls" => match serde_json::from_str::<DirectoryArguments>(arguments) {
            Ok(arguments) => list_directory(Path::new(&arguments.path))
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        },
        "read" => match serde_json::from_str::<FileArguments>(arguments) {
            Ok(arguments) => read_file(
                Path::new(&arguments.path),
                arguments.offset,
                arguments.limit,
            )
            .await
            .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        },
        _ => Err(format!("unknown tool: {name}")),
    };
    result.unwrap_or_else(|error| format!("Error: {error}"))
}
