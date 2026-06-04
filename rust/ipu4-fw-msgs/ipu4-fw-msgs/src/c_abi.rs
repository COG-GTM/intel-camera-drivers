//! C ABI surface (`--features c-abi`).
//!
//! These `#[no_mangle] extern "C"` functions expose the safe marshaling
//! implementation back through the original firmware ABI so the C driver can
//! call into the Rust port without changes to its data structures. They take
//! the raw `#[repr(C)]` structs from [`ipu4_fw_msgs_sys`] and plain byte
//! buffers, validate everything, and delegate to the safe code in
//! [`crate::messages`].
//!
//! Integer return convention for the serialize/parse helpers:
//! * `>= 0` — number of bytes written / consumed.
//! * `RET_ERR_NULL` (-1) — a required pointer was null or a value was invalid.
//! * `RET_ERR_BUFFER` (-2) — the supplied buffer was too small.

use core::slice;

use ipu4_fw_msgs_sys as sys;

use crate::enums::{IslUse, IsysError, SendType};
use crate::error::Error;
use crate::ids::{BufferId, CssVirtualAddress};
use crate::layout::N_IPU_FW_ISYS_RESOLUTION_INFO;
use crate::messages::{
    Cropping, FrameBuffSet, InputPinInfo, IsaConfig, Marshal, OutputPinInfo, OutputPinPayload,
    ParamPin, ProxySendQueueToken, Resolution, SendQueueToken, StreamCfgData,
};
use crate::tables::bits_per_pixel;

/// Null pointer or invalid value.
pub const RET_ERR_NULL: isize = -1;
/// Destination/source buffer too small.
pub const RET_ERR_BUFFER: isize = -2;

fn err_to_ret(e: Error) -> isize {
    match e {
        Error::BufferTooSmall { .. } => RET_ERR_BUFFER,
        _ => RET_ERR_NULL,
    }
}

// --- sys -> safe conversions ------------------------------------------------

impl From<&sys::ipu_fw_isys_resolution_abi> for Resolution {
    fn from(r: &sys::ipu_fw_isys_resolution_abi) -> Self {
        Resolution::new(r.width, r.height)
    }
}

impl From<&sys::ipu_fw_isys_input_pin_info_abi> for InputPinInfo {
    fn from(p: &sys::ipu_fw_isys_input_pin_info_abi) -> Self {
        InputPinInfo {
            input_res: (&p.input_res).into(),
            dt: p.dt,
            mipi_store_mode: p.mipi_store_mode,
            bits_per_pix: p.bits_per_pix,
        }
    }
}

impl From<&sys::ipu_fw_isys_output_pin_info_abi> for OutputPinInfo {
    fn from(p: &sys::ipu_fw_isys_output_pin_info_abi) -> Self {
        OutputPinInfo {
            output_res: (&p.output_res).into(),
            stride: p.stride,
            watermark_in_lines: p.watermark_in_lines,
            send_irq: p.send_irq,
            input_pin_id: p.input_pin_id,
            pt: p.pt,
            ft: p.ft,
            online: p.online,
        }
    }
}

impl From<&sys::ipu_fw_isys_cropping_abi> for Cropping {
    fn from(c: &sys::ipu_fw_isys_cropping_abi) -> Self {
        Cropping {
            top_offset: c.top_offset,
            left_offset: c.left_offset,
            bottom_offset: c.bottom_offset,
            right_offset: c.right_offset,
        }
    }
}

impl From<&sys::ipu_fw_isys_output_pin_payload_abi> for OutputPinPayload {
    fn from(p: &sys::ipu_fw_isys_output_pin_payload_abi) -> Self {
        OutputPinPayload::new(BufferId(p.out_buf_id), CssVirtualAddress(p.addr))
    }
}

impl From<&sys::ipu_fw_isys_param_pin_abi> for ParamPin {
    fn from(p: &sys::ipu_fw_isys_param_pin_abi) -> Self {
        ParamPin::new(BufferId(p.param_buf_id), CssVirtualAddress(p.addr))
    }
}

