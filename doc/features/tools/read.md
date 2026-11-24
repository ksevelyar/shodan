# Read file

## Requirements

* User can ask Shodan to read a text file.
* User can ask Shodan to read part of a text file.
* User is informed when a file cannot be read.

## User scenarios

### 1. User reads a file

* A file named `greeting.txt` contains the text `Hello, Shodan!`.
* User asks Shodan to read `greeting.txt`.
* User sees the file reading request and its result, `Hello, Shodan!`, in the conversation.
* Model responds using the file contents.

### 2. User reads part of a file

* A file named `lines.txt` contains `First line`, `Second line` and `Third line` on separate lines.
* User asks Shodan to read the second line of `lines.txt`.
* User sees the file reading request and its result, `Second line`, in the conversation.
* Model responds using the requested line.

### 3. User reads a nonexistent file

* The file `missing.txt` does not exist in the session directory.
* User asks Shodan to read `missing.txt`.
* User sees the file reading request and its error in the conversation.
* User is informed that the file could not be read.
