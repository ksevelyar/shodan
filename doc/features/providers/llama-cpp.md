# llama.cpp provider

## Requirements

* User can chat with a local llama.cpp server in router mode.
* Shodan selects the first loaded model at startup. If no model is loaded, Shodan loads and selects the first available model.
* User sees the selected model name in the status panel before sending a message.

## User scenarios

### 1. User starts a session with two loaded models

* Router has LFM3-Omni-2B and MiniCPM5-2B loaded, in that order.
* User starts Shodan.
* Shodan sends `GET /v1/models`. Router responds with `200` and `tests/features/fixtures/llama-cpp/models/both-loaded.json`.
* User sees `LFM3-Omni-2B-Q4_K_M` on the right of the status panel.

### 2. User starts a session with no loaded models

* Router has LFM3-Omni-2B and MiniCPM5-2B available, in that order. Both are unloaded.
* User starts Shodan.
* Shodan sends `GET /v1/models`. Router responds with `200` and `tests/features/fixtures/llama-cpp/models/both-unloaded.json`.
* Shodan sends `POST /models/load` with `{"model":"LFM3-Omni-2B-Q4_K_M"}`. Router responds with `200` and `{"success":true}`.
* Shodan sends `GET /v1/models`. Router responds with `200` and `tests/features/fixtures/llama-cpp/models/lfm3-loading.json`.
* Shodan sends `GET /v1/models`. Router responds with `200` and `tests/features/fixtures/llama-cpp/models/lfm3-loaded.json`.
* User sees `LFM3-Omni-2B-Q4_K_M` on the right of the status panel.

### 3. User starts a session with the second model loaded

* Router has LFM3-Omni-2B and MiniCPM5-2B available, in that order. LFM3-Omni-2B is unloaded and MiniCPM5-2B is loaded.
* User starts Shodan.
* Shodan sends `GET /v1/models`. Router responds with `200` and `tests/features/fixtures/llama-cpp/models/minicpm-loaded.json`.
* User sees `MiniCPM5-2B-Q4_K_M` on the right of the status panel.
