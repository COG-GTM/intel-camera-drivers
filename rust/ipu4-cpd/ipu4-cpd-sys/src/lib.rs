//! Raw `unsafe extern "C"` FFI bindings for the Intel IPU4 CPD
//! (Compact Package Directory) firmware-package parser.
//!
//! This crate faithfully mirrors the on-disk / in-memory C structures and the
//! public function signatures declared in
//! `drivers/media/pci/intel-ipu4/intel-ipu4-cpd.{c,h}`.
//!
//! It contains **only** declarations — no logic. The safe, idiomatic parser
//! lives in the sibling [`ipu4-cpd`] crate, which is what callers should use.
//! These bindings exist so the layout/ABI of the original C types is captured
//! in one place and can be referenced (and statically size-checked) by the
//! safe crate's parity tests.
//!
//! # Safety
//!
//! Every item in this crate is a thin mirror of a C definition. The `extern "C"`
//! functions are `unsafe` to call: they take raw pointers into kernel-allocated
//! buffers and assume the kernel's invariants hold. The whole point of the
//! `ipu4-cpd` crate is to replace these with bounds-checked, slice-based APIs.

#![allow(non_camel_case_types)]

use core::ffi::{c_int, c_uint, c_ulong, c_void};

// ---------------------------------------------------------------------------
// Sizing constants (mirrors of the `#define`s in intel-ipu4-cpd.h)
// ---------------------------------------------------------------------------

pub const INTEL_IPU4_CPD_SIZE_OF_FW_ARCH_VERSION: usize = 7;
pub const INTEL_IPU4_CPD_SIZE_OF_SYSTEM_VERSION: usize = 11;
pub const INTEL_IPU4_CPD_SIZE_OF_COMPONENT_NAME: usize = 12;

pub const INTEL_IPU4_CPD_METADATA_EXTN_TYPE_IUNIT: u32 = 0x10;

pub const INTEL_IPU4_CPD_METADATA_IMAGE_TYPE_RESERVED: u32 = 0;
pub const INTEL_IPU4_CPD_METADATA_IMAGE_TYPE_BOOTLOADER: u32 = 1;
pub const INTEL_IPU4_CPD_METADATA_IMAGE_TYPE_MAIN_FIRMWARE: u32 = 2;

pub const INTEL_IPU4_CPD_PKG_DIR_PSYS_SERVER_IDX: u32 = 0;
pub const INTEL_IPU4_CPD_PKG_DIR_ISYS_SERVER_IDX: u32 = 1;

pub const INTEL_IPU4_CPD_PKG_DIR_CLIENT_PG_TYPE: u32 = 3;

// ---------------------------------------------------------------------------
// Kernel type aliases / opaque types
// ---------------------------------------------------------------------------

/// Kernel `dma_addr_t`. On 64-bit IPU4 builds this is a 64-bit physical
/// address. Mirrored here so the function signatures below are faithful.
pub type dma_addr_t = u64;

/// Opaque mirror of `struct intel_ipu4_bus_device`.
#[repr(C)]
pub struct intel_ipu4_bus_device {
    _private: [u8; 0],
}

/// Opaque mirror of `struct intel_ipu4_device`.
#[repr(C)]
pub struct intel_ipu4_device {
    _private: [u8; 0],
}

// ---------------------------------------------------------------------------
// Packed on-the-wire structures (mirrors of the `__packed` C structs)
// ---------------------------------------------------------------------------

/// Mirror of `struct __packed intel_ipu4_cpd_module_data_hdr` (44 bytes).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct intel_ipu4_cpd_module_data_hdr {
    pub hdr_len: u32,
    pub endian: u32,
    pub fw_pkg_date: u32,
    pub hive_sdk_date: u32,
    pub compiler_date: u32,
    pub target_platform_type: u32,
    pub sys_ver: [u8; INTEL_IPU4_CPD_SIZE_OF_SYSTEM_VERSION],
    pub fw_arch_ver: [u8; INTEL_IPU4_CPD_SIZE_OF_FW_ARCH_VERSION],
    pub rsvd: [u8; 2],
}

/// Mirror of `struct __packed intel_ipu4_cpd_hdr` (16 bytes).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct intel_ipu4_cpd_hdr {
    pub hdr_mark: u32,
    pub ent_cnt: u32,
    pub hdr_ver: u8,
    pub ent_ver: u8,
    pub hdr_len: u8,
    pub chksm: u8,
    pub name: u32,
}

