//! Error types for the firmware communication layer.
//!
//! The C implementation signals failure either by returning a `NULL` pointer
//! (token acquisition / prepare) or a negative `errno` (`-EIO`, `-EBUSY`).
//! Here those failure modes become explicit, exhaustively-matchable variants.

use core::fmt;

/// Errors produced by the firmware communication layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FwComError {
    /// The send queue has no free slot for another token.
    QueueFull,
    /// The receive queue has no pending token.
    QueueEmpty,
    /// A dmem index read back from shared memory is out of range.
    ///
    /// This mirrors the C `is_index_valid()` guard, which protects against a
    /// firmware that scribbles a bogus index into shared dmem.
    IndexOutOfRange {
        /// The out-of-range index value read from dmem.
        index: u32,
        /// The queue size the index was checked against.
        size: u32,
    },
    /// The requested queue number does not exist.
    QueueIndexOutOfRange {
        /// The requested queue number.
        index: usize,
        /// The number of queues that exist.
        count: usize,
    },
    /// A caller-supplied token buffer does not match the queue's token size.
    TokenSizeMismatch {
        /// The queue's configured token size in bytes.
        expected: usize,
        /// The size of the buffer the caller supplied.
        actual: usize,
    },
    /// A queue was configured with a zero `queue_size`.
    ZeroQueueSize,
    /// A queue was configured with a zero `token_size`.
    ZeroTokenSize,
    /// The total shared allocation size overflowed `usize`.
    SizeOverflow,
    /// The firmware cell is not ready (`-EIO` in the C driver).
    CellNotReady,
    /// The syscom is not in the `READY` state (`-EBUSY` in the C driver).
    Busy,
}

impl fmt::Display for FwComError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FwComError::QueueFull => write!(f, "send queue is full"),
            FwComError::QueueEmpty => write!(f, "receive queue is empty"),
            FwComError::IndexOutOfRange { index, size } => {
                write!(f, "dmem index {index} out of range for queue size {size}")
            }
            FwComError::QueueIndexOutOfRange { index, count } => {
                write!(f, "queue index {index} out of range (have {count} queues)")
            }
            FwComError::TokenSizeMismatch { expected, actual } => {
                write!(f, "token size mismatch: expected {expected}, got {actual}")
            }
            FwComError::ZeroQueueSize => write!(f, "queue_size must be non-zero"),
            FwComError::ZeroTokenSize => write!(f, "token_size must be non-zero"),
            FwComError::SizeOverflow => write!(f, "shared allocation size overflow"),
            FwComError::CellNotReady => write!(f, "firmware cell is not ready"),
            FwComError::Busy => write!(f, "syscom is busy / not ready"),
        }
    }
}

impl std::error::Error for FwComError {}

/// The C driver returns negative `errno` values; this maps the relevant errors
/// back onto those codes so the `c-abi` shim can preserve behaviour.
impl FwComError {
    /// Negative errno equivalent, matching the original driver's return codes.
    pub fn to_errno(&self) -> i32 {
        match self {
            FwComError::CellNotReady => -5, // -EIO
            _ => -16,                       // -EBUSY
        }
    }
}

/// Convenience result alias.
pub type Result<T> = core::result::Result<T, FwComError>;
