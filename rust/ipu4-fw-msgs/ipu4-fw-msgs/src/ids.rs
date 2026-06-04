//! Strongly-typed newtypes for pin, stream and buffer indices.
//!
//! The C driver passes all of these around as bare integers (`u8`/`u32`/`u64`),
//! which makes it trivially easy to swap a pin id for a stream handle or to
//! index an array out of bounds. These newtypes make such mistakes a
//! compile-time error and centralize range validation.

use crate::error::{Error, Result};
use crate::layout::{INTEL_IPU4_MAX_IPINS, INTEL_IPU4_MAX_OPINS, INTEL_IPU4_STREAM_ID_MAX};

/// Index of an input pin within a stream configuration.
///
/// Valid range is `0..INTEL_IPU4_MAX_IPINS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputPinId(u8);

impl InputPinId {
    /// Create a new input-pin id, validating it against the ABI maximum.
    pub fn new(index: u8) -> Result<Self> {
        if (index as usize) < INTEL_IPU4_MAX_IPINS {
            Ok(Self(index))
        } else {
            Err(Error::IndexOutOfRange {
                field: "input_pin",
                index: index as usize,
                len: INTEL_IPU4_MAX_IPINS,
            })
        }
    }

    /// The raw index value.
    #[inline]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Index of an output pin within a stream configuration or frame buffer set.
///
/// Valid range is `0..INTEL_IPU4_MAX_OPINS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutputPinId(u8);

impl OutputPinId {
    /// Create a new output-pin id, validating it against the ABI maximum.
    pub fn new(index: u8) -> Result<Self> {
        if (index as usize) < INTEL_IPU4_MAX_OPINS {
            Ok(Self(index))
        } else {
            Err(Error::IndexOutOfRange {
                field: "output_pin",
                index: index as usize,
                len: INTEL_IPU4_MAX_OPINS,
            })
        }
    }

    /// The raw index value.
    #[inline]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Handle identifying a virtual stream.
///
/// Valid range is `0..INTEL_IPU4_STREAM_ID_MAX`. The firmware uses this value
/// (offset by the proxy queue) to select the message queue for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamHandle(u8);

impl StreamHandle {
    /// Create a new stream handle, validating it against the ABI maximum.
    pub fn new(handle: u8) -> Result<Self> {
        if (handle as usize) < INTEL_IPU4_STREAM_ID_MAX {
            Ok(Self(handle))
        } else {
            Err(Error::IndexOutOfRange {
                field: "stream_handle",
                index: handle as usize,
                len: INTEL_IPU4_STREAM_ID_MAX,
            })
        }
    }

    /// The raw handle value.
    #[inline]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Opaque firmware buffer identifier (`out_buf_id`, `param_buf_id`,
/// `buf_handle`, `buf_id`).
///
/// These are CPU-side handles the firmware echoes back verbatim; they carry no
/// arithmetic meaning, so the newtype prevents accidental mixing with virtual
/// addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BufferId(pub u64);

impl BufferId {
    /// The raw 64-bit identifier.
    #[inline]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A CSS virtual address (DMA-mapped payload pointer) as seen by the firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CssVirtualAddress(pub u32);

impl CssVirtualAddress {
    /// The raw 32-bit address.
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }
}
