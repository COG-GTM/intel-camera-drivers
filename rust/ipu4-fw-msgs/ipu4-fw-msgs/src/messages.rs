//! Idiomatic, memory-safe message types with builders and `&[u8]`-based
//! serialization/deserialization.
//!
//! Every type implements [`Marshal`], which serializes into and parses from
//! byte slices using the bounds-checked cursors in [`crate::bytes`]. The byte
//! layout reproduces the C struct layout (including padding) exactly, so output
//! is interchangeable with the firmware ABI — but unlike the C code, no raw
//! pointer casts are involved, eliminating misaligned access, buffer overruns
//! and use-after-free by construction.

use crate::bytes::{Reader, Writer};
use crate::enums::{IslUse, IsysError, ProxyError, RespType, SendType};
use crate::error::{Error, Result};
use crate::ids::{BufferId, CssVirtualAddress, OutputPinId, StreamHandle};
use crate::layout::*;
use crate::tables::bits_per_pixel;

/// Trait for types that can be serialized to / deserialized from the firmware
/// ABI byte layout.
pub trait Marshal: Sized {
    /// Exact serialized size in bytes (== `sizeof` the C struct).
    const SERIALIZED_SIZE: usize;

    /// Serialize into `buf`, returning the number of bytes written.
    ///
    /// Returns [`Error::BufferTooSmall`] if `buf` is shorter than
    /// [`Marshal::SERIALIZED_SIZE`].
    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize>;

    /// Parse a value from `buf`.
    fn deserialize(buf: &[u8]) -> Result<Self>;

    /// Serialize into a freshly allocated [`Vec<u8>`] of exactly
    /// [`Marshal::SERIALIZED_SIZE`] bytes.
    fn serialize(&self) -> Vec<u8> {
        let mut v = vec![0u8; Self::SERIALIZED_SIZE];
        // Cannot fail: the buffer is sized to SERIALIZED_SIZE.
        let n = self
            .serialize_into(&mut v)
            .expect("serialize buffer correctly sized");
        debug_assert_eq!(n, Self::SERIALIZED_SIZE);
        v
    }
}

macro_rules! impl_marshal {
    ($t:ty, $size:expr) => {
        impl Marshal for $t {
            const SERIALIZED_SIZE: usize = $size;
            fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
                let mut w = Writer::new(buf);
                self.write(&mut w)?;
                Ok(w.position())
            }
            fn deserialize(buf: &[u8]) -> Result<Self> {
                let mut r = Reader::new(buf);
                Self::read(&mut r)
            }
        }
    };
}

// ===========================================================================
// Leaf structs
// ===========================================================================

/// Generic resolution (`struct ipu_fw_isys_resolution_abi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.u32(self.width)?;
        w.u32(self.height)
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            width: r.u32()?,
            height: r.u32()?,
        })
    }
}
impl_marshal!(Resolution, SIZEOF_RESOLUTION);

/// Output pin payload (`struct ipu_fw_isys_output_pin_payload_abi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutputPinPayload {
    pub out_buf_id: BufferId,
    pub addr: CssVirtualAddress,
}

impl OutputPinPayload {
    pub const fn new(out_buf_id: BufferId, addr: CssVirtualAddress) -> Self {
        Self { out_buf_id, addr }
    }
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.u64(self.out_buf_id.get())?;
        w.u32(self.addr.get())?;
        w.pad(4) // trailing padding to 16 (u64 alignment)
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let out_buf_id = BufferId(r.u64()?);
        let addr = CssVirtualAddress(r.u32()?);
        r.skip(4)?;
        Ok(Self { out_buf_id, addr })
    }
}
impl_marshal!(OutputPinPayload, SIZEOF_OUTPUT_PIN_PAYLOAD);

/// Param pin (`struct ipu_fw_isys_param_pin_abi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParamPin {
    pub param_buf_id: BufferId,
    pub addr: CssVirtualAddress,
}

