# Providers and models

## Requirements

* The initial configuration selects llama as the default provider and Inkling-2-Mini-UD-IQ3_XXS as its default model.
* The initial llama model list contains Inkling-2-Mini-UD-IQ3_XXS with thinking level `low`.
* Each configured provider defines its available models, their cycling order, and its default model.
* User can select a provider and model with CLI arguments.
* A selected provider uses its configured default model unless the user selects a model.
* Models with a configured thinking level use that level for responses.
* User cycles through the selected provider's configured models with ctrl+space.
* User sees the selected model immediately when cycling models.
* User can send a message to an unloaded llama model when the router supports automatic loading.
* User sees an actionable error when the selected provider or model cannot be used.

## User scenarios

### 1. User starts with an existing configuration

* User has configured OpenAI as the default provider and removed llama from `~/.shodan/settings.json`.
* User starts Shodan.
* Shodan uses the configured OpenAI default model and preserves the provider configuration.

### 2. User selects a provider

* User has configured llama as the default provider and OpenAI with `gpt-6.1-terra` as its default model.
* User starts Shodan with `--provider openai`.
* User sees `openai gpt-6.1-terra high` in the status panel.

### 3. User selects a model

* User has configured OpenAI with `gpt-6.1-terra` and `gpt-6-luna`.
* User starts Shodan with `--provider openai --model gpt-6-luna`.
* User sees `openai gpt-6-luna max` in the status panel.

### 4. User chats with Inkling at its configured thinking level

* User starts Shodan with Inkling-2-Mini-UD-IQ3_XXS configured at thinking level `low`.
* User sends a message.
* The model receives the request with thinking level `low` and responds.

### 5. User cycles through configured models

* User configures Inkling-2-Mini-UD-IQ3_XXS followed by Qwen3.8-27B-UD-IQ4_XS for llama, with Inkling as the default.
* User starts Shodan and presses ctrl+space.
* User immediately sees Qwen in the status panel.
* User sends a message and receives a response from Qwen.
* User presses ctrl+space again and immediately sees Inkling in the status panel.

### 6. User selects an unavailable provider

* User starts Shodan with a provider that is not configured.
* User sees an error identifying the provider.

### 7. User selects an unavailable model

* User starts Shodan with a model that is not configured for the selected provider.
* User sees an error identifying the model.

### 8. User cycles through configured OpenAI models

* User configures `gpt-6.1-terra` followed by `gpt-6-luna`, with `gpt-6.1-terra` as the default.
* User starts Shodan with OpenAI selected and presses ctrl+space.
* User sees `openai gpt-6-luna max` in the status panel.
* User presses ctrl+space again and sees `openai gpt-6.1-terra high`.

