//! Error type for message construction, validation and (de)serialization.

use core::fmt;

/// Result alias used throughout the crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Errors produced while building, serializing or parsing firmware messages.
///
/// Every fallible operation in this crate returns [`Error`] instead of relying
/// on raw pointer casts or undefined behaviour, which is the central
/// memory-safety improvement over the original C marshaling code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A destination/source byte buffer was too small for the message.
    ///
    /// `expected` bytes were required but only `actual` were available. This is
    /// the safe-Rust replacement for the C code's unchecked pointer casts that
    /// could read or write past the end of a buffer (buffer overrun).
    BufferTooSmall { expected: usize, actual: usize },
    /// A count exceeded the fixed ABI capacity of an array field.
    TooMany {
        /// Human-readable name of the field that overflowed.
        field: &'static str,
        /// The value that was supplied.
        value: usize,
        /// The maximum allowed value.
        max: usize,
    },
    /// An index addressed an element outside the fixed ABI array bounds.
    IndexOutOfRange {
        /// Human-readable name of the indexed field.
        field: &'static str,
        /// The index that was supplied.
        index: usize,
        /// The maximum allowed index (exclusive upper bound).
        len: usize,
    },
    /// A scalar value was outside the range permitted by the ABI.
    ValueOutOfRange {
        /// Human-readable name of the field.
        field: &'static str,
        /// The value that was supplied.
        value: u64,
        /// The maximum allowed value (inclusive).
        max: u64,
    },
    /// A serialized enum discriminant did not match any known variant.
    InvalidEnum {
        /// Human-readable name of the enum.
        field: &'static str,
        /// The raw discriminant that was read.
        value: u32,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BufferTooSmall { expected, actual } => {
                write!(f, "buffer too small: needed {expected} bytes, had {actual}")
            }
            Error::TooMany { field, value, max } => {
                write!(f, "too many {field}: {value} exceeds maximum {max}")
            }
            Error::IndexOutOfRange { field, index, len } => {
                write!(f, "index {index} out of range for {field} (len {len})")
            }
            Error::ValueOutOfRange { field, value, max } => {
                write!(f, "value {value} out of range for {field} (max {max})")
            }
            Error::InvalidEnum { field, value } => {
                write!(f, "invalid discriminant {value} for {field}")
            }
        }
    }
}

impl std::error::Error for Error {}
