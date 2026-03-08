fn main() {
    divan::main();
}

#[divan::bench]
fn placeholder() -> i32 {
    divan::black_box(2 + 2)
}
