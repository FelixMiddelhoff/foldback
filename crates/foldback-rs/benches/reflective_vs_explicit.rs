// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reflective vs. explicit hashing cost per tick (foldback-reflective-hashing.md
//! §5): not assumed free, measured. Run with:
//!
//! ```text
//! cargo bench -p foldback-rs --features bevy
//! ```
//!
//! Both paths hash the same `Unit` value for a representative entity
//! count (100 / 1,000) with no `.foldback` file attached — `Session`
//! without `record_to` still computes every hash, it just skips the
//! write, so this measures the hashing/walk cost itself, not I/O.

use std::hint::black_box;

use bevy_reflect::Reflect;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use foldback_core::session::Session;
use foldback_core::FoldbackHash;
use foldback_rs::bevy::hash_reflected;

#[derive(Reflect, FoldbackHash, Clone)]
struct Position {
    x: f32,
    y: f32,
    z: f32,
}

// `FieldBytes` isn't implemented for `Position` (it's not a primitive),
// so the manual/explicit path hashes its components directly rather
// than nesting a `#[foldback(hash)] pos: Position` field — this keeps
// the explicit side using only the API it's actually meant for (recipe
// 5's primitives-and-arrays scope), rather than reaching for `reflect`
// on the explicit side too, which would blur the comparison.
#[derive(Reflect, FoldbackHash, Clone)]
struct Unit {
    #[foldback(hash)]
    pos_x: f32,
    #[foldback(hash)]
    pos_y: f32,
    #[foldback(hash)]
    pos_z: f32,
    #[foldback(hash)]
    hp: i32,
    #[foldback(hash)]
    velocity: [f32; 3],
}

#[derive(Reflect, FoldbackHash, Clone)]
struct ReflectiveUnit {
    #[foldback(reflect)]
    pos: Position,
    #[foldback(hash)]
    hp: i32,
    #[foldback(hash)]
    velocity: [f32; 3],
}

fn make_units(n: usize) -> Vec<Unit> {
    (0..n)
        .map(|i| Unit {
            pos_x: i as f32,
            pos_y: i as f32 * 2.0,
            pos_z: i as f32 * 3.0,
            hp: 100 - (i % 100) as i32,
            velocity: [1.0, 0.0, -1.0],
        })
        .collect()
}

fn make_reflective_units(n: usize) -> Vec<ReflectiveUnit> {
    (0..n)
        .map(|i| ReflectiveUnit {
            pos: Position {
                x: i as f32,
                y: i as f32 * 2.0,
                z: i as f32 * 3.0,
            },
            hp: 100 - (i % 100) as i32,
            velocity: [1.0, 0.0, -1.0],
        })
        .collect()
}

fn bench_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("reflective_vs_explicit");
    for &entity_count in &[100usize, 1_000usize] {
        let explicit_units = make_units(entity_count);
        let reflective_units = make_reflective_units(entity_count);

        group.bench_with_input(
            BenchmarkId::new("explicit", entity_count),
            &entity_count,
            |b, _| {
                let mut session = Session::builder().peer_count(1).build().unwrap();
                b.iter(|| {
                    for (id, unit) in explicit_units.iter().enumerate() {
                        session.hash_fields(0, id as u64, black_box(unit)).unwrap();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("reflective", entity_count),
            &entity_count,
            |b, _| {
                let mut session = Session::builder().peer_count(1).build().unwrap();
                b.iter(|| {
                    for (id, unit) in reflective_units.iter().enumerate() {
                        hash_reflected(&mut session, 0, id as u64, "unit", black_box(unit))
                            .unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_hashing);
criterion_main!(benches);