impl ParamPin {
    pub const fn new(param_buf_id: BufferId, addr: CssVirtualAddress) -> Self {
        Self { param_buf_id, addr }
    }
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.u64(self.param_buf_id.get())?;
        w.u32(self.addr.get())?;
        w.pad(4)
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let param_buf_id = BufferId(r.u64()?);
        let addr = CssVirtualAddress(r.u32()?);
        r.skip(4)?;
        Ok(Self { param_buf_id, addr })
    }
}
impl_marshal!(ParamPin, SIZEOF_PARAM_PIN);

/// Output pin descriptor (`struct ipu_fw_isys_output_pin_info_abi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutputPinInfo {
    pub output_res: Resolution,
    pub stride: u32,
    pub watermark_in_lines: u32,
    pub send_irq: u8,
    /// Related input pin id.
    pub input_pin_id: u8,
    /// Pin type (`enum ipu_fw_isys_pin_type`).
    pub pt: u8,
    /// Frame format type (`enum ipu_fw_isys_frame_format_type`).
    pub ft: u8,
    pub online: u8,
}

impl OutputPinInfo {
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        self.output_res.write(w)?;
        w.u32(self.stride)?;
        w.u32(self.watermark_in_lines)?;
        w.u8(self.send_irq)?;
        w.u8(self.input_pin_id)?;
        w.u8(self.pt)?;
        w.u8(self.ft)?;
        w.u8(self.online)?;
        w.pad(3) // trailing padding to 24
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let output_res = Resolution::read(r)?;
        let stride = r.u32()?;
        let watermark_in_lines = r.u32()?;
        let send_irq = r.u8()?;
        let input_pin_id = r.u8()?;
        let pt = r.u8()?;
        let ft = r.u8()?;
        let online = r.u8()?;
        r.skip(3)?;
        Ok(Self {
            output_res,
            stride,
            watermark_in_lines,
            send_irq,
            input_pin_id,
            pt,
            ft,
            online,
        })
    }
}
impl_marshal!(OutputPinInfo, SIZEOF_OUTPUT_PIN_INFO);

/// Input pin descriptor (`struct ipu_fw_isys_input_pin_info_abi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputPinInfo {
    pub input_res: Resolution,
    /// MIPI data type (`enum ipu_fw_isys_mipi_data_type`).
    pub dt: u8,
    /// MIPI store mode (`enum ipu_fw_isys_mipi_store_mode`).
    pub mipi_store_mode: u8,
    /// Native bits per pixel; usually derived from `dt` via [`bits_per_pixel`].
    pub bits_per_pix: u8,
}

impl InputPinInfo {
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        self.input_res.write(w)?;
        w.u8(self.dt)?;
        w.u8(self.mipi_store_mode)?;
        w.u8(self.bits_per_pix)?;
        w.pad(1) // trailing padding to 12
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let input_res = Resolution::read(r)?;
        let dt = r.u8()?;
        let mipi_store_mode = r.u8()?;
        let bits_per_pix = r.u8()?;
        r.skip(1)?;
        Ok(Self {
            input_res,
            dt,
            mipi_store_mode,
            bits_per_pix,
        })
    }
}
impl_marshal!(InputPinInfo, SIZEOF_INPUT_PIN_INFO);

/// ISA configuration (`struct ipu_fw_isys_isa_cfg_abi`).
///
/// The C `cfg` bitfield is packed into a single little-endian `u32`. Bit
/// positions (LSB first) are documented per field below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IsaConfig {
    pub isa_res: [Resolution; N_IPU_FW_ISYS_RESOLUTION_INFO],
    pub blc: bool,
    pub lsc: bool,
    pub dpc: bool,
    pub downscaler: bool,
    pub awb: bool,
    pub af: bool,
    pub ae: bool,
    /// 8-bit PAF field (bits 7..15).
    pub paf: u8,
    pub send_irq_stats_ready: bool,
    pub send_resp_stats_ready: bool,
}

