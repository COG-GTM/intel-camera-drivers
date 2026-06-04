//! The safe CPD parser core.
//!
//! This module ports the validation and walking logic from `intel-ipu4-cpd.c`.
//! It contains **no `unsafe`**: every read goes through the bounds-checked
//! helpers in [`crate::bytes`], every offset/size add is overflow-checked, and
//! fixed entry indices (manifest/metadata/moduledata) are range-checked instead
//! of being blindly dereferenced as the C macros did.

use crate::bytes::slice;
use crate::consts::*;
use crate::error::{CpdError, Result};
use crate::types::Size;
use crate::view::{CpdEntry, CpdHeader, MetadataComponent, MetadataExtn, ModuleDataHeader};

/// Structural validation of a CPD region — the port of `intel_ipu4_cpd_validate_cpd`.
///
/// `cpd` is the region that must contain the header followed by the entry table
/// (its length plays the role of the C `cpd_size`). `data_size` is the size that
/// each entry's `offset`/`len` is range-checked against (the C `data_size`).
///
/// Returns the validated entry count on success.
///
/// # Memory safety vs. C
/// The C version computed `data_size - ent->offset` *after* checking
/// `data_size < ent->offset`; here the subtraction is additionally guarded so a
/// malformed `offset`/`len` can never wrap.
pub fn validate_cpd(cpd: &[u8], data_size: u32) -> Result<u32> {
    // Ensure the CPD header is present.
    if cpd.len() < SIZEOF_CPD_HDR {
        return Err(CpdError::BufferTooSmall {
            need: SIZEOF_CPD_HDR,
            len: cpd.len(),
        });
    }

    let hdr = CpdHeader::parse(cpd)?;
    let ent_cnt = hdr.ent_cnt();

    // Ensure the declared entry table fits in the region.
    let capacity = (cpd.len() - SIZEOF_CPD_HDR) / SIZEOF_CPD_ENT;
    if (capacity as u64) < ent_cnt as u64 {
        return Err(CpdError::BadEntryCount {
            ent_cnt,
            capacity: capacity as u32,
        });
    }

    // Ensure every entry points within the backing data region.
    let entries = slice(cpd, SIZEOF_CPD_HDR, ent_cnt as usize * SIZEOF_CPD_ENT)?;
    for i in 0..ent_cnt as usize {
        let ent = CpdEntry::parse_at(entries, i)?;
        let offset = ent.offset().get();
        let len = ent.len().get();
        // C: `data_size < offset || data_size - offset < len`
        if data_size < offset || data_size - offset < len {
            return Err(CpdError::EntryOutOfRange {
                index: i,
                offset,
                len,
                data_size,
            });
        }
    }

    Ok(ent_cnt)
}

/// Port of `intel_ipu4_cpd_validate_metadata`.
pub fn validate_metadata(metadata: &[u8]) -> Result<()> {
    let size = metadata.len() as u32;

    if (size as usize) < SIZEOF_METADATA_EXTN || size > MAX_METADATA_SIZE {
        return Err(CpdError::InvalidMetadataSize { size });
    }

    let extn = MetadataExtn::parse(metadata)?;
    if extn.extn_type() != METADATA_EXTN_TYPE_IUNIT
        || extn.img_type() != METADATA_IMAGE_TYPE_MAIN_FIRMWARE
    {
        return Err(CpdError::InvalidMetadataType {
            extn_type: extn.extn_type(),
            img_type: extn.img_type(),
        });
    }

    if (size as usize - SIZEOF_METADATA_EXTN) % SIZEOF_METADATA_CMPNT != 0 {
        return Err(CpdError::InvalidMetadataSize { size });
    }

    Ok(())
}

/// Port of `intel_ipu4_cpd_validate_moduledata`.
///
/// `expected_fw_pkg_release` is the value the C code compared against the
/// compile-time `IA_CSS_FW_PKG_RELEASE` constant; it is a parameter here so the
/// parser stays self-contained and testable.
pub fn validate_moduledata(moduledata: &[u8], expected_fw_pkg_release: u32) -> Result<()> {
    let size = moduledata.len() as u32;

    if (size as usize) < SIZEOF_MODULE_DATA_HDR {
        return Err(CpdError::InvalidModuleDataSize { size, hdr_len: 0 });
    }
    let mod_hdr = ModuleDataHeader::parse(moduledata)?;
    let hdr_len = mod_hdr.hdr_len();
    if size < hdr_len {
        return Err(CpdError::InvalidModuleDataSize { size, hdr_len });
    }

    if mod_hdr.fw_pkg_date() != expected_fw_pkg_release {
        return Err(CpdError::FwPkgReleaseMismatch {
            found: mod_hdr.fw_pkg_date(),
            expected: expected_fw_pkg_release,
        });
    }

    // Validate the embedded CPD directory that follows the moduledata header.
    let inner = slice(moduledata, hdr_len as usize, (size - hdr_len) as usize)?;
    validate_cpd(inner, size)?;

    Ok(())
}

/// Convenience free function: the full port of
/// `intel_ipu4_cpd_validate_cpd_file`.
///
/// Parses the file structurally and then runs the header-mark / manifest /
/// metadata / moduledata checks, returning `Ok(())` iff the C function would
/// have returned `0`.
pub fn validate_cpd_file_with_release(buf: &[u8], expected_fw_pkg_release: u32) -> Result<()> {
    CpdFile::parse(buf)?.validate(expected_fw_pkg_release)
}

/// A validated handle over a complete CPD file.
///
/// Construction runs the structural [`validate_cpd`] pass; the richer
/// [`CpdFile::validate`] performs the full equivalent of
/// `intel_ipu4_cpd_validate_cpd_file`.
#[derive(Debug, Clone, Copy)]
pub struct CpdFile<'a> {
    buf: &'a [u8],
    header: CpdHeader<'a>,
}

