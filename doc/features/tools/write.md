# Write file

## Requirements

* User can ask Shodan to create or replace a text file.
* Writing a file creates missing parent directories.
* Writing requests show `write` followed by the file path.
* Writing requests and results share a continuous purple background across the conversation width.
* Written content shows line numbers and syntax highlighting for code.
* User is informed when a file cannot be written.

## User scenarios

### 1. User creates a file

* User asks Shodan to write `Hello, Shodan!` to `notes/greeting.txt`.
* User sees the writing request and its result.
* `notes/greeting.txt` contains `Hello, Shodan!`.
* Model reports the created file.

### 2. User replaces a file

* `greeting.txt` contains `Hello, world!`.
* User asks Shodan to replace its contents with `Hello, Shodan!`.
* User sees the writing request and its result.
* `greeting.txt` contains `Hello, Shodan!`.
* Model reports the change.

### 3. User writes to an invalid path

* A directory named `greeting.txt` exists.
* User asks Shodan to write a file at `greeting.txt`.
* User sees the writing request and its error.
* User is informed that the file could not be written.

### 4. User writes a source file

* User asks Shodan to write a Rust source file.
* User sees `write` followed by the file path and the written content on a continuous purple background across the conversation width.
* The content shows line numbers and Rust syntax highlighting.
* The file contains the requested content.
