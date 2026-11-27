use std::path::Path;

use globset::{GlobBuilder, GlobMatcher};
use ignore::WalkBuilder;

pub(super) fn create_walker(path: &Path) -> ignore::Walk {
    let mut builder = WalkBuilder::new(path);
    builder
        .hidden(false)
        .require_git(false)
        .sort_by_file_path(std::cmp::Ord::cmp);
    builder.filter_entry(|entry| entry.file_name() != ".git");
    builder.build()
}

pub(super) struct PathPattern {
    matcher: GlobMatcher,
    match_file_name: bool,
}

pub(super) fn compile_glob(pattern: &str) -> Result<PathPattern, String> {
    let matcher = GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|error| error.to_string())?
        .compile_matcher();
    Ok(PathPattern {
        matcher,
        match_file_name: !pattern.contains('/'),
    })
}

pub(super) fn matches_glob(pattern: &PathPattern, relative: &Path) -> bool {
    if pattern.match_file_name {
        relative
            .file_name()
            .is_some_and(|name| pattern.matcher.is_match(name))
    } else {
        pattern.matcher.is_match(relative)
    }
}
