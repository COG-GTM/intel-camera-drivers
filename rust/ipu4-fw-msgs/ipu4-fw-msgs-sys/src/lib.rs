//! Raw `unsafe extern "C"` FFI bindings that faithfully mirror the IPU4 ISYS
//! firmware message ABI as defined in the original C driver:
//!
//! * `drivers/media/pci/intel-ipu4/intel-ipu4-isys-fw-msgs.h`
//! * `drivers/media/pci/intel-ipu4/intel-ipu4-isysapi-fw-types.h`
//! * `drivers/media/pci/intel-ipu4/intel-ipu4-fw-com.h`
//!
//! These declarations exist so that the safe `ipu4-fw-msgs` crate can be
//! validated for byte-exact ABI parity with the firmware structs. Every struct
//! is `#[repr(C)]` and every field mirrors the C declaration (name, type and
//! order) so that the in-memory layout is identical to the kernel driver on a
//! standard LP64 little-endian target.
//!
//! Nothing in this crate is safe to call on its own: the `extern "C"` functions
//! are the firmware-communication entry points implemented elsewhere in the
//! kernel driver and are declared here purely to document the ABI surface.

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use core::ffi::{c_int, c_uint, c_void};

// ---------------------------------------------------------------------------
// Compile-time ABI constants (intel-ipu4-isysapi-fw-types.h)
// ---------------------------------------------------------------------------

/// Max number of input pins (`INTEL_IPU4_MAX_IPINS`).
pub const INTEL_IPU4_MAX_IPINS: usize = 4;
/// Max number of output pins (`INTEL_IPU4_MAX_OPINS` = `MAX_IPINS` + 2).
pub const INTEL_IPU4_MAX_OPINS: usize = INTEL_IPU4_MAX_IPINS + 2;
/// Max number of supported virtual streams (`INTEL_IPU4_STREAM_ID_MAX`).
pub const INTEL_IPU4_STREAM_ID_MAX: usize = 8;
/// Max number of SRAM buffer partitions (`INTEL_IPU4_NOF_SRAM_BLOCKS_MAX`).
pub const INTEL_IPU4_NOF_SRAM_BLOCKS_MAX: usize = INTEL_IPU4_STREAM_ID_MAX;
/// Max number of input pins routed in ISL (`INTEL_IPU4_MAX_IPINS_IN_ISL`).
pub const INTEL_IPU4_MAX_IPINS_IN_ISL: usize = 2;
/// Max number of planes per frame format (`INTEL_IPU4_PIN_PLANES_MAX`).
pub const INTEL_IPU4_PIN_PLANES_MAX: usize = 4;

/// Number of resolution-info entries (`N_IPU_FW_ISYS_RESOLUTION_INFO`).
pub const N_IPU_FW_ISYS_RESOLUTION_INFO: usize = 2;
/// Number of cropping locations (`N_IPU_FW_ISYS_CROPPING_LOCATION`).
pub const N_IPU_FW_ISYS_CROPPING_LOCATION: usize = 4;
/// Number of MIPI data types (`N_IPU_FW_ISYS_MIPI_DATA_TYPE`).
pub const N_IPU_FW_ISYS_MIPI_DATA_TYPE: usize = 0x40;

// fw-msgs.h queue indexing constants
pub const ISYS_FW_NBR_QUEUES: usize = 2;
pub const ISYS_NBR_PROXY_QUEUES: usize = 1;
pub const ISYS_PROXY_INDEX: usize = 0;
pub const ISYS_MSG_INDEX: usize = 1;

// ---------------------------------------------------------------------------
// Enums (mirrored as C `int`-sized via repr(C))
// ---------------------------------------------------------------------------

/// `enum ipu_fw_isys_resp_type`
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ipu_fw_isys_resp_type {
    IPU_FW_ISYS_RESP_TYPE_STREAM_OPEN_DONE = 0,
    IPU_FW_ISYS_RESP_TYPE_STREAM_START_ACK,
    IPU_FW_ISYS_RESP_TYPE_STREAM_START_AND_CAPTURE_ACK,
    IPU_FW_ISYS_RESP_TYPE_STREAM_CAPTURE_ACK,
    IPU_FW_ISYS_RESP_TYPE_STREAM_STOP_ACK,
    IPU_FW_ISYS_RESP_TYPE_STREAM_FLUSH_ACK,
    IPU_FW_ISYS_RESP_TYPE_STREAM_CLOSE_ACK,
    IPU_FW_ISYS_RESP_TYPE_PIN_DATA_READY,
    IPU_FW_ISYS_RESP_TYPE_PIN_DATA_WATERMARK,
    IPU_FW_ISYS_RESP_TYPE_FRAME_SOF,
    IPU_FW_ISYS_RESP_TYPE_FRAME_EOF,
    IPU_FW_ISYS_RESP_TYPE_STREAM_START_AND_CAPTURE_DONE,
    IPU_FW_ISYS_RESP_TYPE_STREAM_CAPTURE_DONE,
    IPU_FW_ISYS_RESP_TYPE_PIN_DATA_SKIPPED,
    IPU_FW_ISYS_RESP_TYPE_STREAM_CAPTURE_SKIPPED,
    IPU_FW_ISYS_RESP_TYPE_FRAME_SOF_DISCARDED,
    IPU_FW_ISYS_RESP_TYPE_FRAME_EOF_DISCARDED,
    IPU_FW_ISYS_RESP_TYPE_STATS_DATA_READY,
    N_IPU_FW_ISYS_RESP_TYPE,
}

