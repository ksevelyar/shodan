use std::{io, path::Path};

pub(super) async fn list_directory(path: &Path) -> io::Result<String> {
    let mut directory = tokio::fs::read_dir(path).await?;
    let mut entries = Vec::new();
    while let Some(entry) = directory.next_entry().await? {
        let mut name = entry.file_name().to_string_lossy().into_owned();
        if entry.metadata().await?.is_dir() {
            name.push('/');
        }
        entries.push(name);
    }
    entries.sort();
    Ok(entries.join("\n"))
}