impl IsaConfig {
    /// Pack the boolean / `paf` flags into the ABI `cfg` word.
    pub fn pack_cfg(&self) -> u32 {
        let mut v = 0u32;
        v |= self.blc as u32;
        v |= (self.lsc as u32) << 1;
        v |= (self.dpc as u32) << 2;
        v |= (self.downscaler as u32) << 3;
        v |= (self.awb as u32) << 4;
        v |= (self.af as u32) << 5;
        v |= (self.ae as u32) << 6;
        v |= (self.paf as u32) << 7;
        v |= (self.send_irq_stats_ready as u32) << 15;
        v |= (self.send_resp_stats_ready as u32) << 16;
        v
    }

    /// Unpack the ABI `cfg` word into individual flags.
    pub(crate) fn unpack_cfg(isa_res: [Resolution; N_IPU_FW_ISYS_RESOLUTION_INFO], v: u32) -> Self {
        Self {
            isa_res,
            blc: v & 1 != 0,
            lsc: v & (1 << 1) != 0,
            dpc: v & (1 << 2) != 0,
            downscaler: v & (1 << 3) != 0,
            awb: v & (1 << 4) != 0,
            af: v & (1 << 5) != 0,
            ae: v & (1 << 6) != 0,
            paf: ((v >> 7) & 0xff) as u8,
            send_irq_stats_ready: v & (1 << 15) != 0,
            send_resp_stats_ready: v & (1 << 16) != 0,
        }
    }

    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        for res in &self.isa_res {
            res.write(w)?;
        }
        w.u32(self.pack_cfg())
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let mut isa_res = [Resolution::default(); N_IPU_FW_ISYS_RESOLUTION_INFO];
        for res in &mut isa_res {
            *res = Resolution::read(r)?;
        }
        let cfg = r.u32()?;
        Ok(Self::unpack_cfg(isa_res, cfg))
    }
}
impl_marshal!(IsaConfig, SIZEOF_ISA_CFG);

/// Cropping coordinates (`struct ipu_fw_isys_cropping_abi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cropping {
    pub top_offset: i32,
    pub left_offset: i32,
    pub bottom_offset: i32,
    pub right_offset: i32,
}

impl Cropping {
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.i32(self.top_offset)?;
        w.i32(self.left_offset)?;
        w.i32(self.bottom_offset)?;
        w.i32(self.right_offset)
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            top_offset: r.i32()?,
            left_offset: r.i32()?,
            bottom_offset: r.i32()?,
            right_offset: r.i32()?,
        })
    }
}
impl_marshal!(Cropping, SIZEOF_CROPPING);

// ===========================================================================
// Stream configuration (stream-open payload)
// ===========================================================================

/// Stream configuration data (`struct ipu_fw_isys_stream_cfg_data_abi`).
///
/// This is the payload referenced by a `STREAM_OPEN` command. `input_pins` and
/// `output_pins` hold only the *valid* pins (their lengths become
/// `nof_input_pins` / `nof_output_pins`); unused ABI array slots are serialized
/// as zero, exactly as the kernel driver leaves a `memset`-zeroed struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCfgData {
    pub isa_cfg: IsaConfig,
    pub crop: [Cropping; N_IPU_FW_ISYS_CROPPING_LOCATION],
    pub input_pins: Vec<InputPinInfo>,
    pub output_pins: Vec<OutputPinInfo>,
    pub compfmt: u32,
    pub send_irq_sof_discarded: bool,
    pub send_irq_eof_discarded: bool,
    pub send_resp_sof_discarded: bool,
    pub send_resp_eof_discarded: bool,
    pub src: u8,
    pub vc: u8,
    pub isl_use: IslUse,
}

impl Default for StreamCfgData {
    fn default() -> Self {
        Self {
            isa_cfg: IsaConfig::default(),
            crop: [Cropping::default(); N_IPU_FW_ISYS_CROPPING_LOCATION],
            input_pins: Vec::new(),
            output_pins: Vec::new(),
            compfmt: 0,
            send_irq_sof_discarded: false,
            send_irq_eof_discarded: false,
            send_resp_sof_discarded: false,
            send_resp_eof_discarded: false,
            src: 0,
            vc: 0,
            isl_use: IslUse::NoIslNoIsa,
        }
    }
}

