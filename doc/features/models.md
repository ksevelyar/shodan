# Models

## Requirements

* Shodan selects the first available model from its ordered model definitions.
* If no defined model is available, Shodan selects the first loaded model.
* If no defined model is available and no model is loaded, Shodan loads and selects the first available model.
* Inkling-2-Mini-UD-IQ3_XXS is the first model definition.
* Qwen3.8-27B-UD-IQ4_XS uses the low thinking level by default.
* Inkling-2-Mini-UD-IQ3_XXS uses the low thinking level by default.
* Models with a defined thinking level use that level for responses.
* The user cycles to the next model definition with ctrl+space.
* Cycling selects the next model definition and loads it if it is unloaded.

## User scenarios

### 1. User starts a session with the preferred model unloaded

* Router has Inkling-2-Mini-UD-IQ3_XXS unloaded and another model loaded.
* User starts Shodan.
* Shodan loads and selects Inkling-2-Mini-UD-IQ3_XXS.
* User sees `Inkling-2-Mini-UD-IQ3_XXS` in the status panel.

### 2. User chats with Inkling at its default thinking level

* Router has Inkling-2-Mini-UD-IQ3_XXS loaded.
* User starts Shodan and sends a message.
* Shodan requests a response with `reasoning_effort` set to `low`.
* User receives the model response.

### 3. User cycles to Qwen with ctrl+space

* Router has Inkling-2-Mini-UD-IQ3_XXS loaded and Qwen3.8-27B-UD-IQ4_XS unloaded.
* User starts Shodan and presses ctrl+space.
* Shodan loads and selects Qwen3.8-27B-UD-IQ4_XS.
* User sees `Qwen3.8-27B-UD-IQ4_XS` in the status panel.

### 4. User chats with Qwen after cycling

* Router has Inkling-2-Mini-UD-IQ3_XXS loaded and Qwen3.8-27B-UD-IQ4_XS unloaded.
* User starts Shodan, presses ctrl+space, and sends a message.
* Shodan requests a response from Qwen3.8-27B-UD-IQ4_XS with `reasoning_effort` set to `low`.
* User receives the model response.

### 5. User cycles around back to Inkling

* Router has Inkling-2-Mini-UD-IQ3_XXS loaded and Qwen3.8-27B-UD-IQ4_XS unloaded.
* User starts Shodan and presses ctrl+space twice.
* Shodan cycles through Qwen3.8-27B-UD-IQ4_XS back to Inkling-2-Mini-UD-IQ3_XXS.
* User sees `Inkling-2-Mini-UD-IQ3_XXS` in the status panel.
