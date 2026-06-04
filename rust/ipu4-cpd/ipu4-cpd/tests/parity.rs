//! Golden-file parity test.
//!
//! Parses a committed sample CPD binary (`tests/data/sample_cpd.bin`) and
//! asserts field-by-field that the safe parser recovers exactly the values that
//! were encoded into it by the independent [`CpdBuilder`] reference.
//!
//! To (re)generate the golden file after an intentional format change, run:
//! `cargo test -p ipu4-cpd --test parity -- --ignored generate_golden`

mod common;

use common::{CpdBuilder, FW_PKG_RELEASE};
use ipu4_cpd::{build_pkg_dir, validate_cpd_file_with_release, CpdFile};
use std::path::PathBuf;

/// The exact configuration encoded in the committed golden file.
fn golden_builder() -> CpdBuilder {
    CpdBuilder::new()
        .components(3)
        .manifest_len(64)
        .payload_size(32)
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/sample_cpd.bin")
}

#[test]
#[ignore = "regenerates the committed golden file on demand"]
fn generate_golden() {
    let bytes = golden_builder().build();
    std::fs::create_dir_all(golden_path().parent().unwrap()).unwrap();
    std::fs::write(golden_path(), &bytes).unwrap();
    eprintln!("wrote {} bytes to {}", bytes.len(), golden_path().display());
}

#[test]
fn golden_file_matches_builder() {
    let on_disk = std::fs::read(golden_path()).expect("golden file present");
    let expected = golden_builder().build();
    assert_eq!(
        on_disk, expected,
        "committed golden file is stale; rerun the `generate_golden` ignored test"
    );
}

#[test]
fn golden_file_parses_field_by_field() {
    let b = golden_builder();
    let bytes = std::fs::read(golden_path()).expect("golden file present");

    // Whole-file validation passes.
    validate_cpd_file_with_release(&bytes, FW_PKG_RELEASE).expect("golden validates");

    let cpd = CpdFile::parse(&bytes).expect("parse");
    assert_eq!(cpd.header().hdr_mark(), ipu4_cpd::CPD_HDR_MARK);
    assert_eq!(cpd.entry_count(), 3);

    // Entry table integrity.
    let man = cpd.manifest().unwrap();
    let met = cpd.metadata().unwrap();
    let mdl = cpd.moduledata().unwrap();
    assert_eq!(man.len().get() as usize, 64);
    assert!(cpd.entry_data(&man).is_ok());
    assert!(cpd.entry_data(&met).is_ok());
    assert!(cpd.entry_data(&mdl).is_ok());

    // Metadata component table parity.
    let meta = cpd.metadata_section().unwrap();
    assert_eq!(meta.component_count(), b.component_count());
    for i in 0..b.component_count() {
        let c = meta.component(i as u32).unwrap();
        assert_eq!(c.id(), b.component_id(i), "id[{i}]");
        assert_eq!(c.ver(), b.component_ver(i), "ver[{i}]");
        assert_eq!(c.size(), b.component_size(), "size[{i}]");
        assert_eq!(c.entry_point(), b.component_entry_point(i), "entry_point[{i}]");
        assert_eq!(c.icache_base_offs(), b.component_icache(i), "icache[{i}]");

        // Convenience accessors agree with direct component reads.
        assert_eq!(cpd.pg_icache_base(i as u32).unwrap(), b.component_icache(i));
        assert_eq!(
            cpd.pg_entry_point(i as u32).unwrap(),
            b.component_entry_point(i)
        );
    }

    // pkg_dir assembly parity.
    let base_addr: u64 = 0x1_0000_0000;
    let moduledata = cpd.entry_data(&mdl).unwrap();
    let metadata = cpd.entry_data(&met).unwrap();
    let pkg = build_pkg_dir(moduledata, metadata, base_addr).unwrap();

    assert_eq!(pkg.num_entries(), b.component_count() as u64 + 1);
    for i in 0..b.component_count() {
        let expected_addr = base_addr + b.moduledata_payload_offset(i) as u64;
        assert_eq!(pkg.address(i).unwrap(), expected_addr, "addr[{i}]");
        assert_eq!(pkg.size(i).unwrap(), b.component_size() as u64, "size[{i}]");
        assert_eq!(pkg.type_of(i).unwrap(), b.component_id(i) as u64, "type[{i}]");
    }
}
