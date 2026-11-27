use std::{
    fs::{self, OpenOptions},
    path::Path,
    sync::atomic::AtomicBool,
};

use serde::Deserialize;

use super::{WriteDisplay, replace_file_atomically, run_file_mutation};

pub(super) fn describe() -> serde_json::Value {
    serde_json::json!({"type":"function","function":{"name":"write","description":"Create or overwrite a text file and create missing parent directories.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path"},"content":{"type":"string","description":"File contents"}},"required":["path","content"]}}})
}

#[derive(Deserialize)]
pub(super) struct WriteArguments {
    path: String,
    content: String,
}

pub(super) async fn write_file(arguments: WriteArguments) -> Result<WriteDisplay, String> {
    run_file_mutation(move |cancelled| write_contents(arguments, cancelled)).await
}

fn write_contents(
    arguments: WriteArguments,
    cancelled: &AtomicBool,
) -> Result<WriteDisplay, String> {
    let path = Path::new(&arguments.path);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let (destination, metadata) = match fs::canonicalize(path) {
        Ok(destination) => {
            OpenOptions::new()
                .write(true)
                .open(&destination)
                .map_err(|error| error.to_string())?;
            let metadata = fs::metadata(&destination).map_err(|error| error.to_string())?;
            (destination, Some(metadata))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if fs::symlink_metadata(path).is_ok() {
                return Err(error.to_string());
            }
            (path.to_owned(), None)
        }
        Err(error) => return Err(error.to_string()),
    };
    replace_file_atomically(
        &destination,
        &arguments.content,
        metadata.as_ref(),
        cancelled,
    )
    .map_err(|error| error.to_string())?;
    Ok(WriteDisplay {
        path: arguments.path,
        content: arguments.content,
    })
}
