//! Round-trip (serialize -> deserialize) parity tests for every message type.

use ipu4_fw_msgs::*;

/// Assert `serialize` produces `SERIALIZED_SIZE` bytes and that
/// deserialize->serialize reproduces the exact same bytes.
fn assert_byte_roundtrip<T: Marshal>(v: &T) {
    let bytes = v.serialize();
    assert_eq!(bytes.len(), T::SERIALIZED_SIZE, "serialized size mismatch");
    let parsed = T::deserialize(&bytes).expect("deserialize");
    assert_eq!(parsed.serialize(), bytes, "byte round-trip mismatch");
}

#[test]
fn resolution_roundtrip() {
    let r = Resolution::new(1920, 1080);
    assert_byte_roundtrip(&r);
    assert_eq!(Resolution::deserialize(&r.serialize()).unwrap(), r);
}

#[test]
fn output_pin_payload_roundtrip() {
    let p = OutputPinPayload::new(BufferId(0xdead_beef_cafe), CssVirtualAddress(0x1234_5678));
    assert_byte_roundtrip(&p);
    assert_eq!(OutputPinPayload::deserialize(&p.serialize()).unwrap(), p);
}

#[test]
fn param_pin_roundtrip() {
    let p = ParamPin::new(BufferId(0x42), CssVirtualAddress(0x84));
    assert_byte_roundtrip(&p);
    assert_eq!(ParamPin::deserialize(&p.serialize()).unwrap(), p);
}

#[test]
fn output_pin_info_roundtrip() {
    let p = OutputPinInfo {
        output_res: Resolution::new(640, 480),
        stride: 1280,
        watermark_in_lines: 7,
        send_irq: 1,
        input_pin_id: 2,
        pt: 3,
        ft: 20,
        online: 1,
    };
    assert_byte_roundtrip(&p);
    assert_eq!(OutputPinInfo::deserialize(&p.serialize()).unwrap(), p);
}

#[test]
fn input_pin_info_roundtrip() {
    let p = InputPinInfo {
        input_res: Resolution::new(4096, 2160),
        dt: 0x2b,
        mipi_store_mode: 1,
        bits_per_pix: 10,
    };
    assert_byte_roundtrip(&p);
    assert_eq!(InputPinInfo::deserialize(&p.serialize()).unwrap(), p);
}

#[test]
fn isa_config_roundtrip_and_bit_packing() {
    let cfg = IsaConfig {
        isa_res: [Resolution::new(10, 20), Resolution::new(30, 40)],
        blc: true,
        lsc: false,
        dpc: true,
        downscaler: false,
        awb: true,
        af: false,
        ae: true,
        paf: 0xa5,
        send_irq_stats_ready: true,
        send_resp_stats_ready: false,
    };
    assert_byte_roundtrip(&cfg);
    assert_eq!(IsaConfig::deserialize(&cfg.serialize()).unwrap(), cfg);

    // Verify the documented bit positions explicitly.
    let word = cfg.pack_cfg();
    assert_eq!(word & 1, 1); // blc bit 0
    assert_eq!((word >> 2) & 1, 1); // dpc bit 2
    assert_eq!((word >> 4) & 1, 1); // awb bit 4
    assert_eq!((word >> 6) & 1, 1); // ae bit 6
    assert_eq!((word >> 7) & 0xff, 0xa5); // paf bits 7..15
    assert_eq!((word >> 15) & 1, 1); // send_irq_stats_ready bit 15
    assert_eq!((word >> 16) & 1, 0); // send_resp_stats_ready bit 16
}

#[test]
fn cropping_roundtrip_negative() {
    let c = Cropping {
        top_offset: -5,
        left_offset: 10,
        bottom_offset: -100,
        right_offset: 2000,
    };
    assert_byte_roundtrip(&c);
    assert_eq!(Cropping::deserialize(&c.serialize()).unwrap(), c);
}

