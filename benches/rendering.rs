//! Benchmarks for document rendering.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use gander::document::Document;

fn bench_visible_lines(c: &mut Criterion) {
    let md = include_str!("../tests/fixtures/simple.md");
    let doc = Document::parse(md).unwrap();

    c.bench_function("visible_lines", |b| {
        b.iter(|| doc.visible_lines(black_box(0), black_box(24)))
    });
}

criterion_group!(benches, bench_visible_lines);
criterion_main!(benches);
