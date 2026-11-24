# Run bash commands

## Requirements

* User can ask Shodan to run a bash command.
* User is informed when a command fails.
* User can abort a running command with Esc.

## User scenarios

### 1. User runs a command

* User asks Shodan to run `echo hello from bash` using the bash tool.
* User sees the command request and its result, `hello from bash`, in the conversation.
* Model responds using the command output.

### 2. User is informed when a command fails

* User asks Shodan to run a command that exits with a non-zero status.
* User sees the command request and its failure in the conversation.
* User is informed that the command failed and why.

### 3. User aborts a running command

* User asks Shodan to run a long-running command.
* While the command runs, user presses Esc.
* User sees the request was aborted and the command stops running.
