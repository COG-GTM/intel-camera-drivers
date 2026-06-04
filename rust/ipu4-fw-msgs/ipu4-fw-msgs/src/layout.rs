//! ABI layout constants mirroring the C headers.
//!
//! All sizes below are the `sizeof` of the corresponding C struct on a standard
//! LP64 little-endian target (the kernel ABI), including trailing padding. The
//! safe serializer reproduces this layout byte-for-byte, which is verified by
//! the golden-file parity tests against reference C output.

// --- counts (intel-ipu4-isysapi-fw-types.h) ---
pub const INTEL_IPU4_MAX_IPINS: usize = 4;
pub const INTEL_IPU4_MAX_OPINS: usize = INTEL_IPU4_MAX_IPINS + 2;
pub const INTEL_IPU4_STREAM_ID_MAX: usize = 8;
pub const N_IPU_FW_ISYS_RESOLUTION_INFO: usize = 2;
pub const N_IPU_FW_ISYS_CROPPING_LOCATION: usize = 4;
pub const N_IPU_FW_ISYS_MIPI_DATA_TYPE: usize = 0x40;

// --- queue indexing (intel-ipu4-isys-fw-msgs.h) ---
pub const ISYS_FW_NBR_QUEUES: usize = 2;
pub const ISYS_NBR_PROXY_QUEUES: usize = 1;
pub const ISYS_PROXY_INDEX: usize = 0;
pub const ISYS_MSG_INDEX: usize = 1;

// --- serialized sizes (sizeof of each C struct, LP64) ---
pub const SIZEOF_RESOLUTION: usize = 8;
pub const SIZEOF_OUTPUT_PIN_PAYLOAD: usize = 16;
pub const SIZEOF_OUTPUT_PIN_INFO: usize = 24;
pub const SIZEOF_PARAM_PIN: usize = 16;
pub const SIZEOF_INPUT_PIN_INFO: usize = 12;
pub const SIZEOF_ISA_CFG: usize = 20;
pub const SIZEOF_CROPPING: usize = 16;
pub const SIZEOF_STREAM_CFG_DATA: usize = 292;
pub const SIZEOF_FRAME_BUFF_SET: usize = 120;
pub const SIZEOF_ERROR_INFO: usize = 8;
pub const SIZEOF_RESP_INFO: usize = 64;
pub const SIZEOF_PROXY_ERROR_INFO: usize = 8;
pub const SIZEOF_PROXY_RESP_INFO: usize = 12;
pub const SIZEOF_SEND_QUEUE_TOKEN: usize = 16;
pub const SIZEOF_PROXY_SEND_QUEUE_TOKEN: usize = 16;
