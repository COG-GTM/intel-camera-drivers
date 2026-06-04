//! Safe, idiomatic Rust port of the IPU4 ISYS firmware message marshaling
//! module (`drivers/media/pci/intel-ipu4/intel-ipu4-isys-fw-msgs.{c,h}`).
//!
//! The original C driver marshals firmware messages by casting between struct
//! pointers and byte buffers (`(struct foo *)buf`). That pattern is the source
//! of three classic memory-safety hazards which this crate eliminates:
//!
//! * **Misaligned access** — the C code reinterprets arbitrary buffers as
//!   structs; here every field is read/written through bounds-checked,
//!   endianness-explicit cursors ([`crate::bytes`]), so alignment is irrelevant.
//! * **Buffer overruns** — pointer casts assume the buffer is large enough;
//!   here every access is length-checked and a short buffer yields
//!   [`Error::BufferTooSmall`] instead of out-of-bounds memory access.
//! * **Use-after-free / aliasing** — the C code passes raw `void *` /
//!   `dma_addr_t` around; here ownership is expressed through owned values and
//!   borrowed slices, and the strong newtypes in [`crate::ids`] prevent mixing
//!   buffer ids, addresses, pin indices and stream handles.
//!
//! # Layout
//!
//! * [`Marshal`] — serialize to / deserialize from the firmware ABI byte
//!   layout (byte-exact with the C structs on an LP64 little-endian target).
//! * Message types & builders — [`StreamCfgData`], [`FrameBuffSet`],
//!   [`SendQueueToken`], [`ProxySendQueueToken`], [`RespInfo`],
//!   [`ProxyRespInfo`], plus the leaf descriptors they contain.
//! * [`ids`] — strong newtypes for pin / stream / buffer indices.
//! * [`enums`] — ABI enums with validating `from_u32`.
//!
//! With `--features c-abi`, the [`c_abi`] module re-exports the safe
//! implementation through the original C ABI via `#[no_mangle] extern "C"`.

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_debug_implementations)]

pub mod bytes;
pub mod enums;
pub mod error;
pub mod ids;
pub mod layout;
pub mod messages;
pub mod tables;

#[cfg(feature = "c-abi")]
pub mod c_abi;

pub use enums::{IslUse, IsysError, ProxyError, RespType, SendType};
pub use error::{Error, Result};
pub use ids::{BufferId, CssVirtualAddress, InputPinId, OutputPinId, StreamHandle};
pub use messages::{
    message_queue_index, proxy_queue_index, Cropping, ErrorInfo, FrameBuffSet, FrameBuffSetBuilder,
    InputPinInfo, IsaConfig, Marshal, OutputPinInfo, OutputPinPayload, ParamPin, ProxyErrorInfo,
    ProxyRespInfo, ProxySendQueueToken, Resolution, RespInfo, SendQueueToken, StreamCfgBuilder,
    StreamCfgData,
};
pub use tables::bits_per_pixel;