impl StreamCfgData {
    /// Start building a stream configuration.
    pub fn builder() -> StreamCfgBuilder {
        StreamCfgBuilder::default()
    }

    /// Number of valid input pins (the serialized `nof_input_pins`).
    pub fn nof_input_pins(&self) -> u8 {
        self.input_pins.len() as u8
    }

    /// Number of valid output pins (the serialized `nof_output_pins`).
    pub fn nof_output_pins(&self) -> u8 {
        self.output_pins.len() as u8
    }

    /// Port of `intel_ipu4_isys_set_fw_params`: fill each input pin's
    /// `bits_per_pix` from the MIPI-data-type lookup table.
    pub fn set_fw_params(&mut self) {
        for pin in &mut self.input_pins {
            pin.bits_per_pix = bits_per_pixel(pin.dt) as u8;
        }
    }

    fn validate(&self) -> Result<()> {
        if self.input_pins.len() > INTEL_IPU4_MAX_IPINS {
            return Err(Error::TooMany {
                field: "input_pins",
                value: self.input_pins.len(),
                max: INTEL_IPU4_MAX_IPINS,
            });
        }
        if self.output_pins.len() > INTEL_IPU4_MAX_OPINS {
            return Err(Error::TooMany {
                field: "output_pins",
                value: self.output_pins.len(),
                max: INTEL_IPU4_MAX_OPINS,
            });
        }
        Ok(())
    }

    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        self.validate()?;
        self.isa_cfg.write(w)?;
        for c in &self.crop {
            c.write(w)?;
        }
        for i in 0..INTEL_IPU4_MAX_IPINS {
            match self.input_pins.get(i) {
                Some(pin) => pin.write(w)?,
                None => w.pad(SIZEOF_INPUT_PIN_INFO)?,
            }
        }
        for i in 0..INTEL_IPU4_MAX_OPINS {
            match self.output_pins.get(i) {
                Some(pin) => pin.write(w)?,
                None => w.pad(SIZEOF_OUTPUT_PIN_INFO)?,
            }
        }
        w.u32(self.compfmt)?;
        w.u8(self.nof_input_pins())?;
        w.u8(self.nof_output_pins())?;
        w.u8(self.send_irq_sof_discarded as u8)?;
        w.u8(self.send_irq_eof_discarded as u8)?;
        w.u8(self.send_resp_sof_discarded as u8)?;
        w.u8(self.send_resp_eof_discarded as u8)?;
        w.u8(self.src)?;
        w.u8(self.vc)?;
        w.u8(self.isl_use.as_u32() as u8)?;
        w.pad(3) // trailing padding to 292
    }

    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let isa_cfg = IsaConfig::read(r)?;
        let mut crop = [Cropping::default(); N_IPU_FW_ISYS_CROPPING_LOCATION];
        for c in &mut crop {
            *c = Cropping::read(r)?;
        }
        let mut all_inputs = [InputPinInfo::default(); INTEL_IPU4_MAX_IPINS];
        for pin in &mut all_inputs {
            *pin = InputPinInfo::read(r)?;
        }
        let mut all_outputs = [OutputPinInfo::default(); INTEL_IPU4_MAX_OPINS];
        for pin in &mut all_outputs {
            *pin = OutputPinInfo::read(r)?;
        }
        let compfmt = r.u32()?;
        let nof_input_pins = r.u8()? as usize;
        let nof_output_pins = r.u8()? as usize;
        let send_irq_sof_discarded = r.u8()? != 0;
        let send_irq_eof_discarded = r.u8()? != 0;
        let send_resp_sof_discarded = r.u8()? != 0;
        let send_resp_eof_discarded = r.u8()? != 0;
        let src = r.u8()?;
        let vc = r.u8()?;
        let isl_use = IslUse::from_u32(r.u8()? as u32)?;
        r.skip(3)?;

        if nof_input_pins > INTEL_IPU4_MAX_IPINS {
            return Err(Error::TooMany {
                field: "nof_input_pins",
                value: nof_input_pins,
                max: INTEL_IPU4_MAX_IPINS,
            });
        }
        if nof_output_pins > INTEL_IPU4_MAX_OPINS {
            return Err(Error::TooMany {
                field: "nof_output_pins",
                value: nof_output_pins,
                max: INTEL_IPU4_MAX_OPINS,
            });
        }

        Ok(Self {
            isa_cfg,
            crop,
            input_pins: all_inputs[..nof_input_pins].to_vec(),
            output_pins: all_outputs[..nof_output_pins].to_vec(),
            compfmt,
            send_irq_sof_discarded,
            send_irq_eof_discarded,
            send_resp_sof_discarded,
            send_resp_eof_discarded,
            src,
            vc,
            isl_use,
        })
    }
}
impl_marshal!(StreamCfgData, SIZEOF_STREAM_CFG_DATA);

