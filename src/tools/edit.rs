use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::Deserialize;

use super::EditDisplay;

#[derive(Deserialize)]
pub(super) struct EditArguments {
    pub path: String,
    pub edits: Vec<Replacement>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Replacement {
    old_text: String,
    new_text: String,
}

struct TemporaryFile(PathBuf);

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(super) async fn edit_file(arguments: EditArguments) -> Result<EditDisplay, String> {
    tokio::task::spawn_blocking(move || apply_edits(arguments))
        .await
        .map_err(|error| format!("Edit task failed: {error}"))?
}

fn apply_edits(arguments: EditArguments) -> Result<EditDisplay, String> {
    static MUTATION_LOCK: Mutex<()> = Mutex::new(());
    let _guard = MUTATION_LOCK.lock().map_err(|error| error.to_string())?;
    if arguments.edits.is_empty() {
        return Err("edits must contain at least one replacement".to_owned());
    }
    let path = fs::canonicalize(&arguments.path)
        .map_err(|error| format!("Could not edit {}: {error}", arguments.path))?;
    let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err(format!("{} is not a file", arguments.path));
    }
    let original = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", arguments.path))?;
    let mut regions = Vec::new();
    for (index, replacement) in arguments.edits.iter().enumerate() {
        if replacement.old_text.is_empty() {
            return Err(format!("edits[{index}].oldText must not be empty"));
        }
        let mut matches = original.char_indices().filter_map(|(position, _)| {
            original[position..]
                .starts_with(&replacement.old_text)
                .then_some(position)
        });
        let start = matches.next().ok_or_else(|| {
            format!(
                "Could not find edits[{index}].oldText in {}",
                arguments.path
            )
        })?;
        if matches.next().is_some() {
            return Err(format!(
                "edits[{index}].oldText is ambiguous in {}",
                arguments.path
            ));
        }
        regions.push((
            start,
            start + replacement.old_text.len(),
            &replacement.new_text,
        ));
    }
    regions.sort_by_key(|region| region.0);
    if regions
        .windows(2)
        .any(|regions| regions[0].1 > regions[1].0)
    {
        return Err("Replacement regions overlap".to_owned());
    }
    let mut updated = String::new();
    let mut position = 0;
    for (start, end, replacement) in regions {
        updated.push_str(&original[position..start]);
        updated.push_str(replacement);
        position = end;
    }
    updated.push_str(&original[position..]);
    if original != updated {
        OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|error| format!("Could not write {}: {error}", arguments.path))?;
        write_replacement(&path, &updated, &metadata)
            .map_err(|error| format!("Could not write {}: {error}", arguments.path))?;
    }
    Ok(EditDisplay {
        path: arguments.path,
        original,
        updated,
    })
}

fn write_replacement(path: &Path, content: &str, metadata: &fs::Metadata) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("file has no parent directory"))?;
    let temporary_path = directory.join(format!(".shodan-{}", uuid::Uuid::new_v4()));
    let temporary_file = TemporaryFile(temporary_path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_file.0)?;
    file.set_permissions(metadata.permissions())?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    fs::rename(&temporary_file.0, path)?;
    Ok(())
}
