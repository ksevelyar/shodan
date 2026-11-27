use std::{io, path::Path};

use tokio::io::{AsyncBufReadExt, BufReader};

pub(super) fn describe() -> serde_json::Value {
    serde_json::json!({"type":"function","function":{"name":"read","description":"Read a text file","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path"},"offset":{"type":"integer","description":"First line, starting at 1"},"limit":{"type":"integer","description":"Maximum number of lines"}},"required":["path"]}}})
}

pub(super) async fn read_file(
    path: &Path,
    offset: Option<usize>,
    limit: Option<usize>,
) -> io::Result<String> {
    read_file_with_context(path, offset, limit)
        .await
        .map(|(content, _)| content)
}

pub(super) async fn read_file_with_context(
    path: &Path,
    offset: Option<usize>,
    limit: Option<usize>,
) -> io::Result<(String, String)> {
    let first_line = offset.unwrap_or(1);
    if first_line == 0 || limit == Some(0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "offset and limit must be greater than zero",
        ));
    }
    let file = tokio::fs::File::open(path).await?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut preceding_content = String::new();
    for _ in 1..first_line {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "offset is beyond the end of the file",
            ));
        }
        preceding_content.push_str(&line);
    }
    let mut content = String::new();
    for line_number in 0..limit.unwrap_or(usize::MAX) {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            if line_number == 0 && first_line > 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "offset is beyond the end of the file",
                ));
            }
            break;
        }
        content.push_str(&line);
    }
    Ok((content, preceding_content))
}