/// Builder for [`StreamCfgData`] with range-checked pin insertion.
#[derive(Debug, Default)]
pub struct StreamCfgBuilder {
    inner: StreamCfgData,
}

impl StreamCfgBuilder {
    pub fn isa_cfg(mut self, isa_cfg: IsaConfig) -> Self {
        self.inner.isa_cfg = isa_cfg;
        self
    }

    /// Set the cropping descriptor at `location`
    /// (`< N_IPU_FW_ISYS_CROPPING_LOCATION`).
    pub fn crop(mut self, location: usize, crop: Cropping) -> Result<Self> {
        let slot = self
            .inner
            .crop
            .get_mut(location)
            .ok_or(Error::IndexOutOfRange {
                field: "crop",
                index: location,
                len: N_IPU_FW_ISYS_CROPPING_LOCATION,
            })?;
        *slot = crop;
        Ok(self)
    }

    /// Append an input pin, enforcing the `INTEL_IPU4_MAX_IPINS` limit.
    pub fn add_input_pin(mut self, pin: InputPinInfo) -> Result<Self> {
        if self.inner.input_pins.len() >= INTEL_IPU4_MAX_IPINS {
            return Err(Error::TooMany {
                field: "input_pins",
                value: self.inner.input_pins.len() + 1,
                max: INTEL_IPU4_MAX_IPINS,
            });
        }
        self.inner.input_pins.push(pin);
        Ok(self)
    }

    /// Append an output pin, enforcing the `INTEL_IPU4_MAX_OPINS` limit.
    pub fn add_output_pin(mut self, pin: OutputPinInfo) -> Result<Self> {
        if self.inner.output_pins.len() >= INTEL_IPU4_MAX_OPINS {
            return Err(Error::TooMany {
                field: "output_pins",
                value: self.inner.output_pins.len() + 1,
                max: INTEL_IPU4_MAX_OPINS,
            });
        }
        self.inner.output_pins.push(pin);
        Ok(self)
    }

    pub fn compfmt(mut self, compfmt: u32) -> Self {
        self.inner.compfmt = compfmt;
        self
    }

    pub fn src(mut self, src: u8) -> Self {
        self.inner.src = src;
        self
    }

    pub fn vc(mut self, vc: u8) -> Self {
        self.inner.vc = vc;
        self
    }

    pub fn isl_use(mut self, isl_use: IslUse) -> Self {
        self.inner.isl_use = isl_use;
        self
    }

    pub fn send_irq_sof_discarded(mut self, v: bool) -> Self {
        self.inner.send_irq_sof_discarded = v;
        self
    }

    pub fn send_irq_eof_discarded(mut self, v: bool) -> Self {
        self.inner.send_irq_eof_discarded = v;
        self
    }

    pub fn send_resp_sof_discarded(mut self, v: bool) -> Self {
        self.inner.send_resp_sof_discarded = v;
        self
    }

    pub fn send_resp_eof_discarded(mut self, v: bool) -> Self {
        self.inner.send_resp_eof_discarded = v;
        self
    }

    /// If `true`, fill each input pin's `bits_per_pix` from the data-type table
    /// (mirrors `intel_ipu4_isys_set_fw_params`) before returning.
    pub fn auto_bits_per_pix(mut self, enable: bool) -> Self {
        if enable {
            self.inner.set_fw_params();
        }
        self
    }

