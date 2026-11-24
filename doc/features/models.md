# Models

## Requirements

* Shodan selects the first available model from its ordered model definitions.
* If no defined model is available, Shodan selects the first loaded model.
* If no defined model is available and no model is loaded, Shodan loads and selects the first available model.
* Qwen3.8-27B-UD-IQ4_XS uses the low thinking level by default.
* Models with a defined thinking level use that level for responses.

## User scenarios

### 1. User starts a session with the preferred model unloaded

* Router has Qwen3.8-27B-UD-IQ4_XS available and another model loaded.
* User starts Shodan.
* Shodan loads and selects Qwen3.8-27B-UD-IQ4_XS.
* User sees `Qwen3.8-27B-UD-IQ4_XS` in the status panel.

### 2. User chats with Qwen at its default thinking level

* Router has Qwen3.8-27B-UD-IQ4_XS loaded.
* User starts Shodan and sends a message.
* Shodan requests a response with `reasoning_effort` set to `low`.
* User receives the model response.
