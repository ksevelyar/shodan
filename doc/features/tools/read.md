# Read file

## Requirements

* User can ask Shodan to read a text file.
* User can ask Shodan to read part of a text file.
* User can ask Shodan to read an image file and respond using its contents.
* Images exceeding 2000 pixels in either dimension are sent as JPEG images fitting within 2000 by 2000 pixels.
* Image files exceeding 2 MB are sent as JPEG images.
* When an image is converted, the read result shows its original and output dimensions.
* File reading requests show `read` followed by the file path.
* File reading requests and results share a continuous purple background across the conversation width.
* File reading results show line numbers from the original file and syntax highlighting for code.
* Model receives the requested file contents.
* User is informed when a file cannot be read.

## User scenarios

### 1. User reads a file

* A file named `greeting.txt` contains the text `Hello, Shodan!`.
* User asks Shodan to read `greeting.txt`.
* User sees `read` followed by the file path and its result, `Hello, Shodan!`, on a continuous purple background across the conversation width.
* Model responds using the file contents.

### 2. User reads multiple files

* User asks the model to inspect several files.
* Model requests the files together.
* User sees each file reading request followed by its result.
* Model receives the file contents.

### 3. User reads part of a file

* A file named `lines.txt` contains `First line`, `Second line` and `Third line` on separate lines.
* User asks Shodan to read the second line of `lines.txt`.
* User sees the file reading request and its result, `Second line` numbered as line 2, in the conversation.
* Model responds using the requested line.

### 4. User reads a nonexistent file

* The file `missing.txt` does not exist in the session directory.
* User asks Shodan to read `missing.txt`.
* User sees the file reading request and its error in the conversation.
* User is informed that the file could not be read.

### 5. User reads a source file

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

### 6. User asks model to describe an image with one word

* The image `tests/features/fixtures/mcp/blender/describe-image-with-one-word.png` is available in the project.
* User asks Shodan to describe the image in one word.
* Model calls read for the image file.
* User sees the read request and its result in the conversation.
* Model responds with the word `Molecule`.

### 7. User asks model to read an image with large dimensions

* The image `tests/features/fixtures/mcp/blender/akira.png` is 2560 by 1600 pixels.
* User asks Shodan to describe the image.
* Model calls read for the image file.
* User sees the read request and a result showing the original dimensions, 2560 by 1600, and the output dimensions, 2000 by 1250.
* Model receives the image as a 2000 by 1250 JPEG.
* Model responds using the image contents.

### 8. User asks model to read a large image file

* The image `tests/features/fixtures/mcp/blender/akira.bmp` is 1999 by 1249 pixels and larger than 2 MB.
* User asks Shodan to describe the image.
* Model calls read for the image file.
* User sees the read request and a result showing both the original and output dimensions as 1999 by 1249.
* Model receives the image as a 1999 by 1249 JPEG.
* Model responds using the image contents.
