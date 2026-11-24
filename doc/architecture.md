# Architecture

## Libraries

* Ratatui for layout and drawing.
* Crossterm for terminal input and events.
* Tokio for asynchronous tasks and channels.
* Reqwest for streaming HTTP requests.
* Serde and serde_json for request and response data.

## Event flow

The event loop owns application state and rendering. Background tasks send model and tool events to the event loop through a channel. The event loop updates conversation history, starts tool execution and model requests, and redraws the interface. Turn identifiers distinguish current events from events queued before cancellation.
