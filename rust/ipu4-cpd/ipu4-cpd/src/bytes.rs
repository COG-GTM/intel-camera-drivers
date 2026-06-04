//! Bounds-checked, little-endian primitive readers.
//!
//! Every multi-byte field in the CPD format is little-endian (the C structs are
//! `__packed` and the driver runs on little-endian x86). These helpers replace
//! the C pointer casts (`*(u32 *)p`) with reads that can never run past the end
//! of the slice — turning a class of silent buffer overreads into a typed
//! [`CpdError::OutOfBounds`].

use crate::error::{CpdError, Result};

/// Borrow `len` bytes starting at `offset`, or fail if they don't fit.
#[inline]
pub fn slice(buf: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset.checked_add(len).ok_or(CpdError::ArithmeticOverflow)?;
    buf.get(offset..end).ok_or(CpdError::OutOfBounds {
        offset,
        need: len,
        len: buf.len(),
    })
}

/// Read a little-endian `u8` at `offset`.
#[inline]
pub fn u8_at(buf: &[u8], offset: usize) -> Result<u8> {
    buf.get(offset).copied().ok_or(CpdError::OutOfBounds {
        offset,
        need: 1,
        len: buf.len(),
    })
}

/// Read a little-endian `u32` at `offset`.
#[inline]
pub fn u32_at(buf: &[u8], offset: usize) -> Result<u32> {
    let b = slice(buf, offset, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian() {
        let buf = [0x78, 0x56, 0x34, 0x12, 0xAA];
        assert_eq!(u32_at(&buf, 0).unwrap(), 0x1234_5678);
        assert_eq!(u8_at(&buf, 4).unwrap(), 0xAA);
        assert_eq!(slice(&buf, 1, 2).unwrap(), &[0x56, 0x34]);
    }

    #[test]
    fn out_of_bounds_is_typed_error() {
        let buf = [0u8; 3];
        assert!(matches!(u32_at(&buf, 0), Err(CpdError::OutOfBounds { .. })));
        assert!(matches!(u32_at(&buf, 1), Err(CpdError::OutOfBounds { .. })));
        assert!(matches!(u8_at(&buf, 3), Err(CpdError::OutOfBounds { .. })));
        assert!(matches!(slice(&buf, 2, 5), Err(CpdError::OutOfBounds { .. })));
    }

    #[test]
    fn offset_overflow_is_caught() {
        let buf = [0u8; 4];
        assert!(matches!(
            slice(&buf, usize::MAX, 8),
            Err(CpdError::ArithmeticOverflow)
        ));
    }
}
