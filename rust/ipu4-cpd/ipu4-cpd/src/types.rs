//! Strong newtypes for offsets and sizes.
//!
//! The C code passed bare `u32`/`unsigned` values around for both byte offsets
//! and byte lengths, which made it easy to mix them up and to perform
//! unchecked `offset + len` arithmetic. These newtypes keep the two concepts
//! distinct and route all arithmetic through overflow-checked helpers.

use crate::error::{CpdError, Result};

/// A byte offset into a CPD buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Offset(pub u32);

/// A byte size/length of a CPD region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Size(pub u32);

impl Offset {
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl Size {
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for Offset {
    #[inline]
    fn from(v: u32) -> Self {
        Offset(v)
    }
}

impl From<u32> for Size {
    #[inline]
    fn from(v: u32) -> Self {
        Size(v)
    }
}

/// Compute `offset + len` as a `usize`, returning [`CpdError::ArithmeticOverflow`]
/// on wraparound. This is the overflow guard the original C `offset + len`
/// expressions lacked.
#[inline]
pub fn end_of(offset: Offset, len: Size) -> Result<usize> {
    (offset.as_usize())
        .checked_add(len.as_usize())
        .ok_or(CpdError::ArithmeticOverflow)
}
