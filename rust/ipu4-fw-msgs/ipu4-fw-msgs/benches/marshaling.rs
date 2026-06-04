//! Criterion benchmarks for message construction + serialization throughput.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use ipu4_fw_msgs::{
    BufferId, Cropping, CssVirtualAddress, FrameBuffSet, InputPinInfo, IslUse, Marshal,
    OutputPinInfo, OutputPinPayload, ParamPin, ProxySendQueueToken, Resolution, SendQueueToken,
    SendType, StreamCfgData,
};

fn build_stream_cfg() -> StreamCfgData {
    let mut b = StreamCfgData::builder()
        .src(2)
        .vc(1)
        .isl_use(IslUse::SingleIsa)
        .compfmt(0xdead_beef)
        .crop(
            0,
            Cropping {
                top_offset: 1,
                left_offset: 2,
                bottom_offset: 3,
                right_offset: 4,
            },
        )
        .unwrap();
    for i in 0..4u32 {
        b = b
            .add_input_pin(InputPinInfo {
                input_res: Resolution::new(1920 + i, 1080 + i),
                dt: 0x2b,
                mipi_store_mode: 0,
                bits_per_pix: 0,
            })
            .unwrap();
    }
    for i in 0..6u32 {
        b = b
            .add_output_pin(OutputPinInfo {
                output_res: Resolution::new(1920, 1080),
                stride: 3840,
                watermark_in_lines: i,
                send_irq: 1,
                input_pin_id: (i % 4) as u8,
                pt: 0,
                ft: 20,
                online: 1,
            })
            .unwrap();
    }
    b.auto_bits_per_pix(true).build().unwrap()
}

fn build_frame_buff_set() -> FrameBuffSet {
    let mut b = FrameBuffSet::builder()
        .process_group_light(ParamPin::new(BufferId(0x11), CssVirtualAddress(0x22)))
        .send_irq_sof(true)
        .send_resp_eof(true);
    for i in 0..6u64 {
        b = b
            .add_output_pin(OutputPinPayload::new(
                BufferId(0x1000 + i),
                CssVirtualAddress(0x2000 + i as u32),
            ))
            .unwrap();
    }
    b.build().unwrap()
}

fn bench_construction(c: &mut Criterion) {
    let mut g = c.benchmark_group("construction");
    g.bench_function("stream_cfg_build", |b| {
        b.iter(|| black_box(build_stream_cfg()))
    });
    g.bench_function("frame_buff_set_build", |b| {
        b.iter(|| black_box(build_frame_buff_set()))
    });
    g.bench_function("send_token_build", |b| {
        b.iter(|| {
            black_box(SendQueueToken::complex(
                black_box(BufferId(0xabcd)),
                black_box(CssVirtualAddress(0x1234)),
                SendType::StreamCapture,
            ))
        })
    });
    g.finish();
}

fn bench_serialization(c: &mut Criterion) {
    let cfg = build_stream_cfg();
    let fbs = build_frame_buff_set();
    let token = SendQueueToken::complex(
        BufferId(0xabcd),
        CssVirtualAddress(0x1234),
        SendType::StreamOpen,
    );
    let proxy = ProxySendQueueToken::new(1, 2, 3, 4);

    let mut g = c.benchmark_group("serialize");
    g.throughput(Throughput::Bytes(StreamCfgData::SERIALIZED_SIZE as u64));
    g.bench_function("stream_cfg", |b| {
        let mut buf = vec![0u8; StreamCfgData::SERIALIZED_SIZE];
        b.iter(|| {
            cfg.serialize_into(black_box(&mut buf)).unwrap();
            black_box(&buf);
        })
    });
    g.throughput(Throughput::Bytes(FrameBuffSet::SERIALIZED_SIZE as u64));
    g.bench_function("frame_buff_set", |b| {
        let mut buf = vec![0u8; FrameBuffSet::SERIALIZED_SIZE];
        b.iter(|| {
            fbs.serialize_into(black_box(&mut buf)).unwrap();
            black_box(&buf);
        })
    });
    g.throughput(Throughput::Bytes(SendQueueToken::SERIALIZED_SIZE as u64));
    g.bench_function("send_token", |b| {
        let mut buf = vec![0u8; SendQueueToken::SERIALIZED_SIZE];
        b.iter(|| {
            token.serialize_into(black_box(&mut buf)).unwrap();
            black_box(&buf);
        })
    });
    g.bench_function("proxy_token", |b| {
        let mut buf = vec![0u8; ProxySendQueueToken::SERIALIZED_SIZE];
        b.iter(|| {
            proxy.serialize_into(black_box(&mut buf)).unwrap();
            black_box(&buf);
        })
    });
    g.finish();
}

fn bench_deserialization(c: &mut Criterion) {
    let cfg_bytes = build_stream_cfg().serialize();
    let fbs_bytes = build_frame_buff_set().serialize();

    let mut g = c.benchmark_group("deserialize");
    g.throughput(Throughput::Bytes(cfg_bytes.len() as u64));
    g.bench_function("stream_cfg", |b| {
        b.iter(|| black_box(StreamCfgData::deserialize(black_box(&cfg_bytes)).unwrap()))
    });
    g.throughput(Throughput::Bytes(fbs_bytes.len() as u64));
    g.bench_function("frame_buff_set", |b| {
        b.iter(|| black_box(FrameBuffSet::deserialize(black_box(&fbs_bytes)).unwrap()))
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_construction,
    bench_serialization,
    bench_deserialization
);
criterion_main!(benches);