fn isa_cfg_from_sys(c: &sys::ipu_fw_isys_isa_cfg_abi) -> IsaConfig {
    let mut isa_res = [Resolution::default(); N_IPU_FW_ISYS_RESOLUTION_INFO];
    for (dst, src) in isa_res.iter_mut().zip(c.isa_res.iter()) {
        *dst = src.into();
    }
    IsaConfig::unpack_cfg(isa_res, c.cfg)
}

fn stream_cfg_from_sys(
    c: &sys::ipu_fw_isys_stream_cfg_data_abi,
) -> core::result::Result<StreamCfgData, Error> {
    let nof_in = c.nof_input_pins as usize;
    let nof_out = c.nof_output_pins as usize;
    if nof_in > sys::INTEL_IPU4_MAX_IPINS {
        return Err(Error::TooMany {
            field: "nof_input_pins",
            value: nof_in,
            max: sys::INTEL_IPU4_MAX_IPINS,
        });
    }
    if nof_out > sys::INTEL_IPU4_MAX_OPINS {
        return Err(Error::TooMany {
            field: "nof_output_pins",
            value: nof_out,
            max: sys::INTEL_IPU4_MAX_OPINS,
        });
    }
    let mut crop = [Cropping::default(); crate::layout::N_IPU_FW_ISYS_CROPPING_LOCATION];
    for (dst, src) in crop.iter_mut().zip(c.crop.iter()) {
        *dst = src.into();
    }
    Ok(StreamCfgData {
        isa_cfg: isa_cfg_from_sys(&c.isa_cfg),
        crop,
        input_pins: c.input_pins[..nof_in].iter().map(Into::into).collect(),
        output_pins: c.output_pins[..nof_out].iter().map(Into::into).collect(),
        compfmt: c.compfmt,
        send_irq_sof_discarded: c.send_irq_sof_discarded != 0,
        send_irq_eof_discarded: c.send_irq_eof_discarded != 0,
        send_resp_sof_discarded: c.send_resp_sof_discarded != 0,
        send_resp_eof_discarded: c.send_resp_eof_discarded != 0,
        src: c.src,
        vc: c.vc,
        isl_use: IslUse::from_u32(c.isl_use as u32)?,
    })
}

fn isys_error_to_sys(e: IsysError) -> sys::ipu_fw_isys_error {
    use sys::ipu_fw_isys_error as S;
    match e {
        IsysError::None => S::IPU_FW_ISYS_ERROR_NONE,
        IsysError::FwInternalConsistency => S::IPU_FW_ISYS_ERROR_FW_INTERNAL_CONSISTENCY,
        IsysError::HwConsistency => S::IPU_FW_ISYS_ERROR_HW_CONSISTENCY,
        IsysError::DriverInvalidCommandSequence => {
            S::IPU_FW_ISYS_ERROR_DRIVER_INVALID_COMMAND_SEQUENCE
        }
        IsysError::DriverInvalidDeviceConfiguration => {
            S::IPU_FW_ISYS_ERROR_DRIVER_INVALID_DEVICE_CONFIGURATION
        }
        IsysError::DriverInvalidStreamConfiguration => {
            S::IPU_FW_ISYS_ERROR_DRIVER_INVALID_STREAM_CONFIGURATION
        }
        IsysError::DriverInvalidFrameConfiguration => {
            S::IPU_FW_ISYS_ERROR_DRIVER_INVALID_FRAME_CONFIGURATION
        }
        IsysError::InsufficientResources => S::IPU_FW_ISYS_ERROR_INSUFFICIENT_RESOURCES,
        IsysError::HwReportedStr2mmio => S::IPU_FW_ISYS_ERROR_HW_REPORTED_STR2MMIO,
        IsysError::HwReportedSig2cio => S::IPU_FW_ISYS_ERROR_HW_REPORTED_SIG2CIO,
        IsysError::SensorFwSync => S::IPU_FW_ISYS_ERROR_SENSOR_FW_SYNC,
        IsysError::StreamInSuspension => S::IPU_FW_ISYS_ERROR_STREAM_IN_SUSPENSION,
        IsysError::ResponseQueueFull => S::IPU_FW_ISYS_ERROR_RESPONSE_QUEUE_FULL,
    }
}