    /// Validate and produce the [`StreamCfgData`].
    pub fn build(self) -> Result<StreamCfgData> {
        self.inner.validate()?;
        Ok(self.inner)
    }
}

// ===========================================================================
// Frame buffer set (capture payload)
// ===========================================================================

/// Frame buffer set (`struct ipu_fw_isys_frame_buff_set_abi`).
///
/// The ABI struct always carries `INTEL_IPU4_MAX_OPINS` output-pin payload
/// slots; `output_pins` holds the populated ones and the remainder are
/// serialized as zero.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameBuffSet {
    pub output_pins: Vec<OutputPinPayload>,
    pub process_group_light: ParamPin,
    pub send_irq_sof: bool,
    pub send_irq_eof: bool,
    pub send_resp_sof: bool,
    pub send_resp_eof: bool,
}

impl FrameBuffSet {
    pub fn builder() -> FrameBuffSetBuilder {
        FrameBuffSetBuilder::default()
    }

    fn validate(&self) -> Result<()> {
        if self.output_pins.len() > INTEL_IPU4_MAX_OPINS {
            return Err(Error::TooMany {
                field: "output_pins",
                value: self.output_pins.len(),
                max: INTEL_IPU4_MAX_OPINS,
            });
        }
        Ok(())
    }

    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        self.validate()?;
        for i in 0..INTEL_IPU4_MAX_OPINS {
            match self.output_pins.get(i) {
                Some(p) => p.write(w)?,
                None => w.pad(SIZEOF_OUTPUT_PIN_PAYLOAD)?,
            }
        }
        self.process_group_light.write(w)?;
        w.u8(self.send_irq_sof as u8)?;
        w.u8(self.send_irq_eof as u8)?;
        w.u8(self.send_resp_sof as u8)?;
        w.u8(self.send_resp_eof as u8)?;
        w.pad(4) // trailing padding to 120 (u64 alignment)
    }

    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let mut output_pins = Vec::with_capacity(INTEL_IPU4_MAX_OPINS);
        for _ in 0..INTEL_IPU4_MAX_OPINS {
            output_pins.push(OutputPinPayload::read(r)?);
        }
        let process_group_light = ParamPin::read(r)?;
        let send_irq_sof = r.u8()? != 0;
        let send_irq_eof = r.u8()? != 0;
        let send_resp_sof = r.u8()? != 0;
        let send_resp_eof = r.u8()? != 0;
        r.skip(4)?;
        Ok(Self {
            output_pins,
            process_group_light,
            send_irq_sof,
            send_irq_eof,
            send_resp_sof,
            send_resp_eof,
        })
    }
}
impl_marshal!(FrameBuffSet, SIZEOF_FRAME_BUFF_SET);

/// Builder for [`FrameBuffSet`].
#[derive(Debug, Default)]
pub struct FrameBuffSetBuilder {
    inner: FrameBuffSet,
}

impl FrameBuffSetBuilder {
    /// Append an output-pin payload at the next index, enforcing
    /// `INTEL_IPU4_MAX_OPINS`.
    pub fn add_output_pin(mut self, payload: OutputPinPayload) -> Result<Self> {
        if self.inner.output_pins.len() >= INTEL_IPU4_MAX_OPINS {
            return Err(Error::TooMany {
                field: "output_pins",
                value: self.inner.output_pins.len() + 1,
                max: INTEL_IPU4_MAX_OPINS,
            });
        }
        self.inner.output_pins.push(payload);
        Ok(self)
    }

    /// Set a specific output-pin payload by [`OutputPinId`], growing the vector
    /// with default payloads as needed.
    pub fn set_output_pin(mut self, id: OutputPinId, payload: OutputPinPayload) -> Self {
        let idx = id.get() as usize;
        if self.inner.output_pins.len() <= idx {
            self.inner
                .output_pins
                .resize(idx + 1, OutputPinPayload::default());
        }
        self.inner.output_pins[idx] = payload;
        self
    }

    pub fn process_group_light(mut self, pgl: ParamPin) -> Self {
        self.inner.process_group_light = pgl;
        self
    }

