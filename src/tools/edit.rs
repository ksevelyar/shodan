use std::{
    fs::{self, OpenOptions},
    sync::atomic::AtomicBool,
};

use serde::Deserialize;

use super::{EditDisplay, replace_file_atomically, run_file_mutation};

pub(super) fn describe() -> serde_json::Value {
    serde_json::json!({"type":"function","function":{"name":"edit","description":"Edit one existing file with exact, unique, non-overlapping replacements matched against the original file. Include all changes to the file in one call.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path"},"edits":{"type":"array","minItems":1,"items":{"type":"object","properties":{"oldText":{"type":"string","description":"Nonempty text to replace, unique in the original file"},"newText":{"type":"string","description":"Replacement text"}},"required":["oldText","newText"]}}},"required":["path","edits"]}}})
}

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

pub(super) async fn edit_file(arguments: EditArguments) -> Result<EditDisplay, String> {
    run_file_mutation(move |cancelled| apply_edits(arguments, cancelled)).await
}

fn apply_edits(arguments: EditArguments, cancelled: &AtomicBool) -> Result<EditDisplay, String> {
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
        replace_file_atomically(&path, &updated, Some(&metadata), cancelled)
            .map_err(|error| format!("Could not write {}: {error}", arguments.path))?;
    }
    Ok(EditDisplay {
        path: arguments.path,
        original,
        updated,
    })
}
