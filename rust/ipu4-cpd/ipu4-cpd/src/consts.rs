//! Constants ported verbatim from `intel-ipu4-cpd.c` / `intel-ipu4-cpd.h`.

/// `$CPD` — CPD file header marker (`CPD_HDR_MARK`).
pub const CPD_HDR_MARK: u32 = 0x4450_4324;

/// `_IUPKDR_` — package-directory header marker (`PKG_DIR_HDR_MARK`).
pub const PKG_DIR_HDR_MARK: u64 = 0x5f49_5550_4b44_525f;

/// 15 entries + header (`MAX_PKG_DIR_ENT_CNT`).
pub const MAX_PKG_DIR_ENT_CNT: usize = 16;

/// 2 qwords per pkg_dir entry/header (`PKG_DIR_ENT_LEN`).
pub const PKG_DIR_ENT_LEN: usize = 2;

/// pkg_dir size in bytes (`PKG_DIR_SIZE`).
pub const PKG_DIR_SIZE: usize =
    MAX_PKG_DIR_ENT_CNT * PKG_DIR_ENT_LEN * core::mem::size_of::<u64>();

pub const PKG_DIR_ID_SHIFT: u32 = 48;
pub const PKG_DIR_ID_MASK: u64 = 0x7f;
pub const PKG_DIR_VERSION_SHIFT: u32 = 32;
pub const PKG_DIR_SIZE_MASK: u64 = 0xf_ffff;

/// Maximum manifest size: 2K DWORDs (`MAX_MANIFEST_SIZE`).
pub const MAX_MANIFEST_SIZE: u32 = 2 * 1024 * core::mem::size_of::<u32>() as u32;

/// Maximum metadata size: 64K (`MAX_METADATA_SIZE`).
pub const MAX_METADATA_SIZE: u32 = 64 * 1024;

pub const MAX_COMPONENT_ID: u32 = 127;
pub const MAX_COMPONENT_VERSION: u32 = 0xffff;

/// Fixed CPD entry indices (`CPD_*_IDX`).
pub const CPD_MANIFEST_IDX: usize = 0;
pub const CPD_METADATA_IDX: usize = 1;
pub const CPD_MODULEDATA_IDX: usize = 2;

pub const METADATA_EXTN_TYPE_IUNIT: u32 = 0x10;
pub const METADATA_IMAGE_TYPE_MAIN_FIRMWARE: u32 = 2;

// On-wire structure sizes (bytes). These mirror `sizeof()` of the matching
// `__packed` C structs and are verified against `ipu4-cpd-sys` in tests.
pub const SIZEOF_CPD_HDR: usize = 16;
pub const SIZEOF_CPD_ENT: usize = 24;
pub const SIZEOF_METADATA_EXTN: usize = 28;
pub const SIZEOF_METADATA_CMPNT: usize = 68;
pub const SIZEOF_MODULE_DATA_HDR: usize = 44;
