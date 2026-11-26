# Interface

## Requirements

* A sticky top panel allows the user to switch system prompts.
* The user can scroll through conversation history.
* A textarea allows the user to compose messages.
* The status panel remains visible at the bottom while the user scrolls through conversation history.
* User can select text across the interface by dragging the mouse.
* User can scroll conversation history while keeping a selection.
* User can extend a conversation selection by dragging to the edge of the conversation.
* Shodan copies selected text to the clipboard when the user releases the mouse.
* User can configure a clipboard command in `~/.shodan/settings.json` as a list containing the program and its arguments.
* Shodan sends copied text to the command on standard input.
* Long conversation text remains readable across lines.
* The selection remains highlighted when the clipboard is unavailable.

## User scenarios

### 1. User selects visible text

* User receives a response and types a message.
* User drags across conversation text, the message input, and the status panel.
* User sees the selected text highlighted.
* User drags in the opposite direction and sees the new selection highlighted.

### 2. User scrolls with a conversation selection

* User receives a long response and selects part of the conversation.
* User scrolls up with the mouse wheel and scrolls back down.
* User sees the original selection highlighted.

### 3. User extends a selection beyond the visible conversation

* User receives a long response and starts selecting conversation text.
* User drags to the top of the conversation and holds the mouse button.
* Conversation scrolls up and extends the selection.

### 4. User copies selected text

* User selects text and releases the mouse.
* User pastes the selected text from the clipboard.

### 5. User copies a selection across scrolled conversation

* User selects conversation text and drags beyond the visible conversation.
* User releases the mouse and pastes the selected text from the clipboard.

### 6. User selects text without a clipboard

* User selects text while the clipboard is unavailable.
* User sees the selected text highlighted.

### 7. User reads long hyphenated text

* User sends a long message containing hyphens.
* User sees the complete message in the conversation.

### 8. User configures a clipboard command

* User configures a clipboard command.
* User starts OpenAI login.
* The command receives the authorization URL.

### 9. User copies selected text with a configured clipboard command

* User configures a clipboard command.
* User selects text and releases the mouse.
* The command receives the selected text.
