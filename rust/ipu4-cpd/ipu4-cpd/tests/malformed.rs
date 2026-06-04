//! Fuzz-style boundary tests over malformed / truncated / corrupt CPD inputs.
//!
//! These exercise exactly the bug classes the Rust port eliminates: buffer
//! overreads on truncated buffers, integer-overflow offset math, and
//! out-of-range indices. Every case must return a typed [`CpdError`] instead of
//! reading out of bounds or panicking.

mod common;

use common::{CpdBuilder, FW_PKG_RELEASE};
use ipu4_cpd::{
    build_pkg_dir, validate_cpd_file_with_release, CpdError, CpdFile, Metadata,
};

fn valid() -> Vec<u8> {
    CpdBuilder::new().components(3).build()
}

#[test]
fn baseline_is_valid() {
    assert!(validate_cpd_file_with_release(&valid(), FW_PKG_RELEASE).is_ok());
}

#[test]
fn empty_and_tiny_buffers_never_panic() {
    for len in 0..40usize {
        let buf = vec![0u8; len];
        // Must return an error, never panic / overread.
        let _ = CpdFile::parse(&buf);
        let _ = validate_cpd_file_with_release(&buf, FW_PKG_RELEASE);
    }
}

#[test]
fn truncated_at_every_length_is_safe() {
    let full = valid();
    for cut in 0..full.len() {
        let buf = &full[..cut];
        // The exhaustive truncation sweep must never panic.
        let _ = validate_cpd_file_with_release(buf, FW_PKG_RELEASE);
        if let Ok(cpd) = CpdFile::parse(buf) {
            // If structural parse succeeded, entry access stays in bounds.
            for i in 0..cpd.entry_count() as usize {
                let _ = cpd.entry(i);
            }
        }
    }
}

#[test]
fn bad_header_mark_is_rejected() {
    let buf = CpdBuilder::new().hdr_mark(0xDEAD_BEEF).build();
    let err = validate_cpd_file_with_release(&buf, FW_PKG_RELEASE).unwrap_err();
    assert!(matches!(err, CpdError::BadHeaderMark { .. }), "{err}");
}

