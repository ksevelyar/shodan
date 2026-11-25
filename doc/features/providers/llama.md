# llama provider

## Requirements

* User can chat with a local llama server in router mode.
* User sees the selected model name in the status panel before sending a message.
* User can configure the llama server address.
* User can select an unloaded model and send it a message when the router supports automatic loading.
* User sees an actionable error when the selected model cannot be used.

## User scenarios

### 1. User connects to a configured server

* User configures the llama server address in `~/.shodan/settings.json`.
* User starts Shodan and sends a message.
* User receives a response from the configured server.

