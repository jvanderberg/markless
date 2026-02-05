//! Benchmarks for markdown parsing.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use markless::document::Document;

fn bench_parse_simple(c: &mut Criterion) {
    let md = "# Hello\n\nWorld";
    c.bench_function("parse_simple", |b| {
        b.iter(|| Document::parse(black_box(md)).unwrap())
    });
}

fn bench_parse_medium(c: &mut Criterion) {
    let md = include_str!("../tests/fixtures/simple.md");
    c.bench_function("parse_medium", |b| {
        b.iter(|| Document::parse(black_box(md)).unwrap())
    });
}

criterion_group!(benches, bench_parse_simple, bench_parse_medium);
criterion_main!(benches);