#[test]
fn entry_count_overflow_is_rejected() {
    // Header claims a huge entry count that cannot fit in the buffer.
    let mut buf = valid();
    buf[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    let err = CpdFile::parse(&buf).unwrap_err();
    assert!(matches!(err, CpdError::BadEntryCount { .. }), "{err}");
}

#[test]
fn missing_fixed_entry_is_rejected() {
    // Only one entry present, but metadata/moduledata are looked up at idx 1/2.
    let mut buf = valid();
    buf[4..8].copy_from_slice(&1u32.to_le_bytes());
    let cpd = CpdFile::parse(&buf).unwrap();
    assert!(matches!(
        cpd.metadata().unwrap_err(),
        CpdError::MissingEntry { index: 1, .. }
    ));
    assert!(matches!(
        cpd.moduledata().unwrap_err(),
        CpdError::MissingEntry { index: 2, .. }
    ));
}

#[test]
fn entry_offset_out_of_range_is_rejected() {
    // Point the manifest entry (idx 0, offset field at hdr+0*ent + 12) far past EOF.
    let mut buf = valid();
    let manifest_off_field = 16 + 12;
    buf[manifest_off_field..manifest_off_field + 4].copy_from_slice(&0xFFFF_0000u32.to_le_bytes());
    let err = CpdFile::parse(&buf).unwrap_err();
    assert!(matches!(err, CpdError::EntryOutOfRange { .. }), "{err}");
}

#[test]
fn manifest_too_large_is_rejected() {
    // Manifest length within file but above MAX_MANIFEST_SIZE (8192).
    let buf = CpdBuilder::new().manifest_len(8193).build();
    let err = validate_cpd_file_with_release(&buf, FW_PKG_RELEASE).unwrap_err();
    assert!(matches!(err, CpdError::ManifestTooLarge { .. }), "{err}");
}

#[test]
fn bad_metadata_type_is_rejected() {
    let buf = CpdBuilder::new().img_type(99).build();
    let err = validate_cpd_file_with_release(&buf, FW_PKG_RELEASE).unwrap_err();
    assert!(matches!(err, CpdError::InvalidMetadataType { .. }), "{err}");

    let buf = CpdBuilder::new().extn_type(0xFF).build();
    let err = validate_cpd_file_with_release(&buf, FW_PKG_RELEASE).unwrap_err();
    assert!(matches!(err, CpdError::InvalidMetadataType { .. }), "{err}");
}

#[test]
fn fw_pkg_release_mismatch_is_rejected() {
    let buf = CpdBuilder::new().fw_pkg_date(0x1111_2222).build();
    let err = validate_cpd_file_with_release(&buf, FW_PKG_RELEASE).unwrap_err();
    assert!(matches!(err, CpdError::FwPkgReleaseMismatch { .. }), "{err}");
}

#[test]
fn metadata_component_index_is_range_checked() {
    let buf = valid();
    let cpd = CpdFile::parse(&buf).unwrap();
    let meta = cpd.metadata_section().unwrap();
    let count = meta.component_count() as u32;
    let err = meta.component(count).unwrap_err();
    assert!(matches!(err, CpdError::ComponentOutOfRange { .. }), "{err}");
    // Above MAX_COMPONENT_ID is also rejected.
    assert!(matches!(
        meta.component(9999).unwrap_err(),
        CpdError::ComponentOutOfRange { .. }
    ));
}

#[test]
fn short_metadata_is_rejected() {
    // Smaller than the extension header.
    assert!(matches!(
        Metadata::parse(&[0u8; 10]).unwrap_err(),
        CpdError::InvalidMetadataSize { .. }
    ));
}

#[test]
fn pkg_dir_too_many_components_is_rejected() {
    // 16 components exceeds the 15-entry pkg_dir capacity.
    let buf = CpdBuilder::new().components(16).build();
    let cpd = CpdFile::parse(&buf).unwrap();
    let met = cpd.metadata().unwrap();
    let mdl = cpd.moduledata().unwrap();
    let moduledata = cpd.entry_data(&mdl).unwrap();
    let metadata = cpd.entry_data(&met).unwrap();
    let err = build_pkg_dir(moduledata, metadata, 0).unwrap_err();
    assert!(matches!(err, CpdError::TooManyComponents { .. }), "{err}");
}

#[test]
fn pkg_dir_base_addr_overflow_is_rejected() {
    let buf = valid();
    let cpd = CpdFile::parse(&buf).unwrap();
    let met = cpd.metadata().unwrap();
    let mdl = cpd.moduledata().unwrap();
    let moduledata = cpd.entry_data(&mdl).unwrap();
    let metadata = cpd.entry_data(&met).unwrap();
    // base + offset overflows u64.
    let err = build_pkg_dir(moduledata, metadata, u64::MAX).unwrap_err();
    assert!(matches!(err, CpdError::ArithmeticOverflow), "{err}");
}

#[test]
fn pkg_dir_index_is_range_checked() {
    let buf = valid();
    let cpd = CpdFile::parse(&buf).unwrap();
    let met = cpd.metadata().unwrap();
    let mdl = cpd.moduledata().unwrap();
    let moduledata = cpd.entry_data(&mdl).unwrap();
    let metadata = cpd.entry_data(&met).unwrap();
    let pkg = build_pkg_dir(moduledata, metadata, 0x1000).unwrap();
    assert!(matches!(
        pkg.address(999).unwrap_err(),
        CpdError::PkgDirIndexOutOfRange { .. }
    ));
}

/// Pseudo-fuzz: flip bytes across the file and ensure no panic / overread.
#[test]
fn single_byte_corruption_sweep_never_panics() {
    let base = valid();
    for i in 0..base.len() {
        for &v in &[0x00u8, 0xFF, 0x7F, 0x80] {
            let mut buf = base.clone();
            buf[i] = v;
            // Result may be Ok or Err — it must simply never panic / overread.
            if let Ok(cpd) = CpdFile::parse(&buf) {
                let _ = cpd.validate(FW_PKG_RELEASE);
                if let (Ok(met), Ok(mdl)) = (cpd.metadata(), cpd.moduledata()) {
                    if let (Ok(md), Ok(meta)) = (cpd.entry_data(&mdl), cpd.entry_data(&met)) {
                        let _ = build_pkg_dir(md, meta, 0x1000);
                    }
                }
            }
        }
    }
}
