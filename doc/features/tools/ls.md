# List directory

## Requirements

* User can ask Shodan to list a directory.
* Directory listing requests show `ls` followed by the directory path.
* Directory listings include hidden entries and distinguish directories from files.
* User is informed when a directory cannot be listed.

## User scenarios

### 1. User lists a directory

* A directory contains `notes.txt`, `.settings` and a directory named `src`.
* User asks Shodan to list that directory.
* User sees `ls` followed by the directory path and its result in the conversation.
* The result contains `notes.txt`, `.settings` and `src`, with `src` identified as a directory.
* Model responds using the listing.

### 2. User lists a nonexistent directory

* The path `missing-directory` does not exist in the session directory.
* User asks Shodan to list `missing-directory`.
* User sees the directory listing request and its error in the conversation.
* User is informed that the directory could not be listed.
