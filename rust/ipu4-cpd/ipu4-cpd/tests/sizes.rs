//! Verify the safe crate's on-wire size constants match the `#[repr(C, packed)]`
//! struct layouts in the `ipu4-cpd-sys` FFI bindings (which in turn mirror the
//! C `__packed` structs). This keeps the parser's offset math anchored to the
//! authoritative ABI definition.

use core::mem::size_of;
use ipu4_cpd::consts::*;
use ipu4_cpd_sys as sys;

#[test]
fn safe_consts_match_ffi_struct_sizes() {
    assert_eq!(SIZEOF_CPD_HDR, size_of::<sys::intel_ipu4_cpd_hdr>());
    assert_eq!(SIZEOF_CPD_ENT, size_of::<sys::intel_ipu4_cpd_ent>());
    assert_eq!(
        SIZEOF_METADATA_EXTN,
        size_of::<sys::intel_ipu_cpd_metadata_extn>()
    );
    assert_eq!(
        SIZEOF_METADATA_CMPNT,
        size_of::<sys::intel_ipu4_cpd_metadata_cmpnt>()
    );
    assert_eq!(
        SIZEOF_MODULE_DATA_HDR,
        size_of::<sys::intel_ipu4_cpd_module_data_hdr>()
    );
}

#[test]
fn constants_match_c_defines() {
    assert_eq!(CPD_HDR_MARK, 0x4450_4324);
    assert_eq!(PKG_DIR_HDR_MARK, 0x5f49_5550_4b44_525f);
    assert_eq!(PKG_DIR_SIZE, 256);
    assert_eq!(MAX_MANIFEST_SIZE, 8192);
    assert_eq!(MAX_METADATA_SIZE, 65536);
    assert_eq!(MAX_COMPONENT_ID, 127);
    assert_eq!(MAX_COMPONENT_VERSION, 0xffff);
    assert_eq!(
        METADATA_EXTN_TYPE_IUNIT,
        sys::INTEL_IPU4_CPD_METADATA_EXTN_TYPE_IUNIT
    );
    assert_eq!(
        METADATA_IMAGE_TYPE_MAIN_FIRMWARE,
        sys::INTEL_IPU4_CPD_METADATA_IMAGE_TYPE_MAIN_FIRMWARE
    );
}
