//! Safe, bounds-checked little-endian cursors over byte slices.
//!
//! These replace the C driver's `(struct foo *)buf` pointer casts. Every read
//! and write is bounds-checked, so misaligned access and buffer overruns are
//! impossible: a short buffer yields an [`Error`] instead of undefined
//! behaviour. All multi-byte values use little-endian encoding to match the
//! firmware ABI on the target platform.

use crate::error::{Error, Result};

/// Append-only writer over a fixed-size mutable byte slice.
pub(crate) struct Writer<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> Writer<'a> {
    #[inline]
    pub(crate) fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    #[inline]
    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    #[inline]
    fn take(&mut self, n: usize) -> Result<&mut [u8]> {
        let end = self.pos + n;
        if end > self.buf.len() {
            return Err(Error::BufferTooSmall {
                expected: end,
                actual: self.buf.len(),
            });
        }
        let slice = &mut self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// Write `n` zero padding bytes.
    #[inline]
    pub(crate) fn pad(&mut self, n: usize) -> Result<()> {
        for b in self.take(n)? {
            *b = 0;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn u8(&mut self, v: u8) -> Result<()> {
        self.take(1)?[0] = v;
        Ok(())
    }

    #[inline]
    pub(crate) fn u32(&mut self, v: u32) -> Result<()> {
        self.take(4)?.copy_from_slice(&v.to_le_bytes());
        Ok(())
    }

    #[inline]
    pub(crate) fn i32(&mut self, v: i32) -> Result<()> {
        self.take(4)?.copy_from_slice(&v.to_le_bytes());
        Ok(())
    }

    #[inline]
    pub(crate) fn u64(&mut self, v: u64) -> Result<()> {
        self.take(8)?.copy_from_slice(&v.to_le_bytes());
        Ok(())
    }
}

/// Bounds-checked sequential reader over an immutable byte slice.
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    #[inline]
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    #[inline]
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos + n;
        if end > self.buf.len() {
            return Err(Error::BufferTooSmall {
                expected: end,
                actual: self.buf.len(),
            });
        }
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// Skip `n` bytes (e.g. ABI padding).
    #[inline]
    pub(crate) fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    #[inline]
    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    #[inline]
    pub(crate) fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    #[inline]
    pub(crate) fn i32(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    #[inline]
    pub(crate) fn u64(&mut self) -> Result<u64> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
}