fn frame_buff_set_from_sys(c: &sys::ipu_fw_isys_frame_buff_set_abi) -> FrameBuffSet {
    FrameBuffSet {
        output_pins: c.output_pins.iter().map(Into::into).collect(),
        process_group_light: (&c.process_group_light).into(),
        send_irq_sof: c.send_irq_sof != 0,
        send_irq_eof: c.send_irq_eof != 0,
        send_resp_sof: c.send_resp_sof != 0,
        send_resp_eof: c.send_resp_eof != 0,
    }
}

// --- exported functions -----------------------------------------------------

/// Drop-in replacement for `intel_ipu4_isys_set_fw_params`: fill each valid
/// input pin's `bits_per_pix` from the MIPI data-type table.
///
/// # Safety
/// `stream_cfg` must be a valid, non-null pointer to an initialized
/// `ipu_fw_isys_stream_cfg_data_abi`.
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_isys_set_fw_params(
    stream_cfg: *mut sys::ipu_fw_isys_stream_cfg_data_abi,
) {
    if stream_cfg.is_null() {
        return;
    }
    // SAFETY: caller guarantees a valid, initialized struct pointer.
    let cfg = unsafe { &mut *stream_cfg };
    let nof = (cfg.nof_input_pins as usize).min(sys::INTEL_IPU4_MAX_IPINS);
    for pin in &mut cfg.input_pins[..nof] {
        pin.bits_per_pix = bits_per_pixel(pin.dt) as u8;
    }
}

/// Serialize a stream-cfg struct into `out`, returning bytes written.
///
/// # Safety
/// `cfg` must point to a valid struct; `out` must point to at least `out_len`
/// writable bytes (or be null with `out_len == 0`).
#[no_mangle]
pub unsafe extern "C" fn ipu4_fw_msgs_stream_cfg_serialize(
    cfg: *const sys::ipu_fw_isys_stream_cfg_data_abi,
    out: *mut u8,
    out_len: usize,
) -> isize {
    if cfg.is_null() || out.is_null() {
        return RET_ERR_NULL;
    }
    // SAFETY: caller guarantees validity of the pointers and length.
    let cfg = unsafe { &*cfg };
    let buf = unsafe { slice::from_raw_parts_mut(out, out_len) };
    let safe = match stream_cfg_from_sys(cfg) {
        Ok(s) => s,
        Err(e) => return err_to_ret(e),
    };
    match safe.serialize_into(buf) {
        Ok(n) => n as isize,
        Err(e) => err_to_ret(e),
    }
}

/// Serialize a frame-buffer-set struct into `out`, returning bytes written.
///
/// # Safety
/// See [`ipu4_fw_msgs_stream_cfg_serialize`].
#[no_mangle]
pub unsafe extern "C" fn ipu4_fw_msgs_frame_buff_set_serialize(
    fbs: *const sys::ipu_fw_isys_frame_buff_set_abi,
    out: *mut u8,
    out_len: usize,
) -> isize {
    if fbs.is_null() || out.is_null() {
        return RET_ERR_NULL;
    }
    // SAFETY: caller guarantees validity of the pointers and length.
    let fbs = unsafe { &*fbs };
    let buf = unsafe { slice::from_raw_parts_mut(out, out_len) };
    let safe = frame_buff_set_from_sys(fbs);
    match safe.serialize_into(buf) {
        Ok(n) => n as isize,
        Err(e) => err_to_ret(e),
    }
}

