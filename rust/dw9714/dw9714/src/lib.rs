//! # dw9714 — safe Rust driver for the DW9714 VCM lens actuator
//!
//! A safe, idiomatic Rust port of the Intel camera `dw9714` voice-coil-motor
//! (VCM) lens-actuator driver (`drivers/media/i2c/dw9714.c`). The port keeps
//! 100% of the business logic in safe Rust and confines all `unsafe` to the
//! optional C-ABI shim (the [`ffi`] module, gated behind the `c-abi` feature).
//!
//! ## Layout
//!
//! * [`error`] — [`DW9714Error`] replacing negated-`errno` return codes.
//! * [`types`] — strong newtypes ([`LensPosition`], [`RegisterValue`],
//!   [`VcmStep`]) making out-of-range positions and position/register mix-ups
//!   unrepresentable.
//! * [`bus`] — the [`Dw9714Bus`] hardware-abstraction trait.
//! * [`mock`] — an in-memory [`MockI2cBus`] so `cargo test` needs no hardware.
//! * [`device`] — the ported logic: [`Dw9714`] (RAII probe/remove) and
//!   [`PowerGuard`] (RAII open/close).
//! * [`ffi`] — `#[no_mangle] extern "C"` re-exports (feature `c-abi`).
//!
//! ## Example
//!
//! ```
//! use dw9714::{Dw9714, DeviceConfig, LensPosition};
//! use dw9714::mock::MockI2cBus;
//!
//! let mut dev = Dw9714::probe(MockI2cBus::new(), DeviceConfig::new(0x0c)).unwrap();
//! {
//!     // RAII power: resumes on open, suspends on drop.
//!     let mut open = dev.open().unwrap();
//!     open.set_position(LensPosition::new(512).unwrap()).unwrap();
//!     assert_eq!(open.position().get(), 512);
//! }
//! ```

// Without the C-ABI shim the entire crate is unsafe-free and we enforce that
// hard. With `c-abi`, unsafe is denied everywhere except the `ffi` module,
// which opts back in locally — keeping the safety boundary explicit.
#![cfg_attr(not(feature = "c-abi"), forbid(unsafe_code))]
#![cfg_attr(feature = "c-abi", deny(unsafe_code))]
#![warn(missing_docs)]

pub mod bus;
pub mod device;
pub mod error;
pub mod mock;
pub mod types;

#[cfg(feature = "c-abi")]
pub mod ffi;

pub use bus::{BusError, Dw9714Bus};
pub use device::{DeviceConfig, Dw9714, PowerGuard};
pub use error::{DW9714Error, Result, MAX_FOCUS_POS};
pub use types::{LensPosition, RegisterValue, VcmStep};