/// Mirror of `struct __packed intel_ipu4_cpd_ent` (24 bytes).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct intel_ipu4_cpd_ent {
    pub name: [u8; INTEL_IPU4_CPD_SIZE_OF_COMPONENT_NAME],
    pub offset: u32,
    pub len: u32,
    pub rsvd: [u8; 4],
}

/// Mirror of `struct __packed intel_ipu4_cpd_metadata_cmpnt` (68 bytes).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct intel_ipu4_cpd_metadata_cmpnt {
    pub id: u32,
    pub size: u32,
    pub ver: u32,
    pub sha2_hash: [u8; 32],
    pub entry_point: u32,
    pub icache_base_offs: u32,
    pub attrs: [u8; 16],
}

/// Mirror of `struct __packed intel_ipu_cpd_metadata_extn` (28 bytes).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct intel_ipu_cpd_metadata_extn {
    pub extn_type: u32,
    pub len: u32,
    pub img_type: u32,
    pub rsvd: [u8; 16],
}

/// Mirror of `struct __packed intel_ipu_cpd_client_pkg_hdr` (32 bytes).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct intel_ipu_cpd_client_pkg_hdr {
    pub prog_list_offs: u32,
    pub prog_list_size: u32,
    pub prog_desc_offs: u32,
    pub prog_desc_size: u32,
    pub pg_manifest_offs: u32,
    pub pg_manifest_size: u32,
    pub prog_bin_offs: u32,
    pub prog_bin_size: u32,
}

// ---------------------------------------------------------------------------
// Function signatures (mirrors of the exported C ABI in intel-ipu4-cpd.h)
// ---------------------------------------------------------------------------

extern "C" {
    pub fn intel_ipu4_cpd_create_pkg_dir(
        adev: *mut intel_ipu4_bus_device,
        src: *const c_void,
        dma_addr_src: dma_addr_t,
        dma_addr: *mut dma_addr_t,
        pkg_dir_size: *mut c_uint,
    ) -> *mut c_void;

    pub fn intel_ipu4_cpd_free_pkg_dir(
        adev: *mut intel_ipu4_bus_device,
        pkg_dir: *mut u64,
        dma_addr: dma_addr_t,
        pkg_dir_size: c_uint,
    );

    pub fn intel_ipu4_cpd_get_pg_icache_base(
        isp: *mut intel_ipu4_device,
        idx: u8,
        cpd_file: *const c_void,
        cpd_file_size: c_uint,
    ) -> u32;

    pub fn intel_ipu4_cpd_get_pg_entry_point(
        isp: *mut intel_ipu4_device,
        idx: u8,
        cpd_file: *const c_void,
        cpd_file_size: c_uint,
    ) -> u32;

    pub fn intel_ipu4_cpd_validate_cpd_file(
        isp: *mut intel_ipu4_device,
        cpd_file: *const c_void,
        cpd_file_size: c_ulong,
    ) -> c_int;

    pub fn intel_ipu4_cpd_pkg_dir_get_address(
        pkg_dir: *const u64,
        pkg_dir_idx: c_int,
    ) -> c_uint;

    pub fn intel_ipu4_cpd_pkg_dir_get_num_entries(pkg_dir: *const u64) -> c_uint;

    pub fn intel_ipu4_cpd_pkg_dir_get_size(
        pkg_dir: *const u64,
        pkg_dir_idx: c_int,
    ) -> c_uint;

    pub fn intel_ipu4_cpd_pkg_dir_get_type(
        pkg_dir: *const u64,
        pkg_dir_idx: c_int,
    ) -> c_uint;
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn struct_sizes_match_c_layout() {
        assert_eq!(size_of::<intel_ipu4_cpd_hdr>(), 16);
        assert_eq!(size_of::<intel_ipu4_cpd_ent>(), 24);
        assert_eq!(size_of::<intel_ipu4_cpd_metadata_cmpnt>(), 68);
        assert_eq!(size_of::<intel_ipu_cpd_metadata_extn>(), 28);
        assert_eq!(size_of::<intel_ipu4_cpd_module_data_hdr>(), 44);
        assert_eq!(size_of::<intel_ipu_cpd_client_pkg_hdr>(), 32);
    }
}
