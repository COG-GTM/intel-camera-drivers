//! Raw, `unsafe` FFI bindings mirroring the Intel IPU4 firmware communication
//! (syscom) layer defined in:
//!
//! * `drivers/media/pci/intel-ipu4/intel-ipu4-fw-com.c`
//! * `drivers/media/pci/intel-ipu4/intel-ipu4-fw-com.h`
//!
//! These declarations exist to document and pin the original C ABI: the
//! `#[repr(C)]` structs reproduce the exact in-memory layout shared between the
//! host driver and the firmware, and the `extern "C"` block reproduces the
//! exported function signatures. They are intentionally a thin, dependency-free
//! mirror; all safe logic lives in the `ipu4-fw-com` crate.
//!
//! The structs flagged "shared structure" below MUST NOT change layout: they are
//! a contract between the CPU and the ISP firmware.
#![allow(non_camel_case_types)]

use core::ffi::c_void;

// ---------------------------------------------------------------------------
// Constants (verbatim from intel-ipu4-fw-com.c)
// ---------------------------------------------------------------------------

/// Program load or explicit host setting should init to this.
pub const SYSCOM_STATE_UNINIT: u32 = 0x57A7_E000;
/// SP Syscom sets this when it is ready for use.
pub const SYSCOM_STATE_READY: u32 = 0x57A7_E001;
/// SP Syscom sets this when no more syscom accesses will happen.
pub const SYSCOM_STATE_INACTIVE: u32 = 0x57A7_E002;

/// Program load or explicit host setting should init to this.
pub const SYSCOM_COMMAND_UNINIT: u32 = 0x57A7_F000;
/// Host Syscom requests syscom to become inactive.
pub const SYSCOM_COMMAND_INACTIVE: u32 = 0x57A7_F001;

/// Offset (bytes) of the write index within a queue's dmem register pair.
pub const FW_COM_WR_REG: u32 = 0;
/// Offset (bytes) of the read index within a queue's dmem register pair.
pub const FW_COM_RD_REG: u32 = 4;

pub const REGMEM_OFFSET: u32 = 0;

/// Pass pkg_dir address to SPC in non-secure mode.
pub const PKG_DIR_ADDR_REG: u32 = 0;
/// Pass syscom configuration to SPC.
pub const SYSCOM_CONFIG_REG: u32 = 1;
/// Syscom state - modified by SP.
pub const SYSCOM_STATE_REG: u32 = 2;
/// Syscom commands - modified by the host.
pub const SYSCOM_COMMAND_REG: u32 = 3;
/// First syscom queue pointer register.
pub const SYSCOM_QPR_BASE_REG: u32 = 4;

/// Direction of a token-queue access. Mirrors `enum message_direction`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum message_direction {
    DIR_RECV = 0,
    DIR_SEND = 1,
}

// ---------------------------------------------------------------------------
// Shared structures between driver and FW - do not modify layout
// ---------------------------------------------------------------------------

/// Shared structure between driver and FW (`struct sys_queue`).
///
/// Layout is part of the CPU<->ISP contract and must remain stable.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct sys_queue {
    pub host_address: u64,
    pub vied_address: u32,
    pub size: u32,
    pub token_size: u32,
    /// Dmem location for port access.
    pub wr_reg: u32,
    /// Dmem location for port access.
    pub rd_reg: u32,
    pub _align: u32,
}

/// Firmware config copied from host to SP via DDR (`struct ia_css_syscom_config_fw`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ia_css_syscom_config_fw {
    pub firmware_address: u32,
    pub num_input_queues: u32,
    pub num_output_queues: u32,
    /// ISP pointer to an array of `sys_queue` structures.
    pub input_queue: u32,
    /// ISP pointer to an array of `sys_queue` structures.
    pub output_queue: u32,
    /// ISYS / PSYS private data.
    pub specific_addr: u32,
    pub specific_size: u32,
}

// ---------------------------------------------------------------------------
// Public configuration structures (from intel-ipu4-fw-com.h)
// ---------------------------------------------------------------------------

