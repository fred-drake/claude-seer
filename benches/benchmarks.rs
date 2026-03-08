use criterion::{Criterion, criterion_group, criterion_main};

fn placeholder_benchmark(c: &mut Criterion) {
    c.bench_function("placeholder", |b| {
        b.iter(|| {
            let _ = 2 + 2;
        });
    });
}

criterion_group!(benches, placeholder_benchmark);
criterion_main!(benches);
