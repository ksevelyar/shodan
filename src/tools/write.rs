use std::path::Path;

use serde::Deserialize;

use super::WriteDisplay;

pub(super) fn describe() -> serde_json::Value {
    serde_json::json!({"type":"function","function":{"name":"write","description":"Create or overwrite a text file and create missing parent directories.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path"},"content":{"type":"string","description":"File contents"}},"required":["path","content"]}}})
}

#[derive(Deserialize)]
pub(super) struct WriteArguments {
    path: String,
    content: String,
}

pub(super) async fn write_file(arguments: WriteArguments) -> Result<WriteDisplay, String> {
    let path = Path::new(&arguments.path);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| error.to_string())?;
    tokio::fs::write(path, &arguments.content)
        .await
        .map_err(|error| error.to_string())?;
    Ok(WriteDisplay {
        path: arguments.path,
        content: arguments.content,
    })
}