/// Per-queue configuration (`struct ia_css_syscom_queue_config`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ia_css_syscom_queue_config {
    /// Tokens per queue.
    pub queue_size: u32,
    /// Bytes per token.
    pub token_size: u32,
}

/// Opaque forward declaration of `struct intel_ipu4_bus_device`.
#[repr(C)]
pub struct intel_ipu4_bus_device {
    _private: [u8; 0],
}

/// Opaque forward declaration of `struct intel_ipu4_fw_com_context`.
#[repr(C)]
pub struct intel_ipu4_fw_com_context {
    _private: [u8; 0],
}

/// Callback type: returns non-zero if the cell is ready.
pub type cell_ready_fn =
    Option<unsafe extern "C" fn(adev: *mut intel_ipu4_bus_device) -> core::ffi::c_int>;
/// Callback type: starts the cell.
pub type cell_start_fn = Option<unsafe extern "C" fn(adev: *mut intel_ipu4_bus_device)>;

/// Top-level prepare configuration (`struct intel_ipu4_fw_com_cfg`).
#[repr(C)]
pub struct intel_ipu4_fw_com_cfg {
    pub num_input_queues: u32,
    pub num_output_queues: u32,
    pub input: *mut ia_css_syscom_queue_config,
    pub output: *mut ia_css_syscom_queue_config,
    pub dmem_addr: u32,
    /// Firmware-specific configuration data.
    pub specific_addr: *mut c_void,
    pub specific_size: u32,
    pub cell_ready: cell_ready_fn,
    pub cell_start: cell_start_fn,
}

// ---------------------------------------------------------------------------
// Exported C ABI (verbatim signatures from intel-ipu4-fw-com.h)
// ---------------------------------------------------------------------------

extern "C" {
    pub fn intel_ipu4_fw_com_prepare(
        cfg: *mut intel_ipu4_fw_com_cfg,
        adev: *mut intel_ipu4_bus_device,
        base: *mut c_void,
    ) -> *mut c_void;

    pub fn intel_ipu4_fw_com_open(ctx: *mut intel_ipu4_fw_com_context) -> core::ffi::c_int;
    pub fn intel_ipu4_fw_com_ready(ctx: *mut intel_ipu4_fw_com_context) -> core::ffi::c_int;
    pub fn intel_ipu4_fw_com_close(ctx: *mut intel_ipu4_fw_com_context) -> core::ffi::c_int;
    pub fn intel_ipu4_fw_com_release(
        ctx: *mut intel_ipu4_fw_com_context,
        force: u32,
    ) -> core::ffi::c_int;

    pub fn intel_ipu4_recv_get_token(
        ctx: *mut intel_ipu4_fw_com_context,
        q_nbr: core::ffi::c_int,
    ) -> *mut c_void;
    pub fn intel_ipu4_recv_put_token(
        ctx: *mut intel_ipu4_fw_com_context,
        q_nbr: core::ffi::c_int,
    );
    pub fn intel_ipu4_send_get_token(
        ctx: *mut intel_ipu4_fw_com_context,
        q_nbr: core::ffi::c_int,
    ) -> *mut c_void;
    pub fn intel_ipu4_send_put_token(
        ctx: *mut intel_ipu4_fw_com_context,
        q_nbr: core::ffi::c_int,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    #[test]
    fn sys_queue_layout_is_stable() {
        // u64 + 6 * u32 = 8 + 24 = 32 bytes, 8-byte aligned, no padding.
        assert_eq!(size_of::<sys_queue>(), 32);
        assert_eq!(align_of::<sys_queue>(), 8);
    }

    #[test]
    fn config_fw_layout_is_stable() {
        // 7 * u32 = 28 bytes.
        assert_eq!(size_of::<ia_css_syscom_config_fw>(), 28);
        assert_eq!(align_of::<ia_css_syscom_config_fw>(), 4);
    }

    #[test]
    fn queue_config_layout_is_stable() {
        assert_eq!(size_of::<ia_css_syscom_queue_config>(), 8);
    }
}
