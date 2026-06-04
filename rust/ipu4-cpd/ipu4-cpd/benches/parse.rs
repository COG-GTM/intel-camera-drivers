//! Criterion benchmarks for CPD parsing throughput.
//!
//! Measures the cost of full-file validation and pkg_dir assembly across a few
//! representative CPD sizes.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ipu4_cpd::{build_pkg_dir, validate_cpd_file_with_release, CpdFile};

#[path = "../tests/common/mod.rs"]
mod common;

use common::{CpdBuilder, FW_PKG_RELEASE};

fn make_file(components: usize) -> Vec<u8> {
    CpdBuilder::new().components(components).build()
}

fn bench_validate(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_cpd_file");
    for &components in &[1usize, 4, 8, 15] {
        let file = make_file(components);
        group.throughput(Throughput::Bytes(file.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(components),
            &file,
            |b, file| {
                b.iter(|| {
                    let r = validate_cpd_file_with_release(black_box(file), FW_PKG_RELEASE);
                    black_box(r).unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_build_pkg_dir(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_pkg_dir");
    for &components in &[1usize, 4, 8, 15] {
        let file = make_file(components);
        let cpd = CpdFile::parse(&file).unwrap();
        let met = cpd.metadata().unwrap();
        let mdl = cpd.moduledata().unwrap();
        let metadata = cpd.entry_data(&met).unwrap().to_vec();
        let moduledata = cpd.entry_data(&mdl).unwrap().to_vec();
        group.throughput(Throughput::Bytes(moduledata.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(components),
            &(moduledata, metadata),
            |b, (moduledata, metadata)| {
                b.iter(|| {
                    let r = build_pkg_dir(black_box(moduledata), black_box(metadata), 0x1000_0000);
                    black_box(r).unwrap();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_validate, bench_build_pkg_dir);
criterion_main!(benches);
