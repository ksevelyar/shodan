fn fibonacci(number: u32) -> u64 {
    let mut previous = 0;
    let mut current = 1;
    for _ in 0..number {
        (previous, current) = (current, previous + current);
    }
    previous
}