/// `enum ipu_fw_isys_send_type`
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ipu_fw_isys_send_type {
    IPU_FW_ISYS_SEND_TYPE_STREAM_OPEN = 0,
    IPU_FW_ISYS_SEND_TYPE_STREAM_START,
    IPU_FW_ISYS_SEND_TYPE_STREAM_START_AND_CAPTURE,
    IPU_FW_ISYS_SEND_TYPE_STREAM_CAPTURE,
    IPU_FW_ISYS_SEND_TYPE_STREAM_STOP,
    IPU_FW_ISYS_SEND_TYPE_STREAM_FLUSH,
    IPU_FW_ISYS_SEND_TYPE_STREAM_CLOSE,
    N_IPU_FW_ISYS_SEND_TYPE,
}

/// `enum ipu_fw_isys_isl_use`
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ipu_fw_isys_isl_use {
    IPU_FW_ISYS_USE_NO_ISL_NO_ISA = 0,
    IPU_FW_ISYS_USE_SINGLE_DUAL_ISL,
    IPU_FW_ISYS_USE_SINGLE_ISA,
    N_IPU_FW_ISYS_USE,
}

/// `enum ipu_fw_isys_error`
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ipu_fw_isys_error {
    IPU_FW_ISYS_ERROR_NONE = 0,
    IPU_FW_ISYS_ERROR_FW_INTERNAL_CONSISTENCY,
    IPU_FW_ISYS_ERROR_HW_CONSISTENCY,
    IPU_FW_ISYS_ERROR_DRIVER_INVALID_COMMAND_SEQUENCE,
    IPU_FW_ISYS_ERROR_DRIVER_INVALID_DEVICE_CONFIGURATION,
    IPU_FW_ISYS_ERROR_DRIVER_INVALID_STREAM_CONFIGURATION,
    IPU_FW_ISYS_ERROR_DRIVER_INVALID_FRAME_CONFIGURATION,
    IPU_FW_ISYS_ERROR_INSUFFICIENT_RESOURCES,
    IPU_FW_ISYS_ERROR_HW_REPORTED_STR2MMIO,
    IPU_FW_ISYS_ERROR_HW_REPORTED_SIG2CIO,
    IPU_FW_ISYS_ERROR_SENSOR_FW_SYNC,
    IPU_FW_ISYS_ERROR_STREAM_IN_SUSPENSION,
    IPU_FW_ISYS_ERROR_RESPONSE_QUEUE_FULL,
    N_IPU_FW_ISYS_ERROR,
}

/// `enum ipu_fw_proxy_error`
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ipu_fw_proxy_error {
    IPU_FW_PROXY_ERROR_NONE = 0,
    IPU_FW_PROXY_ERROR_INVALID_WRITE_REGION,
    IPU_FW_PROXY_ERROR_INVALID_WRITE_OFFSET,
    N_IPU_FW_PROXY_ERROR,
}

// ---------------------------------------------------------------------------
// Message structs (intel-ipu4-isys-fw-msgs.h)
// ---------------------------------------------------------------------------

/// `struct ipu_fw_isys_buffer_partition_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_buffer_partition_abi {
    pub num_gda_pages: [u32; INTEL_IPU4_STREAM_ID_MAX],
}

/// `struct ipu_fw_isys_fw_config`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_fw_config {
    pub buffer_partition: ipu_fw_isys_buffer_partition_abi,
    pub num_send_queues: [u32; ISYS_FW_NBR_QUEUES],
    pub num_recv_queues: [u32; ISYS_FW_NBR_QUEUES],
}

/// `struct ipu_fw_isys_resolution_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_resolution_abi {
    pub width: u32,
    pub height: u32,
}

