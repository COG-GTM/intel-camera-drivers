//! # ipu4-cpd
//!
//! An idiomatic, memory-safe Rust port of the Intel IPU4 **CPD** (Compact
//! Package Directory) firmware-package parser, originally implemented in C at
//! `drivers/media/pci/intel-ipu4/intel-ipu4-cpd.{c,h}`.
//!
//! The CPD format is a binary container describing the firmware shipped to the
//! IPU4 imaging unit: a `$CPD` header, an entry table pointing at a *manifest*,
//! *metadata* (a component table), and *moduledata* (an embedded directory of
//! firmware components). This crate parses and validates that structure.
//!
//! ## Design
//!
//! * **Slice-based, zero-copy.** Parsing operates on `&[u8]`; views borrow the
//!   input and decode fields on demand. No raw pointers in the public API.
//! * **No `unsafe` in the parser core.** Every read is bounds-checked and every
//!   offset/size computation is overflow-checked (see [`bytes`], [`types`]).
//! * **Typed errors.** Failures are reported as [`CpdError`] rather than a bare
//!   `-EINVAL`.
//! * **Strong newtypes** ([`Offset`], [`Size`]) keep offsets and lengths from
//!   being confused.
//!
//! ## Memory-safety improvements over the C original
//!
//! | Bug class in C | How the Rust port prevents it |
//! |---|---|
//! | Buffer overread (entries 0/1/2 read without checking `ent_cnt`) | [`CpdFile::entry`] range-checks the index → [`CpdError::MissingEntry`] |
//! | Overread on field access via packed-struct pointer casts | [`bytes`] readers return [`CpdError::OutOfBounds`] |
//! | Integer overflow in `offset + len` / `base + offset` | overflow-checked adds → [`CpdError::ArithmeticOverflow`] |
//! | Out-of-bounds pkg_dir write with hostile `ent_cnt` | [`build_pkg_dir`] caps components → [`CpdError::TooManyComponents`] |
//! | NULL / unchecked metadata component index | [`Metadata::component`] range-checks → [`CpdError::ComponentOutOfRange`] |
//!
//! ## C ABI
//!
//! With the `c-abi` cargo feature enabled, the parser is re-exported through
//! `#[no_mangle] extern "C"` symbols (see [`c_abi`]) so it can drop in behind
//! the original kernel call sites.

pub mod bytes;
pub mod consts;
pub mod error;
pub mod parser;
pub mod pkg_dir;
pub mod types;
pub mod view;

#[cfg(feature = "c-abi")]
pub mod c_abi;

pub use consts::CPD_HDR_MARK;
pub use error::{CpdError, Result};
pub use parser::{
    validate_cpd, validate_cpd_file_with_release, validate_metadata, validate_moduledata, CpdFile,
    Metadata,
};
pub use pkg_dir::{build_pkg_dir, PkgDir};
pub use types::{Offset, Size};
pub use view::{
    CpdEntry, CpdHeader, MetadataComponent, MetadataExtn, ModuleDataHeader,
};
