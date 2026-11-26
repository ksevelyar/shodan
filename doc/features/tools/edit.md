# Edit file

## Requirements

* User can ask Shodan to edit an existing text file.
* User can ask Shodan to make multiple changes to a file together.
* Changes preserve the text outside the edited regions.
* Successful edits show a syntax-highlighted diff with line numbers in the tool output.
* User is informed when a file cannot be edited.
* If any change in a request fails, the file remains unchanged.

## User scenarios

### 1. User edits a file

* A file named `fibonacci.rs` contains:

```rust
fn fibonacci(number: u32) -> u64 {
    match number {
        0 => 0,
        1 => 1,
        _ => fibonacci(number - 1) + fibonacci(number - 2),
    }
}
```

* User asks Shodan to replace the recursive Fibonacci implementation with an iterative one.
* User sees the editing request and a syntax-highlighted diff in the conversation. Removed lines show original line numbers and added lines show updated line numbers:

```diff
  1   fn fibonacci(number: u32) -> u64 {
- 2       match number {
- 3           0 => 0,
- 4           1 => 1,
- 5           _ => fibonacci(number - 1) + fibonacci(number - 2),
- 6       }
- 7   }
+ 2       let mut previous = 0;
+ 3       let mut current = 1;
+ 4       for _ in 0..number {
+ 5           (previous, current) = (current, previous + current);
+ 6       }
+ 7       previous
+ 8   }
```

* The file contains:

```rust
fn fibonacci(number: u32) -> u64 {
    let mut previous = 0;
    let mut current = 1;
    for _ in 0..number {
        (previous, current) = (current, previous + current);
    }
    previous
}
```

* Model reports the change.

### 2. User edits multiple parts of a file

* A file named `greeting.txt` contains `Hello, world! Goodbye, world!`.
* User asks Shodan to change `Hello` to `Welcome` and `Goodbye` to `Farewell` together.
* User sees the editing request and its result in the conversation.
* The file contains `Welcome, world! Farewell, world!`.
* Model reports the changes.

### 3. User edits a nonexistent file

* The file `missing.txt` does not exist in the session directory.
* User asks Shodan to edit `missing.txt`.
* User sees the editing request and its error in the conversation.
* User is informed that the file could not be edited.

### 4. User edits text that cannot be found

* A file named `greeting.txt` contains `Hello, world!`.
* User asks Shodan to change `Goodbye` to `Farewell` in `greeting.txt`.
* User sees the editing request and its error in the conversation.
* The file still contains `Hello, world!`.
* User is informed that the text could not be found.

### 5. User edits text that occurs more than once

* A file named `greeting.txt` contains `Hello, world! Goodbye, world!`.
* User asks Shodan to replace `world` in `greeting.txt`.
* User sees the editing request and its error in the conversation.
* The file still contains `Hello, world! Goodbye, world!`.
* User is informed that the text is ambiguous.

### 6. User requests overlapping changes

* A file named `greeting.txt` contains `Hello, world!`.
* User asks Shodan to replace `Hello, world` and `world` together.
* User sees the editing request and its error in the conversation.
* The file still contains `Hello, world!`.
* User is informed that the changes overlap.

### 7. User requests a change alongside an invalid change

* A file named `greeting.txt` contains `Hello, world!`.
* User asks Shodan to change `Hello` to `Welcome` and `Goodbye` to `Farewell` together.
* User sees the editing request and its error in the conversation.
* The file still contains `Hello, world!`.
* User is informed that `Goodbye` could not be found.

## Tool contract

* `edit` accepts a relative or absolute `path` and a nonempty `edits` list.
* Each edit contains a nonempty `oldText` and a `newText`. An empty `newText` deletes the matched text.
* Each `oldText` must match exactly one region of the original file. All regions must be disjoint.
* All replacements are validated against the original file before changes are written.
* On success, the result includes a syntax-highlighted diff with removed and added lines numbered according to the original and updated file respectively.
* Diff lines use `-` for removals, `+` for additions and a space for unchanged context. Unchanged context uses original line numbers.
* The diff includes up to three unchanged lines around each change. Nearby changes share context.
* On failure, the result explains the error.
