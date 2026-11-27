# Benchmarks

## 1. User resumes a session

* User asks the OpenAI model for a technical-debt plan.
* Model reads project files and responds.
* User exits and resumes the session.
* User sees the restored conversation.

`session_restore/cold_process` measures settings construction through the first 120 by 24 test-terminal frame in a fresh process. The session uses recorded responses and files from the current checkout. Fixture preparation, runtime creation and process creation are outside the measurement.

```sh
cargo bench --bench conversation -- session_restore/cold_process
```

## 2. User scrolls a conversation after twenty messages

* User sends twenty messages to the Llama model.
* Model responds to each message.
* User scrolls the conversation.

```sh
cargo bench --bench conversation -- conversation_scroll/chat/20
```

## 3. User scrolls a conversation with file reads

* User sends five messages to the Llama model.
* Model reads a file and responds to each message.
* User scrolls the conversation.

```sh
cargo bench --bench conversation -- conversation_scroll/read/5
```

## 4. User scrolls a technical-debt conversation

* User asks the OpenAI model for a technical-debt plan.
* Model reads project files and responds.
* User scrolls the conversation.

```sh
cargo bench --bench conversation -- conversation_scroll/technical_debt/1
```

## 5. User scrolls while a response is pending

* User asks the OpenAI model for a technical-debt plan.
* Model reads five project files and receives their contents.
* User scrolls the conversation while the next response is pending.

```sh
cargo bench --bench conversation -- conversation_scroll/technical_debt_reads/1
```

## 6. User selects text while a response is pending

* User asks the OpenAI model for a technical-debt plan.
* Model reads five project files and receives their contents.
* User drags a selection across the conversation while the next response is pending.

```sh
cargo bench --bench conversation -- technical_debt_selection_drag
```
