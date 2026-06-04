//! Byte-exact serialization of the structures shared with the firmware.
//!
//! The firmware and the host agree on the *in-memory byte layout* of
//! `struct sys_queue` and `struct ia_css_syscom_config_fw`. The C driver simply
//! writes these structs into DMA memory and lets the compiler's struct layout
//! define the wire format. Here we reproduce that layout explicitly with
//! little-endian field encoding (the IPU4 is little-endian), which lets us
//! golden-test byte-level parity against the C layout without any `transmute`
//! or `unsafe`.

use crate::error::{FwComError, Result};
use ipu4_fw_com_sys::SYSCOM_QPR_BASE_REG;

/// Safe mirror of `struct sys_queue`, with explicit serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SysQueue {
    /// Host-visible base address of the token buffer.
    pub host_address: u64,
    /// ISP-visible (vied) base address of the token buffer.
    pub vied_address: u32,
    /// Number of slots in the queue.
    pub size: u32,
    /// Bytes per token.
    pub token_size: u32,
    /// Dmem register index of the write pointer.
    pub wr_reg: u32,
    /// Dmem register index of the read pointer.
    pub rd_reg: u32,
    /// Padding field to match the C layout.
    pub align: u32,
}

impl SysQueue {
    /// Serialized length in bytes (matches `size_of::<sys_queue>()`).
    pub const SERIALIZED_LEN: usize = 32;

    /// Build a queue descriptor the same way `sys_queue_init()` does.
    pub fn init(
        dmem_index: u32,
        size: u32,
        token_size: u32,
        host_address: u64,
        vied_address: u32,
    ) -> Self {
        SysQueue {
            host_address,
            vied_address,
            size,
            token_size,
            // Port of:
            //   q->wr_reg = SYSCOM_QPR_BASE_REG + dmemindex * 2;
            //   q->rd_reg = SYSCOM_QPR_BASE_REG + 1 + dmemindex * 2;
            wr_reg: SYSCOM_QPR_BASE_REG + dmem_index * 2,
            rd_reg: SYSCOM_QPR_BASE_REG + 1 + dmem_index * 2,
            align: 0,
        }
    }

    /// Serialize to the exact 32-byte little-endian layout of `struct sys_queue`.
    pub fn to_le_bytes(&self) -> [u8; Self::SERIALIZED_LEN] {
        let mut out = [0u8; Self::SERIALIZED_LEN];
        out[0..8].copy_from_slice(&self.host_address.to_le_bytes());
        out[8..12].copy_from_slice(&self.vied_address.to_le_bytes());
        out[12..16].copy_from_slice(&self.size.to_le_bytes());
        out[16..20].copy_from_slice(&self.token_size.to_le_bytes());
        out[20..24].copy_from_slice(&self.wr_reg.to_le_bytes());
        out[24..28].copy_from_slice(&self.rd_reg.to_le_bytes());
        out[28..32].copy_from_slice(&self.align.to_le_bytes());
        out
    }
}

/// Safe mirror of `struct ia_css_syscom_config_fw`, with explicit serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SyscomConfigFw {
    /// Firmware load address.
    pub firmware_address: u32,
    /// Number of host->SP queues.
    pub num_input_queues: u32,
    /// Number of SP->host queues.
    pub num_output_queues: u32,
    /// ISP-visible address of the input `sys_queue` array.
    pub input_queue: u32,
    /// ISP-visible address of the output `sys_queue` array.
    pub output_queue: u32,
    /// ISP-visible address of the firmware-specific blob.
    pub specific_addr: u32,
    /// Size in bytes of the firmware-specific blob.
    pub specific_size: u32,
}

impl SyscomConfigFw {
    /// Serialized length in bytes (matches `size_of::<ia_css_syscom_config_fw>()`).
    pub const SERIALIZED_LEN: usize = 28;

    /// Serialize to the exact 28-byte little-endian layout.
    pub fn to_le_bytes(&self) -> [u8; Self::SERIALIZED_LEN] {
        let mut out = [0u8; Self::SERIALIZED_LEN];
        out[0..4].copy_from_slice(&self.firmware_address.to_le_bytes());
        out[4..8].copy_from_slice(&self.num_input_queues.to_le_bytes());
        out[8..12].copy_from_slice(&self.num_output_queues.to_le_bytes());
        out[12..16].copy_from_slice(&self.input_queue.to_le_bytes());
        out[16..20].copy_from_slice(&self.output_queue.to_le_bytes());
        out[20..24].copy_from_slice(&self.specific_addr.to_le_bytes());
        out[24..28].copy_from_slice(&self.specific_size.to_le_bytes());
        out
    }
}

/// Serialize an arbitrary token payload into a fixed-size queue slot.
///
/// The C driver `memcpy`s exactly `token_size` bytes into the slot. This helper
/// reproduces that, but rejects mismatched payloads instead of overrunning the
/// buffer (the kind of silent overflow the raw `memcpy` could not catch).
pub fn serialize_token(payload: &[u8], token_size: usize) -> Result<Vec<u8>> {
    if payload.len() != token_size {
        return Err(FwComError::TokenSizeMismatch {
            expected: token_size,
            actual: payload.len(),
        });
    }
    Ok(payload.to_vec())
}
