# Status panel

## Requirements
* A status panel appears on the bottom line of the console.
* The left side displays the response indicator, sent tokens, received tokens, and cache hits.
* The response indicator appears after the first message is sent, before the token statistics.
* The response indicator shows elapsed time, updated every second while waiting.
* The right side displays the provider and model name.
* The rightmost element displays the selected model's thinking level when defined.
* Cache hits are displayed as a percentage rounded to the nearest whole number.
* Session statistics and the latest response time are restored with the session.
* A new session starts with empty statistics.

## User scenarios
### 1. User views session statistics
* User sends a message to `LFM3-Omni-2B-Q4_K_M` through llama.
* User receives a response with 82 prompt tokens, 0 cached tokens, and 383 completion tokens.
* User sees `↑82 ↓383 CH0%` on the left of the bottom panel.
* User sees `llama LFM3-Omni-2B-Q4_K_M` on the right.

### 2. User views response time
* User opens the application and sees `↑0 ↓0 CH0%` at the left of the bottom panel.
* User sends a message and sees an animated spinner and `0s` before the token statistics.
* After 5 seconds, user sees the spinner and `5s` while waiting.
* User receives the response after 5.2 seconds and sees `⏱ 5s ↑82 ↓383 CH0%`.
* User sends another message and sees the spinner and `1m 12s` while waiting.
* User receives the response after 1 minute and 12 seconds.
* User sees `⏱ 1m 12s ↑164 ↓766 CH0%`.

### 3. User scrolls the conversation
* User receives a response longer than the conversation area.
* User presses Page Up and sees an earlier part of the conversation.
* The bottom panel remains visible with the same statistics.

### 4. User views the thinking level

* User starts a session with Qwen3.8-27B-UD-IQ4_XS.
* User sees `llama Qwen3.8-27B-UD-IQ4_XS low` on the right of the bottom panel.

### 5. User restores session statistics

* User receives a response with 82 prompt tokens and 383 completion tokens.
* User restores the session and sees the response time and `↑82 ↓383 CH0%`.
* User receives another response with 82 prompt tokens and 383 completion tokens.
* User sees `↑164 ↓766 CH0%`.
* User starts a new session and sees `↑0 ↓0 CH0%`.

### 6. User views rounded cache hits

* User sends a message to Qwen3.8-27B-UD-IQ4_XS.
* User receives a response with 85 prompt tokens and 81 cached tokens.
* User sees `CH95%` in the status panel.
