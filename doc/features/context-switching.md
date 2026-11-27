# Context switching

## Requirements

* A session starts in the read context.
* User cycles through read, edit, review, and blank with Tab.
* The top panel shows the current context.
* Read provides all tools except edit and write. Edit, review, and blank provide all tools.
* Read, edit, and review use their respective prompts from `~/.shodan/read.md`, `~/.shodan/edit.md`, and `~/.shodan/review.md`.
* Shodan appends the project directory's `AGENTS.md` to each nonblank context prompt, separated by a newline.
* On startup, Shodan creates missing read, edit, and review prompt files as empty files and preserves existing prompt files.
* Prompt files that cannot be created or read contribute no text.
* Blank has an empty system prompt.
* The current system prompt appears as the first conversation message before the user sends a message.
* Tab aborts active work, changes context, and preserves the current conversation and session.
* After Tab, the model receives the preserved conversation with the new context's system prompt.
* Ctrl+Tab aborts active work, changes context, and starts a new session with the new system prompt and the last completed model response, if available.
* The new session is saved when the user sends its first message.

## User scenarios

### 1. User starts a session with project instructions

* User has a read prompt and an `AGENTS.md` in the project directory.
* User has existing edit and review prompts.
* User starts Shodan and sees read in the top panel.
* User sees the read prompt followed by the project instructions in the conversation.
* User sees the existing read, edit, and review prompts unchanged.
* User sends a message and the model receives the displayed prompt and all tools except edit and write.

### 2. User switches contexts and keeps the conversation

* User sends a message and receives a model response in read context.
* User presses Tab and sees edit in the top panel.
* User sees the edit prompt and project instructions followed by the existing conversation.
* User sends another message.
* Model receives the edit prompt, the existing conversation, the new message, and all tools.
* User presses Tab and sees review, then presses Tab and sees blank.
* User sees an empty system prompt and the existing conversation.
* User presses Tab and sees read again.

### 3. User switches context during a response

* User receives a model response and sends another message.
* User presses Tab while the next response is in progress.
* Shodan aborts the response and shows the new context with the existing conversation.
* User sends another message.
* Model receives the existing conversation with the new context's system prompt.

### 4. User starts a new session with the last model response

* User sends a message and receives a model response in read context.
* User presses Ctrl+Tab and sees edit in the top panel.
* User sees the edit prompt and project instructions followed by the last model response.
* User sends a message.
* Model receives the edit prompt, the carried response, and the new message.
* User exits and resumes the new session.
* User sees the carried response and the new conversation.

### 5. User starts without prompt files

* User starts Shodan without context prompts or project `AGENTS.md`.
* User sees empty read, edit, and review prompt files in `~/.shodan`.
* User sees read in the top panel and can send a message with an empty system prompt.

### 6. User starts with an unavailable prompt file

* User starts Shodan while the read prompt file cannot be created or read.
* User sends a message with an empty system prompt.
