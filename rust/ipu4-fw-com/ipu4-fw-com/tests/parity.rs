//! Golden parity tests: assert byte-level equivalence between the safe Rust
//! serialization and (a) a fixed golden reference and (b) the raw `#[repr(C)]`
//! FFI struct memory layout from `ipu4-fw-com-sys`.
//!
//! These guarantee the safe port produces firmware-identical bytes on the wire.

use ipu4_fw_com::{SysQueue, SyscomConfigFw};
use ipu4_fw_com_sys as sys;

/// Read the raw little-endian bytes of a `#[repr(C)]` value.
///
/// Used only in tests to compare the FFI struct's in-memory layout against the
/// safe serializer; the IPU4 is little-endian so this is the wire format.
fn struct_bytes<T: Copy>(value: &T) -> Vec<u8> {
    let ptr = value as *const T as *const u8;
    // SAFETY: `T` is a `Copy` `#[repr(C)]` POD struct of `size_of::<T>()` bytes.
    unsafe { core::slice::from_raw_parts(ptr, core::mem::size_of::<T>()) }.to_vec()
}

#[test]
fn sys_queue_matches_golden_bytes() {
    let q = SysQueue {
        host_address: 0x0011_2233_4455_6677,
        vied_address: 0x8899_AABB,
        size: 0x0000_0004,
        token_size: 0x0000_0020,
        wr_reg: 0x0000_0004,
        rd_reg: 0x0000_0005,
        align: 0,
    };

    let golden: [u8; 32] = [
        0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00, // host_address (u64 LE)
        0xBB, 0xAA, 0x99, 0x88, // vied_address
        0x04, 0x00, 0x00, 0x00, // size
        0x20, 0x00, 0x00, 0x00, // token_size
        0x04, 0x00, 0x00, 0x00, // wr_reg
        0x05, 0x00, 0x00, 0x00, // rd_reg
        0x00, 0x00, 0x00, 0x00, // _align
    ];

    assert_eq!(q.to_le_bytes(), golden, "serializer drifted from golden bytes");
}

#[test]
fn sys_queue_matches_ffi_struct_layout() {
    let q = SysQueue {
        host_address: 0xDEAD_BEEF_CAFE_F00D,
        vied_address: 0x1234_5678,
        size: 4,
        token_size: 32,
        wr_reg: 4,
        rd_reg: 5,
        align: 0,
    };

    let ffi = sys::sys_queue {
        host_address: q.host_address,
        vied_address: q.vied_address,
        size: q.size,
        token_size: q.token_size,
        wr_reg: q.wr_reg,
        rd_reg: q.rd_reg,
        _align: q.align,
    };

    assert_eq!(
        q.to_le_bytes().to_vec(),
        struct_bytes(&ffi),
        "safe serialization differs from #[repr(C)] FFI layout"
    );
    assert_eq!(SysQueue::SERIALIZED_LEN, core::mem::size_of::<sys::sys_queue>());
}

#[test]
fn config_fw_matches_golden_bytes() {
    let cfg = SyscomConfigFw {
        firmware_address: 0,
        num_input_queues: 2,
        num_output_queues: 3,
        input_queue: 0x20,
        output_queue: 0x60,
        specific_addr: 0xC0,
        specific_size: 16,
    };

    let golden: [u8; 28] = [
        0x00, 0x00, 0x00, 0x00, // firmware_address
        0x02, 0x00, 0x00, 0x00, // num_input_queues
        0x03, 0x00, 0x00, 0x00, // num_output_queues
        0x20, 0x00, 0x00, 0x00, // input_queue
        0x60, 0x00, 0x00, 0x00, // output_queue
        0xC0, 0x00, 0x00, 0x00, // specific_addr
        0x10, 0x00, 0x00, 0x00, // specific_size
    ];

    assert_eq!(cfg.to_le_bytes(), golden);
}

#[test]
fn config_fw_matches_ffi_struct_layout() {
    let cfg = SyscomConfigFw {
        firmware_address: 0xAABB_CCDD,
        num_input_queues: 2,
        num_output_queues: 3,
        input_queue: 0x20,
        output_queue: 0x60,
        specific_addr: 0xC0,
        specific_size: 16,
    };
    let ffi = sys::ia_css_syscom_config_fw {
        firmware_address: cfg.firmware_address,
        num_input_queues: cfg.num_input_queues,
        num_output_queues: cfg.num_output_queues,
        input_queue: cfg.input_queue,
        output_queue: cfg.output_queue,
        specific_addr: cfg.specific_addr,
        specific_size: cfg.specific_size,
    };
    assert_eq!(cfg.to_le_bytes().to_vec(), struct_bytes(&ffi));
}

#[test]
fn token_serialization_round_trips() {
    let payload = [0xDEu8, 0xAD, 0xBE, 0xEF];
    let bytes = ipu4_fw_com::serialize_token(&payload, 4).unwrap();
    assert_eq!(bytes, payload.to_vec());
    // Mismatched size is rejected rather than truncating/overrunning.
    assert!(ipu4_fw_com::serialize_token(&payload, 8).is_err());
}
