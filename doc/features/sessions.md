# Sessions

## Requirements

* Shodan saves sessions under `~/.shodan/sessions/`, grouped by absolute working directory.
* Shodan mirrors the working directory path beneath `~/.shodan/sessions/`.
* Each session has a UUID and is saved as `<uuid>.jsonl`.
* Sending the first user message creates the session file.
* Shodan saves user messages, completed model messages, and tool results as they occur.
* Shodan saves context and model changes in an existing session.
* User can resume a session with `--session <number>`.
* Session `0` is the most recently updated session for the current working directory.
* Each higher number selects the next most recently updated session for that directory.
* Resuming restores the conversation, context, and model.
* User sees an actionable error when the selected session is unavailable.

## User scenarios

### 1. User resumes a session

* User starts Shodan in `~/projects/shodan` and sends a message.
* Model responds and user exits Shodan.
* User sees the session file at `~/.shodan/sessions/<home-path>/projects/shodan/<uuid>.jsonl`, where `<home-path>` is the absolute home path without its leading slash.
* User starts Shodan in `~/projects/shodan` with `--session 0`.
* User sees the previous conversation.
* User sends another message.
* Model receives the previous conversation and the new message.

### 2. User selects an older session

* User starts a session, sends a message, and exits.
* User starts another session, sends a different message, and exits.
* User starts Shodan with `--session 1`.
* User sees the first conversation.
* User sends another message and exits.
* User starts Shodan with `--session 0`.
* User sees the updated first conversation.

### 3. User resumes a project session with its context, model, and tool results

* User starts Shodan in `~/projects/shodan`.
* User changes context and model.
* User sends a message that leads to a tool call.
* Model receives the tool result and responds.
* User exits and sees the session at `~/.shodan/sessions/<home-path>/projects/shodan/<uuid>.jsonl`.
* User starts Shodan in `~/projects/other`, sends a message, and exits.
* User sees that session at `~/.shodan/sessions/<home-path>/projects/other/<uuid>.jsonl`.
* User starts Shodan in `~/projects/shodan` with `--session 0`.
* User sees the selected context and model, tool call, tool result, and model response.
* User sends another message.
* Model receives the previous conversation, including the tool call and result, with the selected context's system prompt.
* User starts Shodan in `~/projects/other` with `--session 0` and sees that project's conversation.

### 4. User selects an unavailable session

* User starts Shodan with a session number unavailable in the current directory.
* User sees an error identifying the session number.