    pub fn send_irq_sof(mut self, v: bool) -> Self {
        self.inner.send_irq_sof = v;
        self
    }

    pub fn send_irq_eof(mut self, v: bool) -> Self {
        self.inner.send_irq_eof = v;
        self
    }

    pub fn send_resp_sof(mut self, v: bool) -> Self {
        self.inner.send_resp_sof = v;
        self
    }

    pub fn send_resp_eof(mut self, v: bool) -> Self {
        self.inner.send_resp_eof = v;
        self
    }

    pub fn build(self) -> Result<FrameBuffSet> {
        self.inner.validate()?;
        Ok(self.inner)
    }
}

// ===========================================================================
// Command tokens (stream open/start/capture/stop/flush/close + proxy)
// ===========================================================================

/// Send-queue command token (`struct ipu_fw_send_queue_token`).
///
/// This is the message produced for stream open/start/capture/stop/flush/close,
/// mirroring `intel_ipu4_isys_abi_complex_cmd` / `intel_ipu4_isys_abi_simple_cmd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendQueueToken {
    /// CPU-side buffer handle (`buf_handle`); zero for "simple" commands.
    pub buf_handle: BufferId,
    /// DMA-mapped payload virtual address (`payload`); zero for "simple".
    pub payload: CssVirtualAddress,
    pub send_type: SendType,
}

impl SendQueueToken {
    /// Build a "complex" command carrying a payload (mirrors
    /// `intel_ipu4_isys_abi_complex_cmd`).
    pub const fn complex(
        buf_handle: BufferId,
        payload: CssVirtualAddress,
        send_type: SendType,
    ) -> Self {
        Self {
            buf_handle,
            payload,
            send_type,
        }
    }

    /// Build a "simple" command with no payload (mirrors
    /// `intel_ipu4_isys_abi_simple_cmd`): `buf_handle` and `payload` are zero.
    pub const fn simple(send_type: SendType) -> Self {
        Self {
            buf_handle: BufferId(0),
            payload: CssVirtualAddress(0),
            send_type,
        }
    }

    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.u64(self.buf_handle.get())?;
        w.u32(self.payload.get())?;
        w.u32(self.send_type.as_u32())
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let buf_handle = BufferId(r.u64()?);
        let payload = CssVirtualAddress(r.u32()?);
        let send_type = SendType::from_u32(r.u32()?)?;
        Ok(Self {
            buf_handle,
            payload,
            send_type,
        })
    }
}
impl_marshal!(SendQueueToken, SIZEOF_SEND_QUEUE_TOKEN);

/// Proxy send-queue token (`struct ipu_fw_proxy_send_queue_token`), mirroring
/// `intel_ipu4_isys_send_proxy_token`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProxySendQueueToken {
    pub request_id: u32,
    pub region_index: u32,
    pub offset: u32,
    pub value: u32,
}

impl ProxySendQueueToken {
    pub const fn new(request_id: u32, region_index: u32, offset: u32, value: u32) -> Self {
        Self {
            request_id,
            region_index,
            offset,
            value,
        }
    }
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.u32(self.request_id)?;
        w.u32(self.region_index)?;
        w.u32(self.offset)?;
        w.u32(self.value)
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            request_id: r.u32()?,
            region_index: r.u32()?,
            offset: r.u32()?,
            value: r.u32()?,
        })
    }
}
impl_marshal!(ProxySendQueueToken, SIZEOF_PROXY_SEND_QUEUE_TOKEN);

// ===========================================================================
// Responses (parsing)
// ===========================================================================

/// Firmware error info (`struct ipu_fw_isys_error_info_abi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorInfo {
    pub error: IsysError,
    pub error_details: u32,
}

impl Default for ErrorInfo {
    fn default() -> Self {
        Self {
            error: IsysError::None,
            error_details: 0,
        }
    }
}

impl ErrorInfo {
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.u32(self.error.as_u32())?;
        w.u32(self.error_details)
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let error = IsysError::from_u32(r.u32()?)?;
        let error_details = r.u32()?;
        Ok(Self {
            error,
            error_details,
        })
    }
}
impl_marshal!(ErrorInfo, SIZEOF_ERROR_INFO);

