# Find paths

## Requirements

* User can find paths matching a glob pattern within a directory.
* Results show paths relative to the searched directory and distinguish directories from files.
* Searches include hidden entries and respect `.gitignore`.
* User can limit the number of results.
* User is informed when results are truncated or a search fails.

## User scenarios

### 1. User finds files

* `src` contains `main.rs` and `nested/lib.rs`.
* User asks Shodan to find Rust files in `src`.
* User sees the search request and the paths `main.rs` and `nested/lib.rs`.
* Model responds using the results.

### 2. User finds hidden entries

* A directory contains `.settings` and an entry excluded by `.gitignore`.
* User asks Shodan to find hidden entries in the directory.
* User sees `.settings`, with ignored entries excluded from the search.

### 3. User finds a directory

* A directory contains a subdirectory named `assets`.
* User asks Shodan to find `assets`.
* User sees `assets` identified as a directory.

### 4. User limits results

* A directory contains more matching paths than the requested limit.
* User asks Shodan to find paths with that limit.
* User sees results up to the limit and is informed that results were truncated.

### 5. User finds no matching paths

* User asks Shodan to find paths matching a pattern with no matches.
* User is informed that no paths were found.

### 6. User searches an invalid directory

* User asks Shodan to find files in a nonexistent directory.
* User sees the search request and its error.
* User is informed that the search failed.
