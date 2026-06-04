//! Criterion benchmarks for the dw9714 hot path: the lens-position set cycle.
//!
//! Run with `cargo bench`. The mock I2C backend makes this measure the
//! driver's own overhead (validation, register packing, retry bookkeeping)
//! rather than real bus latency.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

use dw9714::mock::MockI2cBus;
use dw9714::{DeviceConfig, Dw9714, LensPosition};

fn bench_set_position(c: &mut Criterion) {
    let mut group = c.benchmark_group("lens_position_set");

    // Hot path: a single validated set_position call (the per-frame autofocus
    // update). Re-create the device per batch so recorded writes don't grow
    // unbounded and skew the measurement.
    group.bench_function("set_position_single", |b| {
        b.iter_batched(
            || Dw9714::probe(MockI2cBus::new(), DeviceConfig::new(0x0c)).unwrap(),
            |mut dev| {
                let pos = LensPosition::new(black_box(512)).unwrap();
                dev.set_position(black_box(pos)).unwrap();
                dev
            },
            BatchSize::SmallInput,
        );
    });

    // Raw-u16 entry point including range validation.
    group.bench_function("set_position_raw_validated", |b| {
        b.iter_batched(
            || Dw9714::probe(MockI2cBus::new(), DeviceConfig::new(0x0c)).unwrap(),
            |mut dev| {
                dev.set_position_raw(black_box(777)).unwrap();
                dev
            },
            BatchSize::SmallInput,
        );
    });

    // A full sweep across the DAC range — representative of a focus search.
    group.bench_function("set_position_sweep_0_1023", |b| {
        b.iter_batched(
            || Dw9714::probe(MockI2cBus::new(), DeviceConfig::new(0x0c)).unwrap(),
            |mut dev| {
                for p in 0..=LensPosition::MAX {
                    let pos = LensPosition::new_saturating(black_box(p));
                    dev.set_position(pos).unwrap();
                }
                dev
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_set_position);
criterion_main!(benches);