/// Build and serialize a send-queue command token, returning bytes written.
///
/// Mirrors `intel_ipu4_isys_abi_complex_cmd`: a payload of zero / null handle
/// yields the "simple" command form. `send_type` must be a valid
/// `enum ipu_fw_isys_send_type` value.
///
/// # Safety
/// `out` must point to at least `out_len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ipu4_fw_msgs_build_send_token(
    buf_handle: u64,
    payload: u32,
    send_type: u32,
    out: *mut u8,
    out_len: usize,
) -> isize {
    if out.is_null() {
        return RET_ERR_NULL;
    }
    let send_type = match SendType::from_u32(send_type) {
        Ok(t) => t,
        Err(e) => return err_to_ret(e),
    };
    let token =
        SendQueueToken::complex(BufferId(buf_handle), CssVirtualAddress(payload), send_type);
    // SAFETY: caller guarantees validity of out/out_len.
    let buf = unsafe { slice::from_raw_parts_mut(out, out_len) };
    match token.serialize_into(buf) {
        Ok(n) => n as isize,
        Err(e) => err_to_ret(e),
    }
}

/// Build and serialize a proxy send-queue token, returning bytes written.
///
/// Mirrors `intel_ipu4_isys_send_proxy_token`'s token construction.
///
/// # Safety
/// `out` must point to at least `out_len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ipu4_fw_msgs_build_proxy_send_token(
    request_id: u32,
    region_index: u32,
    offset: u32,
    value: u32,
    out: *mut u8,
    out_len: usize,
) -> isize {
    if out.is_null() {
        return RET_ERR_NULL;
    }
    let token = ProxySendQueueToken::new(request_id, region_index, offset, value);
    // SAFETY: caller guarantees validity of out/out_len.
    let buf = unsafe { slice::from_raw_parts_mut(out, out_len) };
    match token.serialize_into(buf) {
        Ok(n) => n as isize,
        Err(e) => err_to_ret(e),
    }
}

/// Parse a response-info message from `buf` into the C struct `out`.
///
/// Returns the number of bytes consumed, or a negative error code.
///
/// # Safety
/// `buf` must point to at least `len` readable bytes; `out` must point to a
/// writable `ipu_fw_isys_resp_info_abi`.
#[no_mangle]
pub unsafe extern "C" fn ipu4_fw_msgs_parse_resp_info(
    buf: *const u8,
    len: usize,
    out: *mut sys::ipu_fw_isys_resp_info_abi,
) -> isize {
    if buf.is_null() || out.is_null() {
        return RET_ERR_NULL;
    }
    // SAFETY: caller guarantees validity of the pointers and length.
    let bytes = unsafe { slice::from_raw_parts(buf, len) };
    let resp = match crate::messages::RespInfo::deserialize(bytes) {
        Ok(r) => r,
        Err(e) => return err_to_ret(e),
    };
    let dst = unsafe { &mut *out };
    dst.buf_id = resp.buf_id.get();
    dst.pin = sys::ipu_fw_isys_output_pin_payload_abi {
        out_buf_id: resp.pin.out_buf_id.get(),
        addr: resp.pin.addr.get(),
    };
    dst.process_group_light = sys::ipu_fw_isys_param_pin_abi {
        param_buf_id: resp.process_group_light.param_buf_id.get(),
        addr: resp.process_group_light.addr.get(),
    };
    dst.error_info = sys::ipu_fw_isys_error_info_abi {
        error: isys_error_to_sys(resp.error_info.error),
        error_details: resp.error_info.error_details,
    };
    dst.timestamp = resp.timestamp;
    dst.stream_handle = resp.stream_handle;
    dst.type_ = resp.resp_type.as_u32() as u8;
    dst.pin_id = resp.pin_id;
    dst.acc_id = resp.acc_id;
    crate::layout::SIZEOF_RESP_INFO as isize
}
