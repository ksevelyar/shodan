# Response recording

## Requirements

* User can record responses from any provider using `--record`.
* OpenAI recordings protect credentials while preserving the other response fields.
* Recordings are saved under `recordings/` by provider and request path.
* Recording filenames use UUIDs.
* Event streams use the `.txt` extension.

## User scenarios

### 1. User records provider responses

* User starts Shodan with llama, `example-model` and `--record`.
* User sends a message.
* The provider streams a response to `v1/chat/completions`.
* Shodan saves the response under `recordings/llama/v1-chat-completions/` with a UUID filename and `.txt` extension.

### 2. User records an OpenAI response

* User starts Shodan with `--provider openai --record` and sends a message.
* OpenAI streams a response.
* Shodan saves the response under `recordings/openai/` by request path with a UUID filename and `.txt` extension.

### 3. User records an OpenAI credential refresh

* User starts Shodan with expired OpenAI credentials and `--record`.
* Shodan refreshes the credentials and receives a response.
* Shodan saves a sanitized authentication response with the other response fields preserved.

## Recording

Run Shodan with recording enabled:

```sh
shodan --provider llama --model MODEL --record
```

Omit `--provider` and `--model` to use the configured defaults. Configure the llama server address in `~/.shodan/settings.json`.
Copy recordings into `tests/features/fixtures/` when updating fixtures.
