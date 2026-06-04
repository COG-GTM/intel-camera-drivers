//! Malformed-input rejection tests. The C code would silently read past the
//! end of a short buffer or accept out-of-range enum/count values; the safe
//! port rejects all of these with a typed [`Error`].

use ipu4_fw_msgs::*;

#[test]
fn short_buffers_are_rejected() {
    // Empty buffer for every fixed-size message type.
    assert!(matches!(
        Resolution::deserialize(&[]),
        Err(Error::BufferTooSmall { .. })
    ));
    assert!(matches!(
        StreamCfgData::deserialize(&[]),
        Err(Error::BufferTooSmall { .. })
    ));
    assert!(matches!(
        FrameBuffSet::deserialize(&[]),
        Err(Error::BufferTooSmall { .. })
    ));
    assert!(matches!(
        RespInfo::deserialize(&[0u8; 10]),
        Err(Error::BufferTooSmall { .. })
    ));

    // One byte short of a full stream cfg.
    let buf = vec![0u8; StreamCfgData::SERIALIZED_SIZE - 1];
    assert!(matches!(
        StreamCfgData::deserialize(&buf),
        Err(Error::BufferTooSmall { .. })
    ));
}

#[test]
fn unknown_send_type_is_rejected() {
    let mut bytes =
        SendQueueToken::complex(BufferId(1), CssVirtualAddress(2), SendType::StreamOpen)
            .serialize();
    // send_type is the trailing u32.
    let n = bytes.len();
    bytes[n - 4..].copy_from_slice(&0x00ff_u32.to_le_bytes());
    assert!(matches!(
        SendQueueToken::deserialize(&bytes),
        Err(Error::InvalidEnum {
            field: "send_type",
            value: 255
        })
    ));
}

#[test]
fn unknown_resp_type_is_rejected() {
    let resp = RespInfo {
        buf_id: BufferId(0),
        pin: OutputPinPayload::default(),
        process_group_light: ParamPin::default(),
        error_info: ErrorInfo::default(),
        timestamp: [0, 0],
        stream_handle: 0,
        resp_type: RespType::StreamOpenDone,
        pin_id: 0,
        acc_id: 0,
    };
    let mut bytes = resp.serialize();
    // resp_type byte sits right after stream_handle:
    // buf_id(8)+pin(16)+pgl(16)+error_info(8)+timestamp(8)=56, stream_handle@56, type@57.
    bytes[57] = 200;
    assert!(matches!(
        RespInfo::deserialize(&bytes),
        Err(Error::InvalidEnum {
            field: "resp_type",
            value: 200
        })
    ));
}

#[test]
fn unknown_isys_error_is_rejected() {
    let e = ErrorInfo {
        error: IsysError::None,
        error_details: 0,
    };
    let mut bytes = e.serialize();
    // error is the leading u32.
    bytes[0..4].copy_from_slice(&12345u32.to_le_bytes());
    assert!(matches!(
        ErrorInfo::deserialize(&bytes),
        Err(Error::InvalidEnum {
            field: "isys_error",
            ..
        })
    ));
}

#[test]
fn unknown_proxy_error_is_rejected() {
    let p = ProxyRespInfo {
        request_id: 1,
        error_info: ProxyErrorInfo {
            error: ProxyError::None,
            error_details: 0,
        },
    };
    let mut bytes = p.serialize();
    // request_id(4) then proxy error u32 @4.
    bytes[4..8].copy_from_slice(&9u32.to_le_bytes());
    assert!(matches!(
        ProxyRespInfo::deserialize(&bytes),
        Err(Error::InvalidEnum {
            field: "proxy_error",
            value: 9
        })
    ));
}

#[test]
fn out_of_range_pin_counts_are_rejected() {
    // Build a valid single-pin stream cfg, then corrupt nof_input_pins.
    let cfg = StreamCfgData::builder()
        .add_input_pin(InputPinInfo::default())
        .unwrap()
        .build()
        .unwrap();
    let mut bytes = cfg.serialize();
    // nof_input_pins is at offset 280 (compfmt @276..280).
    bytes[280] = 5; // > INTEL_IPU4_MAX_IPINS (4)
    assert!(matches!(
        StreamCfgData::deserialize(&bytes),
        Err(Error::TooMany {
            field: "nof_input_pins",
            value: 5,
            max: 4
        })
    ));

    // nof_output_pins is at offset 281.
    let mut bytes2 = cfg.serialize();
    bytes2[281] = 7; // > INTEL_IPU4_MAX_OPINS (6)
    assert!(matches!(
        StreamCfgData::deserialize(&bytes2),
        Err(Error::TooMany {
            field: "nof_output_pins",
            value: 7,
            max: 6
        })
    ));
}
