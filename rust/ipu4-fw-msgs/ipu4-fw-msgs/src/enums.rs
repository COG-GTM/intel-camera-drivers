//! Safe enums mirroring the firmware ABI enumerations.
//!
//! Each enum carries the same discriminant values as its C counterpart and is
//! serialized as a little-endian `u32` (C `enum` == `int` on the target ABI).
//! Parsing an unknown discriminant returns [`Error::InvalidEnum`] rather than
//! producing an out-of-range value (malformed-input rejection).

use crate::error::{Error, Result};

macro_rules! abi_enum {
    (
        $(#[$meta:meta])*
        $name:ident ($field:literal) {
            $( $variant:ident = $value:expr ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u32)]
        pub enum $name {
            $( $variant = $value ),+
        }

        impl $name {
            /// The raw C discriminant value.
            #[inline]
            pub const fn as_u32(self) -> u32 {
                self as u32
            }

            /// Parse from a raw discriminant, rejecting unknown values.
            pub fn from_u32(value: u32) -> Result<Self> {
                match value {
                    $( $value => Ok($name::$variant), )+
                    _ => Err(Error::InvalidEnum { field: $field, value }),
                }
            }
        }
    };
}

abi_enum! {
    /// `enum ipu_fw_isys_send_type` — command message kinds.
    SendType ("send_type") {
        StreamOpen = 0,
        StreamStart = 1,
        StreamStartAndCapture = 2,
        StreamCapture = 3,
        StreamStop = 4,
        StreamFlush = 5,
        StreamClose = 6,
    }
}

abi_enum! {
    /// `enum ipu_fw_isys_resp_type` — response message kinds.
    RespType ("resp_type") {
        StreamOpenDone = 0,
        StreamStartAck = 1,
        StreamStartAndCaptureAck = 2,
        StreamCaptureAck = 3,
        StreamStopAck = 4,
        StreamFlushAck = 5,
        StreamCloseAck = 6,
        PinDataReady = 7,
        PinDataWatermark = 8,
        FrameSof = 9,
        FrameEof = 10,
        StreamStartAndCaptureDone = 11,
        StreamCaptureDone = 12,
        PinDataSkipped = 13,
        StreamCaptureSkipped = 14,
        FrameSofDiscarded = 15,
        FrameEofDiscarded = 16,
        StatsDataReady = 17,
    }
}

abi_enum! {
    /// `enum ipu_fw_isys_isl_use` — ISL/ISA usage of a stream.
    IslUse ("isl_use") {
        NoIslNoIsa = 0,
        SingleDualIsl = 1,
        SingleIsa = 2,
    }
}

abi_enum! {
    /// `enum ipu_fw_isys_error` — firmware-reported error codes.
    IsysError ("isys_error") {
        None = 0,
        FwInternalConsistency = 1,
        HwConsistency = 2,
        DriverInvalidCommandSequence = 3,
        DriverInvalidDeviceConfiguration = 4,
        DriverInvalidStreamConfiguration = 5,
        DriverInvalidFrameConfiguration = 6,
        InsufficientResources = 7,
        HwReportedStr2mmio = 8,
        HwReportedSig2cio = 9,
        SensorFwSync = 10,
        StreamInSuspension = 11,
        ResponseQueueFull = 12,
    }
}

abi_enum! {
    /// `enum ipu_fw_proxy_error` — proxy-write error codes.
    ProxyError ("proxy_error") {
        None = 0,
        InvalidWriteRegion = 1,
        InvalidWriteOffset = 2,
    }
}
