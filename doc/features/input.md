# Input

## Requirements

* A textarea allows the user to compose messages.
* Enter submits the message in the input.
* Shift+Enter inserts a newline at the input cursor.
* Left and Right move the input cursor by one character.
* Typing and pasting insert text at the input cursor.
* Backspace deletes the character immediately before the input cursor.
* Up and Down move the input cursor between visible lines.
* Up at the top line selects the previous user message in the current conversation.
* Down at the bottom line selects the next user message in the current conversation.
* Navigating down past the most recent user message restores the draft that was present when history navigation began.
* The message input grows and shrinks to fit its visible lines, including lines created by wrapping, up to 50% of terminal height.
* When the input contains more lines than fit at its maximum height, the input scrolls to keep the cursor visible.
* Pasted text remains in the input until the user submits the message.

## User scenarios

### 1. User sends a multiline message

* User types `first`.
* User presses Shift+Enter and types `second`.
* User sees `first` and `second` on separate lines in the message input.
* User presses Enter.
* User sees a message containing `first` and `second` on separate lines in the conversation.

### 2. User edits a message with horizontal arrow keys

* User types `ac`.
* User presses Left, types `b`, and sees `abc`.
* User presses Left and Backspace, and sees `bc`.
* User presses Right, types `d`, and sees `bdc`.
* User presses Enter and sees `bdc` in the conversation.

### 3. User edits a multiline message with vertical arrow keys

* User types `first`, presses Shift+Enter, and types `third`.
* User presses Up and Shift+Enter, then types `second`.
* User presses Down and types `!`.
* User sees `first`, `second`, and `third!` on separate lines.
* User presses Enter and sees the three lines in the conversation.

### 4. User edits a wrapped message with vertical arrow keys

* User types a message that wraps onto a second visible line.
* User presses Up and inserts text in the first visible line.
* User presses Down and inserts text in the second visible line.
* User sends the edited message.

### 5. User navigates message history and restores a draft

* User sends `first message` and then `second message`.
* User types `draft`.
* User presses Up and sees `second message` in the input.
* User presses Up and sees `first message` in the input.
* User presses Down and sees `second message` in the input.
* User presses Down and sees `draft` in the input.

### 6. User pastes multiline text at the cursor

* User types `start end`.
* User moves the cursor to the space before `end`.
* User pastes `one`, a newline, and `two`.
* User sees `startone` on the first input line and `two end` on the second input line.
* User sees the input expand to show both lines.
* User presses Enter and sees the same two lines in the conversation.

### 7. User grows and shrinks the message input

* User starts with an empty message input.
* User types enough text to wrap onto a second visible line.
* User sees the input grow to show both lines.
* User deletes enough text for the message to fit on one line.
* User sees the input shrink to one visible line.

### 8. User edits input beyond the height limit

* User opens Shodan in a terminal 20 rows high.
* User pastes a message with 12 short numbered lines.
* User sees an input area no more than 10 rows high and sees the cursor at the end of the message.
* User moves the cursor left through the message to the first line.
* User sees the first line and the cursor in the input.
* User presses Enter and sees all 12 lines in the conversation.
