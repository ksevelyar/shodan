# Status panel

## Requirements
* A status panel appears on the bottom line of the console.
* The left side displays the response indicator, sent tokens, received tokens, and cache hits.
* The response indicator appears after the first message is sent, before the token statistics.
* The response indicator shows elapsed time, updated every second while waiting.
* The right side displays the provider and model name.
* Cache hits are displayed as a percentage.

## User scenarios
### 1. User views session statistics
* User sends a message to `LFM3-Omni-2B-Q4_K_M` through llama.cpp.
* User receives a response with 82 prompt tokens, 0 cached tokens, and 383 completion tokens.
* User sees `↑82 ↓383 CH0.0%` on the left of the bottom panel.
* User sees `llama.cpp LFM3-Omni-2B-Q4_K_M` on the right.

### 2. User views response time
* User opens the application and sees `↑0 ↓0 CH0.0%` at the left of the bottom panel.
* User sends a message and sees an animated spinner and `0s` before the token statistics.
* After 5 seconds, user sees the spinner and `5s` while waiting.
* User receives the response after 5.2 seconds and sees `⏱ 5s ↑82 ↓383 CH0.0%`.
* User sends another message and sees the spinner and `1m 12s` while waiting.
* User receives the response after 1 minute and 12 seconds.
* User sees `⏱ 1m 12s ↑164 ↓766 CH0.0%`.

### 3. User scrolls the conversation
* User receives a response longer than the conversation area.
* User presses Page Up and sees an earlier part of the conversation.
* The bottom panel remains visible with the same statistics.
