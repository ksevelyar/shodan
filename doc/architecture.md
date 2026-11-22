# Architecture

## Libraries

* Ratatui for layout and drawing.
* Crossterm for terminal input and events.
* Tokio for asynchronous tasks and channels.
* Reqwest for streaming HTTP requests.
* Serde and serde_json for request and response data.

## Event flow

The interface loop owns application state and rendering. A Tokio task sends requests to the llama.cpp server and passes streamed response events to the interface loop through a channel. The interface loop updates conversation history and redraws the interface.