/// Response info (`struct ipu_fw_isys_resp_info_abi`).
///
/// Parsed from a response-queue token; mirrors `intel_ipu4_isys_abi_get_resp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RespInfo {
    pub buf_id: BufferId,
    pub pin: OutputPinPayload,
    pub process_group_light: ParamPin,
    pub error_info: ErrorInfo,
    pub timestamp: [u32; 2],
    pub stream_handle: u8,
    /// Response type (`enum ipu_fw_isys_resp_type`, stored as `u8`).
    pub resp_type: RespType,
    pub pin_id: u8,
    pub acc_id: u8,
}

impl RespInfo {
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.u64(self.buf_id.get())?;
        self.pin.write(w)?;
        self.process_group_light.write(w)?;
        self.error_info.write(w)?;
        w.u32(self.timestamp[0])?;
        w.u32(self.timestamp[1])?;
        w.u8(self.stream_handle)?;
        w.u8(self.resp_type.as_u32() as u8)?;
        w.u8(self.pin_id)?;
        w.u8(self.acc_id)?;
        w.pad(4) // trailing padding to 64
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let buf_id = BufferId(r.u64()?);
        let pin = OutputPinPayload::read(r)?;
        let process_group_light = ParamPin::read(r)?;
        let error_info = ErrorInfo::read(r)?;
        let timestamp = [r.u32()?, r.u32()?];
        let stream_handle = r.u8()?;
        let resp_type = RespType::from_u32(r.u8()? as u32)?;
        let pin_id = r.u8()?;
        let acc_id = r.u8()?;
        r.skip(4)?;
        Ok(Self {
            buf_id,
            pin,
            process_group_light,
            error_info,
            timestamp,
            stream_handle,
            resp_type,
            pin_id,
            acc_id,
        })
    }
}
impl_marshal!(RespInfo, SIZEOF_RESP_INFO);

/// Proxy error info (`struct ipu_fw_isys_proxy_error_info_abi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxyErrorInfo {
    pub error: ProxyError,
    pub error_details: u32,
}

impl Default for ProxyErrorInfo {
    fn default() -> Self {
        Self {
            error: ProxyError::None,
            error_details: 0,
        }
    }
}

impl ProxyErrorInfo {
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.u32(self.error.as_u32())?;
        w.u32(self.error_details)
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let error = ProxyError::from_u32(r.u32()?)?;
        let error_details = r.u32()?;
        Ok(Self {
            error,
            error_details,
        })
    }
}
impl_marshal!(ProxyErrorInfo, SIZEOF_PROXY_ERROR_INFO);

/// Proxy response info (`struct ipu_fw_isys_proxy_resp_info_abi`), parsed in
/// `handle_proxy_response`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProxyRespInfo {
    pub request_id: u32,
    pub error_info: ProxyErrorInfo,
}

impl ProxyRespInfo {
    pub(crate) fn write(&self, w: &mut Writer) -> Result<()> {
        w.u32(self.request_id)?;
        self.error_info.write(w)
    }
    pub(crate) fn read(r: &mut Reader) -> Result<Self> {
        let request_id = r.u32()?;
        let error_info = ProxyErrorInfo::read(r)?;
        Ok(Self {
            request_id,
            error_info,
        })
    }
}
impl_marshal!(ProxyRespInfo, SIZEOF_PROXY_RESP_INFO);

// ===========================================================================
// Queue routing helpers (ported from the C indexing arithmetic)
// ===========================================================================

/// Message-queue index for a stream command (`stream_handle + ISYS_MSG_INDEX`),
/// mirroring `intel_ipu4_isys_abi_complex_cmd`.
#[inline]
pub fn message_queue_index(stream: StreamHandle) -> usize {
    stream.get() as usize + ISYS_MSG_INDEX
}

/// Proxy command queue index (`ISYS_PROXY_INDEX`).
#[inline]
pub const fn proxy_queue_index() -> usize {
    ISYS_PROXY_INDEX
}
