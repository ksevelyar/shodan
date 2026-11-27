use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use regex::RegexBuilder;
use serde::Deserialize;

use super::search::{Cancellation, compile_glob, create_walker, matches_glob};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SearchArguments {
    pattern: String,
    path: Option<PathBuf>,
    glob: Option<String>,
    ignore_case: Option<bool>,
    literal: Option<bool>,
}

pub(super) fn describe() -> serde_json::Value {
    serde_json::json!({"type":"function","function":{"name":"grep","description":"Search file contents for a pattern. Returns up to 100 matching lines with paths and line numbers. Respects .gitignore.","parameters":{"type":"object","properties":{"pattern":{"type":"string","description":"Regular expression or literal text"},"path":{"type":"string","description":"File or directory to search, defaults to current directory"},"glob":{"type":"string","description":"Glob pattern to filter files"},"ignoreCase":{"type":"boolean","description":"Ignore case"},"literal":{"type":"boolean","description":"Search for literal text"}},"required":["pattern"]}}})
}

pub(super) async fn search(arguments: SearchArguments) -> Result<String, String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancellation = Cancellation(cancelled.clone());
    tokio::task::spawn_blocking(move || search_files(arguments, &cancelled))
        .await
        .map_err(|error| error.to_string())?
}

fn search_files(arguments: SearchArguments, cancelled: &AtomicBool) -> Result<String, String> {
    let path = arguments.path.unwrap_or_else(|| PathBuf::from("."));
    let metadata =
        std::fs::metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let pattern = if arguments.literal.unwrap_or(false) {
        regex::escape(&arguments.pattern)
    } else {
        arguments.pattern
    };
    let expression = RegexBuilder::new(&pattern)
        .case_insensitive(arguments.ignore_case.unwrap_or(false))
        .build()
        .map_err(|error| error.to_string())?;
    let filter = arguments.glob.as_deref().map(compile_glob).transpose()?;
    let mut output = String::new();
    let mut matches = 0;
    let match_limit = 100;
    let output_limit = 50 * 1024;
    for entry in create_walker(&path) {
        if cancelled.load(Ordering::Relaxed) {
            return Err("Search aborted".to_owned());
        }
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let relative = if metadata.is_file() {
            entry.path().file_name().map_or(entry.path(), Path::new)
        } else {
            entry.path().strip_prefix(&path).unwrap_or(entry.path())
        };
        if filter
            .as_ref()
            .is_some_and(|matcher| !matches_glob(matcher, relative))
        {
            continue;
        }
        let file = std::fs::File::open(entry.path())
            .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        for (number, line) in BufReader::new(file).lines().enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Search aborted".to_owned());
            }
            let line = match line {
                Ok(line) => line,
                Err(error) if error.kind() == std::io::ErrorKind::InvalidData => break,
                Err(error) => return Err(error.to_string()),
            };
            if line.contains('\0') {
                break;
            }
            if expression.is_match(&line) {
                let formatted = format!("{}:{}:{}\n", relative.display(), number + 1, line);
                if matches == match_limit || output.len() + formatted.len() > output_limit {
                    output.push_str("[results truncated]");
                    return Ok(output);
                }
                output.push_str(&formatted);
                matches += 1;
            }
        }
    }
    if output.is_empty() {
        Ok("No matches found".to_owned())
    } else {
        Ok(output.trim_end().to_owned())
    }
}
