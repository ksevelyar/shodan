# Context switching

## Requirements

* A session starts in the read context.
* User cycles through read, edit, review, and blank with Tab.
* The top panel shows the current context.
* Read provides all tools except edit. Edit, review, and blank provide all tools.
* Read, edit, and review use their respective prompts from `~/.shodan/read.md`, `~/.shodan/edit.md`, and `~/.shodan/review.md`.
* Shodan appends the project directory's `AGENTS.md` to each nonblank context prompt, separated by a newline.
* Missing or unreadable prompt files contribute no text.
* Blank has an empty system prompt.
* The current system prompt appears as the first conversation message before the user sends a message.
* Switching aborts active work and starts a conversation with the new prompt and the text of the last completed model response, if available.

## User scenarios

### 1. User starts a session with project instructions

* User has a read prompt and an `AGENTS.md` in the project directory.
* User starts Shodan and sees read in the top panel.
* User sees the read prompt followed by the project instructions in the conversation.
* User sends a message and the model receives the displayed prompt and all tools except edit.

### 2. User switches contexts

* User receives a model response in read context.
* User presses Tab and sees edit in the top panel.
* User sees the edit prompt and project instructions followed by the last model response.
* User sends a message and the model receives the edit prompt and all tools.
* User presses Tab and sees review, then presses Tab and sees blank.
* User sees an empty prompt and sends a message with all tools available.
* User presses Tab and sees read again.

### 3. User switches during a response

* User receives a model response and sends another message.
* User presses Tab while the next response is in progress.
* Shodan aborts the response and shows the new prompt followed by the previous completed model response.

### 4. User starts without prompt files

* User starts Shodan without a read prompt or project `AGENTS.md`.
* User sees read in the top panel and can send a message with an empty system prompt.
