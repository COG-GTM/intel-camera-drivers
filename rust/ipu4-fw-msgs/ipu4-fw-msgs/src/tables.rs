//! Static firmware lookup tables ported verbatim from the C driver.

use crate::layout::N_IPU_FW_ISYS_MIPI_DATA_TYPE;

/// Value used for MIPI data types that carry no defined bit width.
pub const IPU_FW_UNSUPPORTED_DATA_TYPE: u32 = 0;

/// `extracted_bits_per_pixel_per_mipi_data_type` from
/// `intel-ipu4-isys-fw-tables.h`: native bits per pixel keyed by MIPI data type.
pub const EXTRACTED_BITS_PER_PIXEL_PER_MIPI_DATA_TYPE: [u32; N_IPU_FW_ISYS_MIPI_DATA_TYPE] = {
    let mut t = [IPU_FW_UNSUPPORTED_DATA_TYPE; N_IPU_FW_ISYS_MIPI_DATA_TYPE];
    t[0x00] = 64; // FRAME_START_CODE
    t[0x01] = 64; // FRAME_END_CODE
    t[0x02] = 64; // LINE_START_CODE
    t[0x03] = 64; // LINE_END_CODE
    t[0x08] = 64; // GENERIC_SHORT1
    t[0x09] = 64; // GENERIC_SHORT2
    t[0x0A] = 64; // GENERIC_SHORT3
    t[0x0B] = 64; // GENERIC_SHORT4
    t[0x0C] = 64; // GENERIC_SHORT5
    t[0x0D] = 64; // GENERIC_SHORT6
    t[0x0E] = 64; // GENERIC_SHORT7
    t[0x0F] = 64; // GENERIC_SHORT8
    t[0x12] = 8; // EMBEDDED
    t[0x18] = 12; // YUV420_8
    t[0x19] = 15; // YUV420_10
    t[0x1A] = 12; // YUV420_8_LEGACY
    t[0x1C] = 12; // YUV420_8_SHIFT
    t[0x1D] = 15; // YUV420_10_SHIFT
    t[0x1E] = 16; // YUV422_8
    t[0x1F] = 20; // YUV422_10
    t[0x20] = 16; // RGB_444
    t[0x21] = 16; // RGB_555
    t[0x22] = 16; // RGB_565
    t[0x23] = 18; // RGB_666
    t[0x24] = 24; // RGB_888
    t[0x28] = 6; // RAW_6
    t[0x29] = 7; // RAW_7
    t[0x2A] = 8; // RAW_8
    t[0x2B] = 10; // RAW_10
    t[0x2C] = 12; // RAW_12
    t[0x2D] = 14; // RAW_14
    t[0x2E] = 16; // RAW_16
    t[0x2F] = 8; // BINARY_8
    t[0x30] = 8; // USER_DEF1
    t[0x31] = 8; // USER_DEF2
    t[0x32] = 8; // USER_DEF3
    t[0x33] = 8; // USER_DEF4
    t[0x34] = 8; // USER_DEF5
    t[0x35] = 8; // USER_DEF6
    t[0x36] = 8; // USER_DEF7
    t[0x37] = 8; // USER_DEF8
    t
};

/// Look up the native bits-per-pixel for a MIPI data type, mirroring the C
/// table indexing in `intel_ipu4_isys_set_fw_params`. Out-of-range data types
/// (>= `N_IPU_FW_ISYS_MIPI_DATA_TYPE`) return `0`.
#[inline]
pub fn bits_per_pixel(dt: u8) -> u32 {
    let idx = dt as usize;
    if idx < N_IPU_FW_ISYS_MIPI_DATA_TYPE {
        EXTRACTED_BITS_PER_PIXEL_PER_MIPI_DATA_TYPE[idx]
    } else {
        IPU_FW_UNSUPPORTED_DATA_TYPE
    }
}
