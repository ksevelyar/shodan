# Read file

## Requirements

* User can ask Shodan to read a text file.
* User can ask Shodan to read part of a text file.
* File reading requests show `read` followed by the file path.
* File reading results show line numbers from the original file and syntax highlighting for code on a purple background across the conversation width.
* Model receives the requested file contents.
* User is informed when a file cannot be read.

## User scenarios

### 1. User reads a file

* A file named `greeting.txt` contains the text `Hello, Shodan!`.
* User asks Shodan to read `greeting.txt`.
* User sees `read` followed by the file path and its result, `Hello, Shodan!`, on a purple background across the conversation width.
* Model responds using the file contents.

### 2. User reads part of a file

* A file named `lines.txt` contains `First line`, `Second line` and `Third line` on separate lines.
* User asks Shodan to read the second line of `lines.txt`.
* User sees the file reading request and its result, `Second line` numbered as line 2, in the conversation.
* Model responds using the requested line.

### 3. User reads a nonexistent file

* The file `missing.txt` does not exist in the session directory.
* User asks Shodan to read `missing.txt`.
* User sees the file reading request and its error in the conversation.
* User is informed that the file could not be read.

### 4. User reads a source file

* A file named `fibonacci.rs` contains:

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

* User asks Shodan to read `fibonacci.rs`.
* User sees the file reading request and its result in the conversation. The result is syntax-highlighted as Rust and shows the original line numbers:

```text
1  fn fibonacci(number: u32) -> u64 {
2      let mut previous = 0;
3      let mut current = 1;
4      for _ in 0..number {
5          (previous, current) = (current, previous + current);
6      }
7      previous
8  }
```

* Model responds using the file contents.
