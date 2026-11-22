# Chat

## Requirements

* User can chat with a Llama model in a console session.
* Conversation history is retained during the session.
* Model responses stream as they are generated.
* User can choose a system prompt when starting a session. The default prompt is short.
* User can end a session with an exit command, end of input, or Ctrl-C.

## User scenarios

### 1. User starts a session and scrolls through a long response

* User starts a new session.
* User enters "say hi in spanish" in the textarea.
* Model responds "hola".
* User enters "explain what Feynman meant when he said no one truly understands quantum mechanics".
* Model streams a long response. User sees the first part before the response finishes.
* User scrolls back to the earlier exchange and forward to the end of the response.

### 2. User exits with Ctrl-C

* User starts a session.
* User presses Ctrl-C.
* Application exits and restores the terminal.
