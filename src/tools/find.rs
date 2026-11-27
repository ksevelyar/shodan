use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::Deserialize;

use super::{
    Cancellation,
    search::{compile_glob, create_walker, matches_glob},
};

#[derive(Deserialize)]
pub(super) struct FindArguments {
    pattern: String,
    path: Option<PathBuf>,
    limit: Option<usize>,
}

pub(super) fn describe() -> serde_json::Value {
    serde_json::json!({"type":"function","function":{"name":"find","description":"Find files and directories matching a glob pattern. Respects .gitignore.","parameters":{"type":"object","properties":{"pattern":{"type":"string","description":"Glob pattern"},"path":{"type":"string","description":"Directory to search, defaults to current directory"},"limit":{"type":"integer","description":"Maximum number of results"}},"required":["pattern"]}}})
}

pub(super) async fn find(arguments: FindArguments) -> Result<String, String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancellation = Cancellation(cancelled.clone());
    tokio::task::spawn_blocking(move || find_paths(arguments, &cancelled))
        .await
        .map_err(|error| error.to_string())?
}

fn find_paths(arguments: FindArguments, cancelled: &AtomicBool) -> Result<String, String> {
    let path = arguments.path.unwrap_or_else(|| PathBuf::from("."));
    let metadata =
        std::fs::metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }
    let matcher = compile_glob(&arguments.pattern)?;
    let limit = arguments.limit.unwrap_or(1000);
    if limit == 0 {
        return Err("limit must be greater than zero".to_owned());
    }
    let output_limit = 50 * 1024;
    let mut output = String::new();
    let mut result_count = 0;
    for entry in create_walker(&path) {
        if cancelled.load(Ordering::Relaxed) {
            return Err("Search aborted".to_owned());
        }
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.depth() == 0 {
            continue;
        }
        let relative = entry.path().strip_prefix(&path).unwrap_or(entry.path());
        if !matches_glob(&matcher, relative) {
            continue;
        }
        let suffix = if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
        {
            "/"
        } else {
            ""
        };
        let formatted = format!("{}{suffix}\n", relative.display());
        if result_count == limit || output.len() + formatted.len() > output_limit {
            output.push_str("[results truncated]");
            return Ok(output);
        }
        output.push_str(&formatted);
        result_count += 1;
    }
    if output.is_empty() {
        Ok("No paths found".to_owned())
    } else {
        Ok(output.trim_end_matches('\n').to_owned())
    }
}
