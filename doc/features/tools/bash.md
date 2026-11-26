# Run bash commands

## Requirements

* User can ask Shodan to run a bash command.
* User sees the command with Bash syntax highlighting and its output in the conversation.
* Successful commands have a purple background and failed commands have a red background.
* User is informed when a command fails.
* User can abort a running command with Esc.

## User scenarios

### 1. User runs a command

* User asks Shodan to run `echo hello` using the bash tool.
* User sees `$ echo hello` with Bash syntax highlighting and its output, `hello`, on a purple background.
* Model responds using the command output.

### 2. User is informed when a command fails

* User asks Shodan to run a command that exits with a non-zero status.
* User sees `$ exit 3` and its failure on a red background.
* User is informed that the command failed and why.

### 3. User aborts a running command

* User asks Shodan to run a long-running command.
* While the command runs, user presses Esc.
* User sees the request was aborted and the command stops running.
