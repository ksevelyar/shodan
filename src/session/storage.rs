use std::{
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{chats::Message, session::Statistics};

#[derive(Serialize, Deserialize)]
pub(super) struct SessionSnapshot {
    pub context: String,
    pub model: String,
    pub provider: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub statistics: Statistics,
}

pub(super) fn get_directory(home_directory: &Path) -> io::Result<PathBuf> {
    let mut directory = home_directory.join(".shodan/sessions");
    for component in std::env::current_dir()?.components() {
        if let Component::Normal(part) = component {
            directory.push(part);
        }
    }
    Ok(directory)
}

pub(super) fn select_session(directory: &Path, number: usize) -> io::Result<PathBuf> {
    let mut sessions = match fs::read_dir(directory) {
        Ok(entries) => entries.collect::<Result<Vec<_>, _>>()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    sessions.retain(|entry| {
        entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "jsonl")
            && entry
                .path()
                .file_stem()
                .and_then(|name| name.to_str())
                .is_some_and(|name| uuid::Uuid::parse_str(name).is_ok())
            && entry.file_type().is_ok_and(|kind| kind.is_file())
    });
    let mut ordered = sessions
        .into_iter()
        .map(|entry| Ok((entry.metadata()?.modified()?, entry.path())))
        .collect::<io::Result<Vec<_>>>()?;
    ordered.sort_unstable_by(|left, right| right.cmp(left));
    ordered
        .into_iter()
        .nth(number)
        .map(|(_, path)| path)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("session {number} is unavailable in {}", directory.display()),
            )
        })
}

pub(super) fn read_session(path: &Path) -> io::Result<SessionSnapshot> {
    let contents = fs::read_to_string(path)?;
    let last = contents
        .lines()
        .last()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "session is empty"))?;
    serde_json::from_str(last).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn write_session(path: &Path, snapshot: &SessionSnapshot) -> io::Result<()> {
    let directory = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "session path has no directory")
    })?;
    fs::create_dir_all(directory)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, snapshot)?;
    writeln!(file)
}