#[test]
fn stream_cfg_roundtrip() {
    let cfg = StreamCfgData::builder()
        .isa_cfg(IsaConfig {
            isa_res: [Resolution::new(1, 2), Resolution::new(3, 4)],
            dpc: true,
            paf: 9,
            ..Default::default()
        })
        .add_input_pin(InputPinInfo {
            input_res: Resolution::new(1920, 1080),
            dt: 0x2c,
            mipi_store_mode: 0,
            bits_per_pix: 12,
        })
        .unwrap()
        .add_output_pin(OutputPinInfo {
            output_res: Resolution::new(1920, 1080),
            stride: 3840,
            watermark_in_lines: 1,
            send_irq: 1,
            input_pin_id: 0,
            pt: 0,
            ft: 20,
            online: 1,
        })
        .unwrap()
        .compfmt(0x1234)
        .src(1)
        .vc(2)
        .isl_use(IslUse::SingleDualIsl)
        .build()
        .unwrap();

    assert_byte_roundtrip(&cfg);
    let parsed = StreamCfgData::deserialize(&cfg.serialize()).unwrap();
    assert_eq!(parsed, cfg);
    assert_eq!(parsed.nof_input_pins(), 1);
    assert_eq!(parsed.nof_output_pins(), 1);
}

#[test]
fn frame_buff_set_roundtrip() {
    let mut b = FrameBuffSet::builder()
        .process_group_light(ParamPin::new(BufferId(0xaa), CssVirtualAddress(0xbb)))
        .send_irq_eof(true)
        .send_resp_sof(true);
    for i in 0..6u64 {
        b = b
            .add_output_pin(OutputPinPayload::new(
                BufferId(i),
                CssVirtualAddress(i as u32 * 2),
            ))
            .unwrap();
    }
    let fbs = b.build().unwrap();
    assert_byte_roundtrip(&fbs);
    assert_eq!(FrameBuffSet::deserialize(&fbs.serialize()).unwrap(), fbs);
}

#[test]
fn send_token_roundtrip_all_types() {
    for st in [
        SendType::StreamOpen,
        SendType::StreamStart,
        SendType::StreamStartAndCapture,
        SendType::StreamCapture,
        SendType::StreamStop,
        SendType::StreamFlush,
        SendType::StreamClose,
    ] {
        let tok = SendQueueToken::complex(BufferId(0x99), CssVirtualAddress(0x11), st);
        assert_byte_roundtrip(&tok);
        assert_eq!(SendQueueToken::deserialize(&tok.serialize()).unwrap(), tok);
    }
    // simple command form has zero handle/payload.
    let simple = SendQueueToken::simple(SendType::StreamClose);
    assert_eq!(simple.buf_handle, BufferId(0));
    assert_eq!(simple.payload, CssVirtualAddress(0));
    assert_byte_roundtrip(&simple);
}

#[test]
fn proxy_send_token_roundtrip() {
    let tok = ProxySendQueueToken::new(1, 2, 3, 4);
    assert_byte_roundtrip(&tok);
    assert_eq!(
        ProxySendQueueToken::deserialize(&tok.serialize()).unwrap(),
        tok
    );
}

#[test]
fn resp_info_roundtrip_all_types() {
    for (i, rt) in [
        RespType::StreamOpenDone,
        RespType::PinDataReady,
        RespType::FrameSof,
        RespType::StatsDataReady,
    ]
    .into_iter()
    .enumerate()
    {
        let resp = RespInfo {
            buf_id: BufferId(0x1000 + i as u64),
            pin: OutputPinPayload::new(BufferId(0x55), CssVirtualAddress(0x66)),
            process_group_light: ParamPin::new(BufferId(0x77), CssVirtualAddress(0x88)),
            error_info: ErrorInfo {
                error: IsysError::None,
                error_details: 0,
            },
            timestamp: [1, 2],
            stream_handle: i as u8,
            resp_type: rt,
            pin_id: 1,
            acc_id: 0,
        };
        assert_byte_roundtrip(&resp);
        assert_eq!(RespInfo::deserialize(&resp.serialize()).unwrap(), resp);
    }
}

#[test]
fn proxy_resp_info_roundtrip() {
    let presp = ProxyRespInfo {
        request_id: 0xfeed,
        error_info: ProxyErrorInfo {
            error: ProxyError::InvalidWriteRegion,
            error_details: 7,
        },
    };
    assert_byte_roundtrip(&presp);
    assert_eq!(
        ProxyRespInfo::deserialize(&presp.serialize()).unwrap(),
        presp
    );
}

#[test]
fn error_info_roundtrip() {
    let e = ErrorInfo {
        error: IsysError::InsufficientResources,
        error_details: 0x1234,
    };
    assert_byte_roundtrip(&e);
    assert_eq!(ErrorInfo::deserialize(&e.serialize()).unwrap(), e);
}
