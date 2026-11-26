# Chat

## Requirements

* User can chat with a configured model in a console session.
* Conversation history is retained during the session.
* User can scroll conversation history one line at a time with the mouse wheel anywhere in the terminal.
* Model responses stream as they are generated.
* User sees model thinking in grey as it streams.
* User sees tool calls and their results in the conversation.
* User can abort a request with Esc, stopping model processing and seeing `Request was aborted` in neon pink.
* User sees `Request failed` when a model request fails.
* Conversation messages appear without role labels.
* User messages have a dark grey background.
* Light grey lines separate the message input from the conversation and status panel.
* User can choose a system prompt when starting a session. The default prompt is short.
* User can end a session with an exit command, end of input, or Ctrl-C.

## User scenarios

### 1. User starts a session and scrolls through a long response

* User starts a new session.
* User asks for a proof-of-concept plan for a monochrome, 2D, low-poly Asteroids game in Bevy, with requirements and user scenarios.
* Model streams a long response. User sees `##` before the response finishes.
* User presses Page Up until the request is visible.
* User presses Page Down until the end of the response, `restart the game.`, is visible.

### 2. User scrolls conversation history with the mouse wheel

* User receives a long response.
* User scrolls up one line with the mouse wheel over the input.
* User scrolls down one line with the mouse wheel over the status panel and sees the end of the response again.

### 3. User composes a message

* User starts a session and types a message.
* User sees a light grey line above and below the message input.
* User sends the message and sees it with a dark grey background.

### 4. User aborts a model response

* User sends a message and sees part of the model response.
* User presses Esc.
* Model stops processing the request.
* User sees the partial response followed by `Request was aborted` in neon pink.
* User sends another message.
* Model receives the conversation without the aborted partial response and responds to the new message.

### 5. User aborts a tool call response

* User asks Shodan to list a directory.
* Model begins requesting a directory listing.
* User presses Esc and sees `Request was aborted`.
* User asks the model to say hello.
* Model receives the conversation and greets the user.

### 6. User sees a failed request

* User sends a message and sees part of the model response.
* The model connection ends before the response is complete.
* User sees `Request failed`.
* User sends another message.
* Model receives the conversation and responds to the new message.

### 7. User watches model thinking

* User asks Shodan to read `greeting.txt`.
* Model streams its thinking about reading the file.
* User sees the thinking in grey before the model finishes.
* Model reads the file and responds.

### 8. User exits with Ctrl-C

* User starts a session.
* User presses Ctrl-C.
* Application exits and restores the terminal.
