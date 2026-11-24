# Response recording

## Requirements

* Tool scenarios can record responses from a local llama.cpp server.
* Recorded responses are saved by scenario for use in feature tests.
* The server address can be configured.

## User scenarios

### 1. User records tool responses

* User loads the recorded model on a local llama.cpp server.
* User runs the tool scenarios with recording enabled.
* Shodan saves each response under `tests/features/fixtures/llama-cpp/tools/` in its scenario directory.
* User runs the feature tests using the recorded responses.

## Recording

With the recorded model loaded on the local llama.cpp server, run:

```sh
SHODAN_RECORD_LLAMA=1 cargo test --test features features::tools:: -- --test-threads=1
```

Set `SHODAN_LLAMA_ADDRESS` to use a different server address.