/// `struct ipu_fw_isys_output_pin_payload_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_output_pin_payload_abi {
    pub out_buf_id: u64,
    pub addr: u32,
}

/// `struct ipu_fw_isys_output_pin_info_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_output_pin_info_abi {
    pub output_res: ipu_fw_isys_resolution_abi,
    pub stride: u32,
    pub watermark_in_lines: u32,
    pub send_irq: u8,
    pub input_pin_id: u8,
    pub pt: u8,
    pub ft: u8,
    pub online: u8,
}

/// `struct ipu_fw_isys_param_pin_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_param_pin_abi {
    pub param_buf_id: u64,
    pub addr: u32,
}

/// `struct ipu_fw_isys_input_pin_info_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_input_pin_info_abi {
    pub input_res: ipu_fw_isys_resolution_abi,
    pub dt: u8,
    pub mipi_store_mode: u8,
    pub bits_per_pix: u8,
}

/// Bitfield block of `struct ipu_fw_isys_isa_cfg_abi`.
///
/// In C this is an anonymous bitfield struct packed into a single 32-bit unit.
/// We mirror it as a raw `u32` whose bit assignment is documented here and
/// implemented in the safe crate. Bit positions (LSB first, System V LP64):
/// `blc`:0, `lsc`:1, `dpc`:2, `downscaler`:3, `awb`:4, `af`:5, `ae`:6,
/// `paf`:7..15 (8 bits), `send_irq_stats_ready`:15, `send_resp_stats_ready`:16.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_isa_cfg_abi {
    pub isa_res: [ipu_fw_isys_resolution_abi; N_IPU_FW_ISYS_RESOLUTION_INFO],
    pub cfg: u32,
}

/// `struct ipu_fw_isys_cropping_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_cropping_abi {
    pub top_offset: i32,
    pub left_offset: i32,
    pub bottom_offset: i32,
    pub right_offset: i32,
}

/// `struct ipu_fw_isys_stream_cfg_data_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_stream_cfg_data_abi {
    pub isa_cfg: ipu_fw_isys_isa_cfg_abi,
    pub crop: [ipu_fw_isys_cropping_abi; N_IPU_FW_ISYS_CROPPING_LOCATION],
    pub input_pins: [ipu_fw_isys_input_pin_info_abi; INTEL_IPU4_MAX_IPINS],
    pub output_pins: [ipu_fw_isys_output_pin_info_abi; INTEL_IPU4_MAX_OPINS],
    pub compfmt: u32,
    pub nof_input_pins: u8,
    pub nof_output_pins: u8,
    pub send_irq_sof_discarded: u8,
    pub send_irq_eof_discarded: u8,
    pub send_resp_sof_discarded: u8,
    pub send_resp_eof_discarded: u8,
    pub src: u8,
    pub vc: u8,
    pub isl_use: u8,
}

/// `struct ipu_fw_isys_frame_buff_set_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_frame_buff_set_abi {
    pub output_pins: [ipu_fw_isys_output_pin_payload_abi; INTEL_IPU4_MAX_OPINS],
    pub process_group_light: ipu_fw_isys_param_pin_abi,
    pub send_irq_sof: u8,
    pub send_irq_eof: u8,
    pub send_resp_sof: u8,
    pub send_resp_eof: u8,
}

/// `struct ipu_fw_isys_error_info_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_error_info_abi {
    pub error: ipu_fw_isys_error,
    pub error_details: u32,
}

/// `struct ipu_fw_isys_resp_info_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_resp_info_abi {
    pub buf_id: u64,
    pub pin: ipu_fw_isys_output_pin_payload_abi,
    pub process_group_light: ipu_fw_isys_param_pin_abi,
    pub error_info: ipu_fw_isys_error_info_abi,
    pub timestamp: [u32; 2],
    pub stream_handle: u8,
    pub type_: u8,
    pub pin_id: u8,
    pub acc_id: u8,
}

/// `struct ipu_fw_isys_proxy_error_info_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_proxy_error_info_abi {
    pub error: ipu_fw_proxy_error,
    pub error_details: u32,
}

/// `struct ipu_fw_isys_proxy_resp_info_abi`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_isys_proxy_resp_info_abi {
    pub request_id: u32,
    pub error_info: ipu_fw_isys_proxy_error_info_abi,
}

/// `struct ipu_fw_proxy_write_queue_token`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_proxy_write_queue_token {
    pub request_id: u32,
    pub region_index: u32,
    pub offset: u32,
    pub value: u32,
}

/// `struct ipu_fw_resp_queue_token`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_resp_queue_token {
    pub resp_info: ipu_fw_isys_resp_info_abi,
}

