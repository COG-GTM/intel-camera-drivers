//! Criterion benchmarks for ring-buffer and send/receive throughput.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use ipu4_fw_com::ring::index::{inc_index, num_free, num_messages};
use ipu4_fw_com::SharedRing;

fn bench_index_math(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_math");
    group.bench_function("num_messages", |b| {
        b.iter(|| num_messages(black_box(3), black_box(7), black_box(8)))
    });
    group.bench_function("num_free", |b| {
        b.iter(|| num_free(black_box(3), black_box(7), black_box(8)))
    });
    group.bench_function("inc_index", |b| {
        b.iter(|| inc_index(black_box(7), black_box(8)))
    });
    group.finish();
}

fn bench_send_recv_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("send_recv_roundtrip");
    for &token_size in &[8usize, 64, 256] {
        group.throughput(Throughput::Bytes(token_size as u64));
        group.bench_function(format!("token_{token_size}b"), |b| {
            let ring = SharedRing::new(64, token_size as u32).unwrap();
            let token = vec![0xABu8; token_size];
            let mut out = vec![0u8; token_size];
            b.iter(|| {
                ring.try_send(black_box(&token)).unwrap();
                ring.try_recv(black_box(&mut out)).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_fill_drain(c: &mut Criterion) {
    let mut group = c.benchmark_group("fill_drain");
    let queue_size = 256u32;
    let token_size = 32usize;
    group.throughput(Throughput::Elements((queue_size - 1) as u64));
    group.bench_function("fill_then_drain_full_queue", |b| {
        let token = vec![0x5Au8; token_size];
        b.iter_batched(
            || SharedRing::new(queue_size, token_size as u32).unwrap(),
            |ring| {
                for _ in 0..ring.capacity() {
                    ring.try_send(black_box(&token)).unwrap();
                }
                let mut out = vec![0u8; token_size];
                while ring.try_recv(&mut out).is_ok() {
                    black_box(&out);
                }
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_index_math,
    bench_send_recv_roundtrip,
    bench_fill_drain
);
criterion_main!(benches);
