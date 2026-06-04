//! Boundary-value tests: max pins / streams / output pins, zero-length and
//! exact-size buffers, and newtype range validation.

use ipu4_fw_msgs::layout::{
    INTEL_IPU4_MAX_IPINS, INTEL_IPU4_MAX_OPINS, INTEL_IPU4_STREAM_ID_MAX, ISYS_MSG_INDEX,
    ISYS_PROXY_INDEX,
};
use ipu4_fw_msgs::*;

fn dummy_input() -> InputPinInfo {
    InputPinInfo {
        input_res: Resolution::new(1, 1),
        dt: 0x2b,
        mipi_store_mode: 0,
        bits_per_pix: 10,
    }
}

fn dummy_output() -> OutputPinInfo {
    OutputPinInfo {
        output_res: Resolution::new(1, 1),
        stride: 1,
        watermark_in_lines: 0,
        send_irq: 0,
        input_pin_id: 0,
        pt: 0,
        ft: 0,
        online: 0,
    }
}

#[test]
fn newtype_ranges() {
    assert!(InputPinId::new((INTEL_IPU4_MAX_IPINS - 1) as u8).is_ok());
    assert!(InputPinId::new(INTEL_IPU4_MAX_IPINS as u8).is_err());

    assert!(OutputPinId::new((INTEL_IPU4_MAX_OPINS - 1) as u8).is_ok());
    assert!(OutputPinId::new(INTEL_IPU4_MAX_OPINS as u8).is_err());

    assert!(StreamHandle::new((INTEL_IPU4_STREAM_ID_MAX - 1) as u8).is_ok());
    assert!(StreamHandle::new(INTEL_IPU4_STREAM_ID_MAX as u8).is_err());
}

#[test]
fn max_input_pins_then_overflow() {
    let mut b = StreamCfgData::builder();
    for _ in 0..INTEL_IPU4_MAX_IPINS {
        b = b.add_input_pin(dummy_input()).expect("within limit");
    }
    let err = b.add_input_pin(dummy_input()).unwrap_err();
    assert!(matches!(
        err,
        Error::TooMany {
            field: "input_pins",
            max,
            ..
        } if max == INTEL_IPU4_MAX_IPINS
    ));
}

#[test]
fn max_output_pins_then_overflow() {
    let mut b = StreamCfgData::builder();
    for _ in 0..INTEL_IPU4_MAX_OPINS {
        b = b.add_output_pin(dummy_output()).expect("within limit");
    }
    assert!(b.add_output_pin(dummy_output()).is_err());
}

#[test]
fn frame_buff_set_max_output_pins_then_overflow() {
    let mut b = FrameBuffSet::builder();
    for i in 0..INTEL_IPU4_MAX_OPINS as u64 {
        b = b
            .add_output_pin(OutputPinPayload::new(BufferId(i), CssVirtualAddress(0)))
            .expect("within limit");
    }
    assert!(b
        .add_output_pin(OutputPinPayload::new(BufferId(99), CssVirtualAddress(0)))
        .is_err());
}

#[test]
fn fully_populated_stream_cfg_serializes_to_exact_size() {
    let mut b = StreamCfgData::builder();
    for _ in 0..INTEL_IPU4_MAX_IPINS {
        b = b.add_input_pin(dummy_input()).unwrap();
    }
    for _ in 0..INTEL_IPU4_MAX_OPINS {
        b = b.add_output_pin(dummy_output()).unwrap();
    }
    let cfg = b.build().unwrap();
    let bytes = cfg.serialize();
    assert_eq!(bytes.len(), StreamCfgData::SERIALIZED_SIZE);
    assert_eq!(cfg.nof_input_pins(), INTEL_IPU4_MAX_IPINS as u8);
    assert_eq!(cfg.nof_output_pins(), INTEL_IPU4_MAX_OPINS as u8);
    assert_eq!(StreamCfgData::deserialize(&bytes).unwrap(), cfg);
}

#[test]
fn zero_pin_messages_roundtrip() {
    // Empty stream cfg (no pins) — all ABI slots zero-filled.
    let cfg = StreamCfgData::default();
    let bytes = cfg.serialize();
    assert_eq!(bytes.len(), StreamCfgData::SERIALIZED_SIZE);
    assert!(bytes.iter().all(|&b| b == 0));
    assert_eq!(StreamCfgData::deserialize(&bytes).unwrap(), cfg);

    // Empty frame buffer set. The ABI struct has no pin-count field — it always
    // carries INTEL_IPU4_MAX_OPINS payload slots — so a deserialized "empty"
    // set materializes all slots (zeroed). The bytes round-trip exactly.
    let fbs = FrameBuffSet::default();
    let fbytes = fbs.serialize();
    assert!(fbytes.iter().all(|&b| b == 0));
    let parsed = FrameBuffSet::deserialize(&fbytes).unwrap();
    assert_eq!(parsed.output_pins.len(), INTEL_IPU4_MAX_OPINS);
    assert_eq!(parsed.serialize(), fbytes);
}

#[test]
fn serialize_into_exact_and_too_small() {
    let tok = ProxySendQueueToken::new(1, 2, 3, 4);
    let mut exact = [0u8; ProxySendQueueToken::SERIALIZED_SIZE];
    assert_eq!(
        tok.serialize_into(&mut exact).unwrap(),
        ProxySendQueueToken::SERIALIZED_SIZE
    );

    let mut small = [0u8; ProxySendQueueToken::SERIALIZED_SIZE - 1];
    assert!(matches!(
        tok.serialize_into(&mut small),
        Err(Error::BufferTooSmall { .. })
    ));
}

#[test]
fn queue_routing_indices() {
    assert_eq!(proxy_queue_index(), ISYS_PROXY_INDEX);
    let s = StreamHandle::new(0).unwrap();
    assert_eq!(message_queue_index(s), ISYS_MSG_INDEX);
    let s7 = StreamHandle::new(7).unwrap();
    assert_eq!(message_queue_index(s7), 7 + ISYS_MSG_INDEX);
}
