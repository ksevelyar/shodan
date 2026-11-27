# Interface

## Requirements

* The user can scroll through conversation history.
* The user can scroll through tool results and abort while a response is in progress.
* The status panel remains visible at the bottom while the user scrolls through conversation history.
* Long conversation text remains readable across lines.
* The conversation stays at the user's scroll position while new content arrives.
* Ctrl-End moves to the bottom and follows new content.

## User scenarios

### 1. User reads long hyphenated text

* User sends a long message containing hyphens.
* User sees the complete message in the conversation.

### 2. User scrolls through a file read and aborts a response

* User asks the model to read a source file.
* The model reads the file and continues its response.
* User scrolls through the source file while the response continues.
* The conversation stays at the user's scroll position.
* User presses Ctrl-End and follows the response at the bottom.
* User aborts the response.
