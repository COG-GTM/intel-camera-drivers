//! Error type for the dw9714 driver.
//!
//! The original C driver communicates failures through negated `errno` integer
//! return codes (`-EIO`, `-EINVAL`, ...). This module replaces that
//! stringly-/integer-typed channel with a proper Rust enum while preserving a
//! lossless mapping back to the C ABI via [`DW9714Error::to_errno`].

use core::fmt;

use dw9714_sys::{DW9714_MAX_FOCUS_POS, EINVAL, EIO, ENODEV, ENOMEM};

/// Errors returned by the safe dw9714 driver.
///
/// Every variant maps to a specific kernel `errno` so the C ABI shim can
/// reproduce the exact return codes the original driver used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DW9714Error {
    /// I2C transfer failed even after the single retry (`-EIO`).
    Io,
    /// An invalid argument was supplied, e.g. an unsupported V4L2 control
    /// id (`-EINVAL`).
    InvalidArgument,
    /// Allocation failed (`-ENOMEM`). Surfaced by the C-ABI constructor.
    OutOfMemory,
    /// The underlying device is missing, e.g. a GPIO could not be acquired
    /// (`-ENODEV`).
    NoDevice,
    /// A requested lens position exceeds [`DW9714_MAX_FOCUS_POS`]. Maps to
    /// `-EINVAL` on the C ABI but carries richer context in Rust.
    PositionOutOfRange {
        /// The position that was requested.
        requested: u16,
        /// The maximum permissible position.
        max: u16,
    },
    /// `probe`/init was attempted on an already-initialized device.
    AlreadyInitialized,
    /// An operation requiring an initialized device was attempted before
    /// `probe`.
    NotInitialized,
}

impl DW9714Error {
    /// Convert to the negated kernel `errno` the original C code would have
    /// returned. Always negative.
    #[must_use]
    pub const fn to_errno(self) -> i32 {
        match self {
            DW9714Error::Io => -EIO,
            DW9714Error::InvalidArgument | DW9714Error::PositionOutOfRange { .. } => -EINVAL,
            DW9714Error::OutOfMemory => -ENOMEM,
            DW9714Error::NoDevice => -ENODEV,
            // No dedicated kernel errno existed for these in the C driver;
            // they were programmer errors that could not happen by
            // construction. -EINVAL is the closest faithful mapping.
            DW9714Error::AlreadyInitialized | DW9714Error::NotInitialized => -EINVAL,
        }
    }
}

impl fmt::Display for DW9714Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DW9714Error::Io => write!(f, "i2c transfer failed"),
            DW9714Error::InvalidArgument => write!(f, "invalid argument"),
            DW9714Error::OutOfMemory => write!(f, "out of memory"),
            DW9714Error::NoDevice => write!(f, "no such device"),
            DW9714Error::PositionOutOfRange { requested, max } => {
                write!(f, "lens position {requested} out of range (max {max})")
            }
            DW9714Error::AlreadyInitialized => write!(f, "device already initialized"),
            DW9714Error::NotInitialized => write!(f, "device not initialized"),
        }
    }
}

impl core::error::Error for DW9714Error {}

/// Convenience alias mirroring the crate's pervasive `Result` shape.
pub type Result<T> = core::result::Result<T, DW9714Error>;

/// The maximum lens position, re-exported for ergonomic range checks.
pub const MAX_FOCUS_POS: u16 = DW9714_MAX_FOCUS_POS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errno_mapping_matches_c_returns() {
        assert_eq!(DW9714Error::Io.to_errno(), -EIO);
        assert_eq!(DW9714Error::InvalidArgument.to_errno(), -EINVAL);
        assert_eq!(DW9714Error::OutOfMemory.to_errno(), -ENOMEM);
        assert_eq!(DW9714Error::NoDevice.to_errno(), -ENODEV);
        assert_eq!(
            DW9714Error::PositionOutOfRange {
                requested: 2000,
                max: 1023
            }
            .to_errno(),
            -EINVAL
        );
        assert_eq!(DW9714Error::AlreadyInitialized.to_errno(), -EINVAL);
        assert_eq!(DW9714Error::NotInitialized.to_errno(), -EINVAL);
    }

    #[test]
    fn all_errnos_are_negative() {
        for e in [
            DW9714Error::Io,
            DW9714Error::InvalidArgument,
            DW9714Error::OutOfMemory,
            DW9714Error::NoDevice,
            DW9714Error::AlreadyInitialized,
            DW9714Error::NotInitialized,
        ] {
            assert!(e.to_errno() < 0, "{e} should map to a negative errno");
        }
    }

    #[test]
    fn display_is_non_empty() {
        assert!(!DW9714Error::Io.to_string().is_empty());
        assert!(DW9714Error::PositionOutOfRange {
            requested: 2000,
            max: 1023
        }
        .to_string()
        .contains("2000"));
    }
}
