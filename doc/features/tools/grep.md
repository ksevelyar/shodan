# Search file contents

## Requirements

* User can search a file or directory for text matching a regular expression or literal string.
* User can request a case-insensitive search and filter files by glob pattern.
* Search results show file paths, line numbers and matching lines.
* Directory searches include hidden files and respect `.gitignore`.
* Search results include up to 100 matches.
* User is informed when results are truncated or a search fails.

## User scenarios

### 1. User searches file contents

* `src/main.rs` contains `fn main()` on line 1.
* User asks Shodan to search `src` for `fn main`.
* User sees the search request and a result identifying `main.rs`, line 1 and `fn main()`.
* Model responds using the search result.

### 2. User searches with a file filter

* `src/main.rs` and `src/notes.txt` contain `hello`.
* User asks Shodan to search Rust files in `src` for `hello`.
* User sees the matching line from `main.rs`.
* Model responds using the search result.

### 3. User searches for literal text without regard to case

* `notes.txt` contains `Version 1.0`.
* User asks Shodan to search literally for `version 1.0`, ignoring case.
* User sees the matching line from `notes.txt`.

### 4. User searches a directory with ignored files

* A directory contains a hidden file `.notes` with `hello` and a file excluded by `.gitignore`.
* User asks Shodan to search the directory for `hello`.
* User sees the match from `.notes`, with ignored files excluded from the search.

### 5. User sees truncated search results

* A directory contains more than 100 matching lines.
* User asks Shodan to search the directory.
* User sees up to 100 matches and is informed that results were truncated.

### 6. User searches with no matches

* User asks Shodan to search a file for text that does not occur.
* User is informed that the search found no matches.

### 7. User searches an invalid path

* User asks Shodan to search a nonexistent path.
* User sees the search request and its error.
* User is informed that the search failed.
