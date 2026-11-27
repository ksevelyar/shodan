# Theme

## Requirements
* The interface uses the [Joker color palette](https://github.com/ksevelyar/joker.vim/blob/master/colors/joker.lua).
* The conversation background is `#1B161F` and normal text is `#C8D6F3`.
* The top and bottom panels have a `#26212A` background and `#817C86` text.
* A horizontal separator appears below the top panel.
* Secondary text is `#919FBA`.
* Thinking text, comments, and separators are `#5E6B85`.
* Context instructions, user messages, and Markdown headings use `#D5AA2A` text on the conversation background.
* Successful tool calls and output use the conversation background.
* Failed tool calls and output use a `#C84E4E` background with dark text.
* Selected text uses a `#D5AA2A` background and `#1B161F` text throughout the interface.
* Error text and diff deletion markers use `#C84E4E`.
* Diff addition markers use `#57A16D`.
* Diff lines use the conversation background.
* Inline code and variables use `#54C6B5`.
* Functions use `#57A16D`.
* Keywords use `#C84E4E`.
* Strings use `#597A90`.
* Types use `#D5AA2A`.
* Numbers and special values use `#FFCE50`.
* Identifiers use `#C16FB4`.

## User scenarios
### 1. User views the interface
* User opens Shodan and sees grey text on both grey panels and light text on the dark conversation background.
* User sends a message and sees yellow text on the conversation background.
* User sees muted separators and thinking text.

### 2. User reads source file
* User reads a Rust source file and sees red keywords and green functions on the conversation background.
* User receives the file in a code block and sees the same syntax colors.

### 3. User edits file
* User edits a file and sees a green addition marker on the conversation background.

### 4. User is informed when command fails
* User runs a failing command and sees the command and error on red backgrounds.

### 5. User selects visible text
* User selects conversation and panel text and sees dark text on a yellow selection background.