impl<'a> CpdFile<'a> {
    /// Parse and structurally validate a CPD file from a byte slice.
    pub fn parse(buf: &'a [u8]) -> Result<Self> {
        validate_cpd(buf, buf.len() as u32)?;
        let header = CpdHeader::parse(buf)?;
        Ok(Self { buf, header })
    }

    /// The CPD header view.
    #[inline]
    pub fn header(&self) -> CpdHeader<'a> {
        self.header
    }

    /// Number of CPD entries declared in the header.
    #[inline]
    pub fn entry_count(&self) -> u32 {
        self.header.ent_cnt()
    }

    /// Borrow the entry at `index`, range-checked against the entry count.
    ///
    /// The C code accessed entries 0/1/2 via macros without ever checking that
    /// `ent_cnt` was large enough — a latent overread. Here a missing entry is
    /// a typed error.
    pub fn entry(&self, index: usize) -> Result<CpdEntry<'a>> {
        if index as u64 >= self.header.ent_cnt() as u64 {
            return Err(CpdError::MissingEntry {
                index,
                ent_cnt: self.header.ent_cnt(),
            });
        }
        let entries = slice(
            self.buf,
            SIZEOF_CPD_HDR,
            self.header.ent_cnt() as usize * SIZEOF_CPD_ENT,
        )?;
        CpdEntry::parse_at(entries, index)
    }

    /// Iterate over all CPD entries.
    pub fn entries(&self) -> impl Iterator<Item = CpdEntry<'a>> + '_ {
        (0..self.header.ent_cnt() as usize).map(move |i| self.entry(i).expect("index < ent_cnt"))
    }

    /// The manifest entry (index 0).
    pub fn manifest(&self) -> Result<CpdEntry<'a>> {
        self.entry(CPD_MANIFEST_IDX)
    }

    /// The metadata entry (index 1).
    pub fn metadata(&self) -> Result<CpdEntry<'a>> {
        self.entry(CPD_METADATA_IDX)
    }

    /// The moduledata entry (index 2).
    pub fn moduledata(&self) -> Result<CpdEntry<'a>> {
        self.entry(CPD_MODULEDATA_IDX)
    }

    /// Borrow the bytes a given entry references within the file (zero-copy).
    pub fn entry_data(&self, ent: &CpdEntry<'a>) -> Result<&'a [u8]> {
        slice(self.buf, ent.offset().as_usize(), ent.len().as_usize())
    }

    /// Full validation — the port of `intel_ipu4_cpd_validate_cpd_file`.
    pub fn validate(&self, expected_fw_pkg_release: u32) -> Result<()> {
        if self.header.hdr_mark() != CPD_HDR_MARK {
            return Err(CpdError::BadHeaderMark {
                found: self.header.hdr_mark(),
                expected: CPD_HDR_MARK,
            });
        }

        let man = self.manifest()?;
        if man.len() > Size(MAX_MANIFEST_SIZE) {
            return Err(CpdError::ManifestTooLarge {
                len: man.len().get(),
                max: MAX_MANIFEST_SIZE,
            });
        }

        let met = self.metadata()?;
        validate_metadata(self.entry_data(&met)?)?;

        let mdl = self.moduledata()?;
        validate_moduledata(self.entry_data(&mdl)?, expected_fw_pkg_release)?;

        Ok(())
    }

    /// The metadata section as a [`Metadata`] view.
    pub fn metadata_section(&self) -> Result<Metadata<'a>> {
        let met = self.metadata()?;
        Metadata::parse(self.entry_data(&met)?)
    }

    /// Port of `intel_ipu4_cpd_get_pg_icache_base`.
    pub fn pg_icache_base(&self, idx: u32) -> Result<u32> {
        Ok(self.metadata_section()?.component(idx)?.icache_base_offs())
    }

    /// Port of `intel_ipu4_cpd_get_pg_entry_point`.
    pub fn pg_entry_point(&self, idx: u32) -> Result<u32> {
        Ok(self.metadata_section()?.component(idx)?.entry_point())
    }
}

/// A view over a metadata section (extension header + component array).
#[derive(Debug, Clone, Copy)]
pub struct Metadata<'a> {
    buf: &'a [u8],
}

impl<'a> Metadata<'a> {
    /// Borrow a metadata section (must contain at least the extension header).
    pub fn parse(buf: &'a [u8]) -> Result<Self> {
        if buf.len() < SIZEOF_METADATA_EXTN {
            return Err(CpdError::InvalidMetadataSize {
                size: buf.len() as u32,
            });
        }
        Ok(Self { buf })
    }

    /// The extension header.
    pub fn extn(&self) -> MetadataExtn<'a> {
        MetadataExtn::parse(self.buf).expect("len checked in parse")
    }

    /// Number of metadata components — the port of the C `cmpnt_count`.
    pub fn component_count(&self) -> usize {
        (self.buf.len() - SIZEOF_METADATA_EXTN) / SIZEOF_METADATA_CMPNT
    }

    /// Borrow the component at `idx` — the port of
    /// `intel_ipu4_cpd_metadata_get_cmpnt`, with range checks preserved.
    pub fn component(&self, idx: u32) -> Result<MetadataComponent<'a>> {
        let count = self.component_count();
        if idx > MAX_COMPONENT_ID || idx as usize >= count {
            return Err(CpdError::ComponentOutOfRange {
                idx,
                count: count as u32,
            });
        }
        let cmpnts = slice(
            self.buf,
            SIZEOF_METADATA_EXTN,
            count * SIZEOF_METADATA_CMPNT,
        )?;
        MetadataComponent::parse_at(cmpnts, idx as usize)
    }
}
