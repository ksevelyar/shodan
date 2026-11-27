use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use globset::{Glob, GlobMatcher};
use ignore::WalkBuilder;

pub(super) struct Cancellation(pub Arc<AtomicBool>);

impl Drop for Cancellation {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

pub(super) fn create_walker(path: &Path) -> ignore::Walk {
    let mut builder = WalkBuilder::new(path);
    builder
        .hidden(false)
        .require_git(false)
        .sort_by_file_path(std::cmp::Ord::cmp);
    builder.filter_entry(|entry| entry.file_name() != ".git");
    builder.build()
}

pub(super) fn compile_glob(pattern: &str) -> Result<GlobMatcher, String> {
    Glob::new(pattern)
        .map(|glob| glob.compile_matcher())
        .map_err(|error| error.to_string())
}

pub(super) fn matches_glob(matcher: &GlobMatcher, relative: &Path) -> bool {
    matcher.is_match(relative)
        || relative
            .file_name()
            .is_some_and(|name| matcher.is_match(name))
}
