# Single prompt

## Requirements

* User can submit a prompt with `--prompt` and receive the final model response.
* The model can use tools before responding.
* User can select the provider and model for the prompt.
* User sees an actionable error when the prompt cannot be completed.
* User can record provider responses while submitting a prompt.
* User sees an error when recording cannot be completed.

## User scenarios

### 1. User submits a prompt

* `greeting.txt` contains `Hello, Shodan!`.
* User runs Shodan with `--prompt "Read greeting.txt and show its contents."`.
* The model reads the file and responds.
* User sees the final response containing `Hello, Shodan!`.
* Shodan exits successfully.

### 2. User selects a provider and model

* User runs Shodan with `--provider llama --model Qwen3.8-27B-UD-IQ4_XS --prompt "Say hello."`.
* The selected model receives the prompt.
* User sees the model's response.
* Shodan exits successfully.

### 3. User sees a prompt error

* User submits a prompt and the model connection fails.
* User sees an error explaining the failure.
* Shodan exits with an error.

### 4. User combines a prompt with login

* User runs `shodan login openai --prompt "hello"`.
* User sees an error explaining that login and a prompt cannot be combined.
* Shodan exits with an error.

### 5. User records a prompt

* User submits a prompt with `--record`.
* Shodan saves the provider's response.
* User sees the final response.
* Shodan exits successfully.

### 6. User sees a response recording error

* User submits a prompt with `--record`.
* Shodan cannot save the provider's response.
* User sees a recording error identifying the recording path.
* Shodan exits with an error.
