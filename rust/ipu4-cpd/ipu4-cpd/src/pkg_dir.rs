//! Package-directory (pkg_dir) construction and accessors.
//!
//! This is the safe port of `intel_ipu4_cpd_parse_module_data` plus the
//! `intel_ipu4_cpd_pkg_dir_get_*` accessors. The kernel original wrote the
//! pkg_dir into a DMA-allocated buffer; here we build an owned `Vec<u64>` of
//! exactly the right length and hand it back. All DMA/allocation concerns are
//! left to the caller — this module is pure, safe binary-structure assembly.

use crate::bytes::slice;
use crate::consts::*;
use crate::error::{CpdError, Result};
use crate::parser::Metadata;
use crate::view::{CpdHeader, ModuleDataHeader};

/// An assembled package directory: a sequence of 64-bit words laid out exactly
/// like the kernel `pkg_dir` (2 qwords per header/entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkgDir {
    words: Vec<u64>,
}

impl PkgDir {
    /// The raw 64-bit words backing this directory.
    #[inline]
    pub fn words(&self) -> &[u64] {
        &self.words
    }

    /// Consume and return the backing words.
    #[inline]
    pub fn into_words(self) -> Vec<u64> {
        self.words
    }

    /// Number of entries recorded in the header — port of
    /// `intel_ipu4_cpd_pkg_dir_get_num_entries` (`pkg_dir[1]`).
    #[inline]
    pub fn num_entries(&self) -> u64 {
        self.words[1]
    }

    /// Index into the entry-pair for component `pkg_dir_idx`, range-checked.
    ///
    /// Mirrors the C `++pkg_dir_idx * PKG_DIR_ENT_LEN` addressing: component
    /// `idx` lives at word `(idx + 1) * 2`.
    fn entry_word_index(&self, pkg_dir_idx: usize) -> Result<usize> {
        let base = pkg_dir_idx
            .checked_add(1)
            .and_then(|v| v.checked_mul(PKG_DIR_ENT_LEN))
            .ok_or(CpdError::ArithmeticOverflow)?;
        // Need words [base, base + 1].
        if base + 1 >= self.words.len() {
            return Err(CpdError::PkgDirIndexOutOfRange {
                index: pkg_dir_idx,
                entries: self.words.len() / PKG_DIR_ENT_LEN,
            });
        }
        Ok(base)
    }

    /// Port of `intel_ipu4_cpd_pkg_dir_get_address` (full 64-bit value).
    pub fn address(&self, pkg_dir_idx: usize) -> Result<u64> {
        let base = self.entry_word_index(pkg_dir_idx)?;
        Ok(self.words[base])
    }

    /// Port of `intel_ipu4_cpd_pkg_dir_get_size`
    /// (`pkg_dir[...+1] & PKG_DIR_SIZE_MASK`).
    pub fn size(&self, pkg_dir_idx: usize) -> Result<u64> {
        let base = self.entry_word_index(pkg_dir_idx)?;
        Ok(self.words[base + 1] & PKG_DIR_SIZE_MASK)
    }

    /// Port of `intel_ipu4_cpd_pkg_dir_get_type`
    /// (`pkg_dir[...+1] >> PKG_DIR_ID_SHIFT & PKG_DIR_ID_MASK`).
    pub fn type_of(&self, pkg_dir_idx: usize) -> Result<u64> {
        let base = self.entry_word_index(pkg_dir_idx)?;
        Ok((self.words[base + 1] >> PKG_DIR_ID_SHIFT) & PKG_DIR_ID_MASK)
    }
}

/// Build a package directory from a moduledata section and its metadata —
/// the safe port of `intel_ipu4_cpd_parse_module_data`.
///
/// * `module_data` — the moduledata CPD entry payload.
/// * `metadata` — the metadata CPD entry payload (component table).
/// * `base_addr` — the device address of `module_data` (the C
///   `dma_addr_module_data`); component addresses are computed relative to it.
///
/// # Memory safety vs. C
/// * The embedded directory header is bounds-checked before its entries are
///   walked (the C code dereferenced `dir_hdr`/`dir_ent` straight from
///   pointer arithmetic).
/// * `base_addr + offset` is overflow-checked.
/// * The entry count is capped at `MAX_PKG_DIR_ENT_CNT - 1`, matching the fixed
///   buffer the kernel allocated — preventing the write past the end the C code
///   would suffer with a hostile `ent_cnt`.
pub fn build_pkg_dir(module_data: &[u8], metadata: &[u8], base_addr: u64) -> Result<PkgDir> {
    let mod_hdr = ModuleDataHeader::parse(module_data)?;
    let hdr_len = mod_hdr.hdr_len() as usize;

    // The embedded directory header sits right after the moduledata header.
    let dir_region = slice(module_data, hdr_len, module_data.len().saturating_sub(hdr_len))?;
    let dir_hdr = CpdHeader::parse(dir_region)?;
    let ent_cnt = dir_hdr.ent_cnt();

    // The kernel buffer holds MAX_PKG_DIR_ENT_CNT entry pairs *including* the
    // header pair, so at most MAX_PKG_DIR_ENT_CNT - 1 components fit.
    let max_components = MAX_PKG_DIR_ENT_CNT - 1;
    if ent_cnt as usize > max_components {
        return Err(CpdError::TooManyComponents {
            count: ent_cnt,
            max: max_components,
        });
    }

    // The directory entry table follows the embedded header.
    let entries = slice(dir_region, SIZEOF_CPD_HDR, ent_cnt as usize * SIZEOF_CPD_ENT)?;

    let meta = Metadata::parse(metadata)?;

    let mut words = vec![0u64; (1 + ent_cnt as usize) * PKG_DIR_ENT_LEN];
    words[0] = PKG_DIR_HDR_MARK;
    // pkg_dir entry count = component count + pkg_dir header
    words[1] = ent_cnt as u64 + 1;

    for i in 0..ent_cnt as usize {
        let ent = crate::view::CpdEntry::parse_at(entries, i)?;

        let id = meta.component(i as u32)?.id();
        if id > MAX_COMPONENT_ID {
            return Err(CpdError::ComponentIdOutOfRange {
                id,
                max: MAX_COMPONENT_ID,
            });
        }
        let ver = meta.component(i as u32)?.ver();
        if ver > MAX_COMPONENT_VERSION {
            return Err(CpdError::ComponentVersionOutOfRange {
                ver,
                max: MAX_COMPONENT_VERSION,
            });
        }

        let addr = base_addr
            .checked_add(ent.offset().get() as u64)
            .ok_or(CpdError::ArithmeticOverflow)?;

        let pair = PKG_DIR_ENT_LEN + i * PKG_DIR_ENT_LEN;
        words[pair] = addr;
        // 63:56 rsvd | 55 rsvd | 54:48 type(id) | 47:32 version | 31:24 rsvd | 23:0 size
        words[pair + 1] = ent.len().get() as u64
            | (id as u64) << PKG_DIR_ID_SHIFT
            | (ver as u64) << PKG_DIR_VERSION_SHIFT;
    }

    Ok(PkgDir { words })
}