/// `struct ipu_fw_send_queue_token`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_send_queue_token {
    pub buf_handle: u64,
    pub payload: u32,
    pub send_type: ipu_fw_isys_send_type,
}

/// `struct ipu_fw_proxy_resp_queue_token`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_proxy_resp_queue_token {
    pub proxy_resp_info: ipu_fw_isys_proxy_resp_info_abi,
}

/// `struct ipu_fw_proxy_send_queue_token`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ipu_fw_proxy_send_queue_token {
    pub request_id: u32,
    pub region_index: u32,
    pub offset: u32,
    pub value: u32,
}

// ---------------------------------------------------------------------------
// fw-com.h support types
// ---------------------------------------------------------------------------

/// `struct ia_css_syscom_queue_config`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ia_css_syscom_queue_config {
    pub queue_size: c_uint,
    pub token_size: c_uint,
}

/// Opaque forward declarations matching the kernel types.
#[repr(C)]
pub struct intel_ipu4_fw_com_context {
    _private: [u8; 0],
}

#[repr(C)]
pub struct intel_ipu4_bus_device {
    _private: [u8; 0],
}

#[repr(C)]
pub struct intel_ipu4_isys {
    _private: [u8; 0],
}

#[repr(C)]
pub struct device {
    _private: [u8; 0],
}

// ---------------------------------------------------------------------------
// Function signatures from the C ABI.
//
// These are the firmware-communication entry points that the original
// `intel-ipu4-isys-fw-msgs.c` calls into / exposes. They are declared here to
// document the ABI surface; the safe crate does not call them (they are
// provided by the kernel driver at link time).
// ---------------------------------------------------------------------------

extern "C" {
    pub fn intel_ipu4_isys_set_fw_params(stream_cfg: *mut ipu_fw_isys_stream_cfg_data_abi);

    pub fn fw_dump_isys_stream_cfg(
        dev: *mut device,
        stream_cfg: *mut ipu_fw_isys_stream_cfg_data_abi,
    );

    pub fn fw_dump_isys_frame_buff_set(
        dev: *mut device,
        buf: *mut ipu_fw_isys_frame_buff_set_abi,
        outputs: c_uint,
    );

    pub fn intel_ipu4_abi_init(isys: *mut intel_ipu4_isys);

    pub fn intel_ipu4_recv_get_token(
        ctx: *mut intel_ipu4_fw_com_context,
        q_nbr: c_int,
    ) -> *mut c_void;
    pub fn intel_ipu4_recv_put_token(ctx: *mut intel_ipu4_fw_com_context, q_nbr: c_int);
    pub fn intel_ipu4_send_get_token(
        ctx: *mut intel_ipu4_fw_com_context,
        q_nbr: c_int,
    ) -> *mut c_void;
    pub fn intel_ipu4_send_put_token(ctx: *mut intel_ipu4_fw_com_context, q_nbr: c_int);
}

#[cfg(test)]
mod abi_size_tests {
    use super::*;
    use core::mem::size_of;

    /// The `#[repr(C)]` layouts must match the kernel ABI `sizeof` values
    /// (verified against the reference C compiler via the golden generator).
    #[test]
    fn struct_sizes_match_c_abi() {
        assert_eq!(size_of::<ipu_fw_isys_resolution_abi>(), 8);
        assert_eq!(size_of::<ipu_fw_isys_output_pin_payload_abi>(), 16);
        assert_eq!(size_of::<ipu_fw_isys_output_pin_info_abi>(), 24);
        assert_eq!(size_of::<ipu_fw_isys_param_pin_abi>(), 16);
        assert_eq!(size_of::<ipu_fw_isys_input_pin_info_abi>(), 12);
        assert_eq!(size_of::<ipu_fw_isys_isa_cfg_abi>(), 20);
        assert_eq!(size_of::<ipu_fw_isys_cropping_abi>(), 16);
        assert_eq!(size_of::<ipu_fw_isys_stream_cfg_data_abi>(), 292);
        assert_eq!(size_of::<ipu_fw_isys_frame_buff_set_abi>(), 120);
        assert_eq!(size_of::<ipu_fw_isys_error_info_abi>(), 8);
        assert_eq!(size_of::<ipu_fw_isys_resp_info_abi>(), 64);
        assert_eq!(size_of::<ipu_fw_isys_proxy_error_info_abi>(), 8);
        assert_eq!(size_of::<ipu_fw_isys_proxy_resp_info_abi>(), 12);
        assert_eq!(size_of::<ipu_fw_send_queue_token>(), 16);
        assert_eq!(size_of::<ipu_fw_proxy_send_queue_token>(), 16);
    }
}
