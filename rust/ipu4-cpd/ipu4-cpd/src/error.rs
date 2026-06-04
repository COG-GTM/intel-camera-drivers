//! Error types for the CPD parser.
//!
//! The C code signalled failure with bare `-EINVAL` / `ERR_PTR(...)`. The Rust
//! port replaces those with a typed [`CpdError`] that pinpoints *which* check
//! failed and *why*, while still being convertible back to the kernel `-EINVAL`
//! convention for the C ABI shim.

use core::fmt;

/// Result alias used throughout the crate.
pub type Result<T> = core::result::Result<T, CpdError>;

/// All the ways CPD parsing/validation can fail.
///
/// Each variant corresponds to a `dev_err(...)` / early-return site in the
/// original C, but carries the offending sizes/offsets so callers (and tests)
/// can see exactly what went wrong instead of a single opaque `-EINVAL`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CpdError {
    /// A read of `need` bytes at `offset` would exceed the `len`-byte buffer.
    /// This is the class of bug (buffer overread) the Rust port eliminates.
    OutOfBounds {
        offset: usize,
        need: usize,
        len: usize,
    },
    /// The buffer is smaller than the fixed-size header it must contain.
    BufferTooSmall { need: usize, len: usize },
    /// The CPD header marker (`$CPD`) did not match.
    BadHeaderMark { found: u32, expected: u32 },
    /// The declared entry count does not fit in the buffer.
    BadEntryCount { ent_cnt: u32, capacity: u32 },
    /// A fixed CPD entry index (manifest/metadata/moduledata) is absent.
    MissingEntry { index: usize, ent_cnt: u32 },
    /// A CPD entry points outside the backing data region.
    EntryOutOfRange {
        index: usize,
        offset: u32,
        len: u32,
        data_size: u32,
    },
    /// The manifest section exceeds `MAX_MANIFEST_SIZE`.
    ManifestTooLarge { len: u32, max: u32 },
    /// The metadata section size is invalid (too small / too large / not a
    /// whole number of components).
    InvalidMetadataSize { size: u32 },
    /// The metadata extension/image type did not match the expected IUNIT /
    /// main-firmware values.
    InvalidMetadataType { extn_type: u32, img_type: u32 },
    /// The moduledata header is inconsistent with the moduledata size.
    InvalidModuleDataSize { size: u32, hdr_len: u32 },
    /// The moduledata firmware-package date did not match the linked library.
    FwPkgReleaseMismatch { found: u32, expected: u32 },
    /// A metadata component index is out of range.
    ComponentOutOfRange { idx: u32, count: u32 },
    /// A parsed component id exceeds `MAX_COMPONENT_ID`.
    ComponentIdOutOfRange { id: u32, max: u32 },
    /// A parsed component version exceeds `MAX_COMPONENT_VERSION`.
    ComponentVersionOutOfRange { ver: u32, max: u32 },
    /// Integer overflow occurred while computing an offset/size. The C code
    /// performed this arithmetic on raw `u32`/pointers with no overflow guard.
    ArithmeticOverflow,
    /// The package directory does not contain the requested index.
    PkgDirIndexOutOfRange { index: usize, entries: usize },
    /// More components than the pkg_dir can hold (`MAX_PKG_DIR_ENT_CNT - 1`).
    TooManyComponents { count: u32, max: usize },
}

impl fmt::Display for CpdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpdError::OutOfBounds { offset, need, len } => write!(
                f,
                "out-of-bounds read: {need} bytes at offset {offset} exceeds buffer of {len}"
            ),
            CpdError::BufferTooSmall { need, len } => {
                write!(f, "buffer too small: need {need} bytes, have {len}")
            }
            CpdError::BadHeaderMark { found, expected } => {
                write!(f, "invalid CPD header mark: {found:#x} != {expected:#x}")
            }
            CpdError::BadEntryCount { ent_cnt, capacity } => write!(
                f,
                "invalid CPD header: entry count {ent_cnt} exceeds capacity {capacity}"
            ),
            CpdError::MissingEntry { index, ent_cnt } => {
                write!(f, "missing CPD entry {index}: only {ent_cnt} present")
            }
            CpdError::EntryOutOfRange {
                index,
                offset,
                len,
                data_size,
            } => write!(
                f,
                "CPD entry {index} (offset {offset}, len {len}) out of range for data size {data_size}"
            ),
            CpdError::ManifestTooLarge { len, max } => {
                write!(f, "invalid manifest size: {len} > {max}")
            }
            CpdError::InvalidMetadataSize { size } => {
                write!(f, "invalid metadata size: {size}")
            }
            CpdError::InvalidMetadataType {
                extn_type,
                img_type,
            } => write!(
                f,
                "invalid metadata descriptor: extn_type={extn_type}, img_type={img_type}"
            ),
            CpdError::InvalidModuleDataSize { size, hdr_len } => {
                write!(f, "invalid moduledata size: size={size}, hdr_len={hdr_len}")
            }
            CpdError::FwPkgReleaseMismatch { found, expected } => write!(
                f,
                "moduledata and library version mismatch ({found:#x} != {expected:#x})"
            ),
            CpdError::ComponentOutOfRange { idx, count } => {
                write!(f, "component index out of range ({idx} >= {count})")
            }
            CpdError::ComponentIdOutOfRange { id, max } => {
                write!(f, "component id out of range ({id} > {max})")
            }
            CpdError::ComponentVersionOutOfRange { ver, max } => {
                write!(f, "component version out of range ({ver} > {max})")
            }
            CpdError::ArithmeticOverflow => write!(f, "arithmetic overflow in offset/size math"),
            CpdError::PkgDirIndexOutOfRange { index, entries } => {
                write!(f, "pkg_dir index {index} out of range ({entries} entries)")
            }
            CpdError::TooManyComponents { count, max } => {
                write!(f, "too many components: {count} > {max}")
            }
        }
    }
}

impl std::error::Error for CpdError {}

impl CpdError {
    /// Map to the kernel `-EINVAL` convention used by the C ABI shim.
    ///
    /// Every validation failure in the original driver returned `-EINVAL`, so
    /// the shim collapses the rich error set back to that single code.
    pub const fn to_errno(&self) -> i32 {
        const EINVAL: i32 = 22;
        -EINVAL
    }
}
