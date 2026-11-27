# Text rendering

## Requirements

* User and model messages display Markdown highlighting.
* Headings appear in bold and retain their heading markers.
* Bold text appears in bold without its delimiters.
* Italic text appears as plain text without its delimiters.
* Inline code appears in a distinct color without its backticks.
* Markdown links display their destination URL.
* Fenced code blocks retain their fences and language label.
* Code blocks use language-specific highlighting when their language is recognized.
* Lists and blockquotes retain their Markdown structure.
* Copying highlighted text copies the displayed text.

## User scenarios

### 1. User reads Markdown in a message

* User sends a message containing a heading, bold text, italic text, inline code, a link, a list, and a blockquote.
* User sees the heading in bold with its marker.
* User sees bold text in bold without its delimiters.
* User sees italic text as plain text without its delimiters.
* User sees inline code in a distinct color without its backticks.
* User sees the link destination.
* User sees the list and blockquote markers.

### 2. User reads Markdown in a model response

* User asks for a response containing Markdown.
* Model streams the response.
* User sees its Markdown highlighted as it appears.

### 3. User reads a code block

* User receives a fenced code block with a recognized language.
* User sees the fences and language label.
* User sees the code highlighted for that language.

### 4. User copies highlighted text

* User selects highlighted conversation text and copies it.
* User pastes the displayed text.
