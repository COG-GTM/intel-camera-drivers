//! Zero-copy views over the packed CPD structures.
//!
//! Each view borrows a `&[u8]` and decodes fields on demand with the
//! bounds-checked readers in [`crate::bytes`]. No data is copied and no
//! `unsafe` is used: a view is just a validated slice plus typed accessors,
//! mirroring the field layout of the corresponding `__packed` C struct.

use crate::bytes::{slice, u32_at, u8_at};
use crate::consts::*;
use crate::error::Result;
use crate::types::{Offset, Size};

/// View over `struct intel_ipu4_cpd_hdr` (16 bytes).
#[derive(Debug, Clone, Copy)]
pub struct CpdHeader<'a> {
    raw: &'a [u8],
}

impl<'a> CpdHeader<'a> {
    /// Borrow a CPD header from the front of `buf` (requires >= 16 bytes).
    pub fn parse(buf: &'a [u8]) -> Result<Self> {
        let raw = slice(buf, 0, SIZEOF_CPD_HDR)?;
        Ok(Self { raw })
    }

    #[inline]
    pub fn hdr_mark(&self) -> u32 {
        u32_at(self.raw, 0).unwrap()
    }
    #[inline]
    pub fn ent_cnt(&self) -> u32 {
        u32_at(self.raw, 4).unwrap()
    }
    #[inline]
    pub fn hdr_ver(&self) -> u8 {
        u8_at(self.raw, 8).unwrap()
    }
    #[inline]
    pub fn ent_ver(&self) -> u8 {
        u8_at(self.raw, 9).unwrap()
    }
    #[inline]
    pub fn hdr_len(&self) -> u8 {
        u8_at(self.raw, 10).unwrap()
    }
    #[inline]
    pub fn chksm(&self) -> u8 {
        u8_at(self.raw, 11).unwrap()
    }
    #[inline]
    pub fn name(&self) -> u32 {
        u32_at(self.raw, 12).unwrap()
    }
}

/// View over `struct intel_ipu4_cpd_ent` (24 bytes).
#[derive(Debug, Clone, Copy)]
pub struct CpdEntry<'a> {
    raw: &'a [u8],
}

impl<'a> CpdEntry<'a> {
    /// Borrow the CPD entry at `index` from an entry array `buf`.
    pub fn parse_at(buf: &'a [u8], index: usize) -> Result<Self> {
        let base = index
            .checked_mul(SIZEOF_CPD_ENT)
            .ok_or(crate::error::CpdError::ArithmeticOverflow)?;
        let raw = slice(buf, base, SIZEOF_CPD_ENT)?;
        Ok(Self { raw })
    }

    /// The 12-byte component name (may be unterminated / non-UTF8).
    #[inline]
    pub fn name(&self) -> &'a [u8] {
        slice(self.raw, 0, INTEL_NAME_LEN).unwrap()
    }
    #[inline]
    pub fn offset(&self) -> Offset {
        Offset(u32_at(self.raw, 12).unwrap())
    }
    /// The `len` field of the CPD entry (a byte count of the referenced region).
    #[inline]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> Size {
        Size(u32_at(self.raw, 16).unwrap())
    }
}

const INTEL_NAME_LEN: usize = 12;

/// View over `struct intel_ipu_cpd_metadata_extn` (28 bytes).
#[derive(Debug, Clone, Copy)]
pub struct MetadataExtn<'a> {
    raw: &'a [u8],
}

impl<'a> MetadataExtn<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<Self> {
        let raw = slice(buf, 0, SIZEOF_METADATA_EXTN)?;
        Ok(Self { raw })
    }
    #[inline]
    pub fn extn_type(&self) -> u32 {
        u32_at(self.raw, 0).unwrap()
    }
    /// The `len` field of the extension header.
    #[inline]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u32 {
        u32_at(self.raw, 4).unwrap()
    }
    #[inline]
    pub fn img_type(&self) -> u32 {
        u32_at(self.raw, 8).unwrap()
    }
}

/// View over `struct intel_ipu4_cpd_metadata_cmpnt` (68 bytes).
#[derive(Debug, Clone, Copy)]
pub struct MetadataComponent<'a> {
    raw: &'a [u8],
}

impl<'a> MetadataComponent<'a> {
    /// Borrow the component at `index` within the component array `buf`.
    pub fn parse_at(buf: &'a [u8], index: usize) -> Result<Self> {
        let base = index
            .checked_mul(SIZEOF_METADATA_CMPNT)
            .ok_or(crate::error::CpdError::ArithmeticOverflow)?;
        let raw = slice(buf, base, SIZEOF_METADATA_CMPNT)?;
        Ok(Self { raw })
    }
    #[inline]
    pub fn id(&self) -> u32 {
        u32_at(self.raw, 0).unwrap()
    }
    #[inline]
    pub fn size(&self) -> u32 {
        u32_at(self.raw, 4).unwrap()
    }
    #[inline]
    pub fn ver(&self) -> u32 {
        u32_at(self.raw, 8).unwrap()
    }
    /// The 32-byte SHA-2 hash of the component.
    #[inline]
    pub fn sha2_hash(&self) -> &'a [u8] {
        slice(self.raw, 12, 32).unwrap()
    }
    #[inline]
    pub fn entry_point(&self) -> u32 {
        u32_at(self.raw, 44).unwrap()
    }
    #[inline]
    pub fn icache_base_offs(&self) -> u32 {
        u32_at(self.raw, 48).unwrap()
    }
}

/// View over `struct intel_ipu4_cpd_module_data_hdr` (44 bytes).
#[derive(Debug, Clone, Copy)]
pub struct ModuleDataHeader<'a> {
    raw: &'a [u8],
}

impl<'a> ModuleDataHeader<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<Self> {
        let raw = slice(buf, 0, SIZEOF_MODULE_DATA_HDR)?;
        Ok(Self { raw })
    }
    #[inline]
    pub fn hdr_len(&self) -> u32 {
        u32_at(self.raw, 0).unwrap()
    }
    #[inline]
    pub fn fw_pkg_date(&self) -> u32 {
        u32_at(self.raw, 8).unwrap()
    }
}
