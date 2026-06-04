//! C ABI re-exports (`c-abi` feature).
//!
//! This module exposes the ported Rust implementation back through a stable C
//! ABI so the kernel module can be migrated incrementally: existing C callers
//! provide a small table of callbacks (I2C / GPIO / power / delay) and drive
//! the Rust logic through `#[no_mangle] extern "C"` entry points, with **no**
//! change required to the rest of the C driver.
//!
//! # Safety boundary
//!
//! Every `unsafe` block in the entire `dw9714` crate lives here. The shim's
//! job is exactly to translate raw C pointers/handles into safe Rust values
//! and back; once inside, all work is delegated to the safe [`Dw9714`] type.
//! Each `unsafe` block documents the invariants the C caller must uphold.

// This is the one module permitted to use `unsafe` (the crate root `deny`s it
// elsewhere). Every block below carries a `SAFETY:` justification.
#![allow(unsafe_code)]
// C-ABI exports intentionally use C naming conventions (`dw9714_c_ops`,
// `dw9714_handle`) so the symbols match what the kernel module expects.
#![allow(non_camel_case_types)]

use core::ffi::{c_int, c_void};

use crate::bus::{BusError, Dw9714Bus};
use crate::device::{DeviceConfig, Dw9714};
use crate::types::{LensPosition, VcmStep};

/// I2C write callback: write `len` bytes from `buf` to slave `addr`.
/// Returns `0` on success, negative `errno` on failure. `user` is the opaque
/// context passed to [`dw9714_rust_create`].
pub type I2cWriteFn =
    unsafe extern "C" fn(user: *mut c_void, addr: u16, buf: *const u8, len: usize) -> c_int;

/// Microsecond delay callback.
pub type DelayUsFn = unsafe extern "C" fn(user: *mut c_void, us: u32);

/// xshutdown GPIO callback. `on != 0` drives the GPIO high. Returns `0` on
/// success, negative `errno` on failure.
pub type SetGpioXsdFn = unsafe extern "C" fn(user: *mut c_void, on: c_int) -> c_int;

/// Sensor power-up callback (`pm_runtime_get_sync`). Returns `0`/negative.
pub type SensorPowerGetFn = unsafe extern "C" fn(user: *mut c_void) -> c_int;

/// Sensor power-down callback (`pm_runtime_put`).
pub type SensorPowerPutFn = unsafe extern "C" fn(user: *mut c_void);

/// Table of host callbacks supplied by the C caller.
///
/// Only [`i2c_write`](Self::i2c_write) is mandatory; the rest may be `NULL`
/// when the platform lacks that capability (e.g. no xshutdown GPIO).
#[repr(C)]
pub struct dw9714_c_ops {
    /// Mandatory I2C write hook.
    pub i2c_write: Option<I2cWriteFn>,
    /// Optional delay hook.
    pub delay_us: Option<DelayUsFn>,
    /// Optional xshutdown GPIO hook.
    pub set_gpio_xsd: Option<SetGpioXsdFn>,
    /// Optional sensor power-up hook.
    pub sensor_power_get: Option<SensorPowerGetFn>,
    /// Optional sensor power-down hook.
    pub sensor_power_put: Option<SensorPowerPutFn>,
}

/// A [`Dw9714Bus`] backed by C callbacks.
///
/// Holds a raw `user` context pointer and is therefore automatically `!Send`
/// and `!Sync` — exactly the correct bound, since the C side owns the
/// threading contract for that pointer.
///
/// Public only because it appears in the [`dw9714_handle`] type alias; its
/// fields remain private so C code can only obtain one via
/// [`dw9714_rust_create`].
pub struct CallbackBus {
    ops: dw9714_c_ops,
    user: *mut c_void,
}

impl Dw9714Bus for CallbackBus {
    fn i2c_write(&mut self, addr: u16, bytes: &[u8]) -> Result<(), BusError> {
        let f = self.ops.i2c_write.ok_or(BusError)?;
        // SAFETY: `f` is a function pointer the C caller guaranteed valid for
        // the lifetime of this handle (via `dw9714_rust_create`). `bytes` is a
        // live Rust slice, so `as_ptr()`/`len()` describe a valid, readable
        // region of exactly `len` bytes. `self.user` is passed back verbatim
        // as the caller's opaque context.
        let ret = unsafe { f(self.user, addr, bytes.as_ptr(), bytes.len()) };
        if ret == 0 {
            Ok(())
        } else {
            Err(BusError)
        }
    }

    fn delay_us(&mut self, us: u32) {
        if let Some(f) = self.ops.delay_us {
            // SAFETY: `f` is a valid C callback per the create-time contract;
            // it takes only the opaque context and a scalar.
            unsafe { f(self.user, us) };
        }
    }

    fn set_gpio_xsd(&mut self, on: bool) -> Result<(), BusError> {
        match self.ops.set_gpio_xsd {
            // SAFETY: valid C callback per the create-time contract; only the
            // opaque context and a scalar are passed.
            Some(f) => {
                let ret = unsafe { f(self.user, c_int::from(on)) };
                if ret == 0 {
                    Ok(())
                } else {
                    Err(BusError)
                }
            }
            None => Ok(()),
        }
    }

    fn sensor_power_get(&mut self) -> Result<(), BusError> {
        match self.ops.sensor_power_get {
            // SAFETY: valid C callback per the create-time contract.
            Some(f) => {
                let ret = unsafe { f(self.user) };
                if ret == 0 {
                    Ok(())
                } else {
                    Err(BusError)
                }
            }
            None => Ok(()),
        }
    }

    fn sensor_power_put(&mut self) {
        if let Some(f) = self.ops.sensor_power_put {
            // SAFETY: valid C callback per the create-time contract.
            unsafe { f(self.user) };
        }
    }
}

/// Opaque handle type handed to C as `struct dw9714_handle *`.
pub type dw9714_handle = Dw9714<CallbackBus>;

/// `uint16_t dw9714_rust_vcm_val(uint16_t data, uint16_t s)` — pure register
/// packing, exported so C call sites can drop their `VCM_VAL` macro.
#[no_mangle]
pub extern "C" fn dw9714_rust_vcm_val(data: u16, s: u16) -> u16 {
    LensPosition::new_saturating(data)
        .register_value(VcmStep::new(s))
        .get()
}

/// Create a dw9714 handle driven by the supplied callbacks.
///
/// * `gpio_xsd` &lt; 0 means "no xshutdown GPIO" (matches the C convention).
/// * `has_sensor_dev != 0` enables the sensor power gating path.
///
/// Returns an owning handle pointer, or `NULL` on error. The handle must be
/// released with [`dw9714_rust_destroy`].
///
/// # Safety
/// `ops` must point to a valid `dw9714_c_ops` for the duration of this call
/// (it is copied). Every non-null callback in `ops`, and the `user` pointer,
/// must remain valid for the entire lifetime of the returned handle.
#[no_mangle]
pub unsafe extern "C" fn dw9714_rust_create(
    i2c_addr: u16,
    gpio_xsd: c_int,
    has_sensor_dev: c_int,
    ops: *const dw9714_c_ops,
    user: *mut c_void,
) -> *mut dw9714_handle {
    if ops.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: caller guarantees `ops` points to a valid, initialized
    // `dw9714_c_ops`. We copy each field out by value; no reference outlives
    // this read.
    let ops = unsafe { &*ops };
    let ops_copy = dw9714_c_ops {
        i2c_write: ops.i2c_write,
        delay_us: ops.delay_us,
        set_gpio_xsd: ops.set_gpio_xsd,
        sensor_power_get: ops.sensor_power_get,
        sensor_power_put: ops.sensor_power_put,
    };
    if ops_copy.i2c_write.is_none() {
        return core::ptr::null_mut();
    }

    let config = DeviceConfig {
        i2c_addr,
        gpio_xsd: if gpio_xsd < 0 {
            None
        } else {
            Some(gpio_xsd as u32)
        },
        has_sensor_dev: has_sensor_dev != 0,
    };
    let bus = CallbackBus {
        ops: ops_copy,
        user,
    };
    match Dw9714::probe(bus, config) {
        Ok(dev) => Box::into_raw(Box::new(dev)),
        Err(_) => core::ptr::null_mut(),
    }
}

/// Destroy a handle previously returned by [`dw9714_rust_create`].
///
/// # Safety
/// `handle` must have been returned by [`dw9714_rust_create`] and not already
/// destroyed. Passing `NULL` is allowed and is a no-op.
#[no_mangle]
pub unsafe extern "C" fn dw9714_rust_destroy(handle: *mut dw9714_handle) {
    if handle.is_null() {
        return;
    }
    // SAFETY: per the contract `handle` is a live pointer from
    // `dw9714_rust_create`; reconstituting the Box transfers ownership back so
    // it is dropped exactly once.
    drop(unsafe { Box::from_raw(handle) });
}

/// Helper: turn a raw handle into a safe mutable reference for the duration of
/// a call.
///
/// # Safety
/// `handle` must be a live pointer from [`dw9714_rust_create`].
unsafe fn handle_mut<'a>(handle: *mut dw9714_handle) -> Option<&'a mut dw9714_handle> {
    // SAFETY: caller guarantees `handle` is live and uniquely accessed for the
    // duration of the returned borrow (single-threaded C call convention).
    unsafe { handle.as_mut() }
}

/// `int dw9714_rust_set_position(handle, uint16_t pos)` — port of
/// `dw9714_t_focus_vcm`. Returns `0` or a negated `errno`.
///
/// # Safety
/// `handle` must be a live handle from [`dw9714_rust_create`].
#[no_mangle]
pub unsafe extern "C" fn dw9714_rust_set_position(handle: *mut dw9714_handle, pos: u16) -> c_int {
    // SAFETY: caller contract documented above.
    let Some(dev) = (unsafe { handle_mut(handle) }) else {
        return crate::error::DW9714Error::InvalidArgument.to_errno();
    };
    match dev.set_position_raw(pos) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// `int dw9714_rust_set_ctrl(handle, uint32_t id, int32_t val)` — port of
/// `dw9714_set_ctrl`.
///
/// # Safety
/// `handle` must be a live handle from [`dw9714_rust_create`].
#[no_mangle]
pub unsafe extern "C" fn dw9714_rust_set_ctrl(
    handle: *mut dw9714_handle,
    id: u32,
    val: i32,
) -> c_int {
    // SAFETY: caller contract documented above.
    let Some(dev) = (unsafe { handle_mut(handle) }) else {
        return crate::error::DW9714Error::InvalidArgument.to_errno();
    };
    match dev.set_ctrl(id, val) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// `int dw9714_rust_runtime_suspend(handle)` — port of
/// `dw9714_runtime_suspend`.
///
/// # Safety
/// `handle` must be a live handle from [`dw9714_rust_create`].
#[no_mangle]
pub unsafe extern "C" fn dw9714_rust_runtime_suspend(handle: *mut dw9714_handle) -> c_int {
    // SAFETY: caller contract documented above.
    let Some(dev) = (unsafe { handle_mut(handle) }) else {
        return crate::error::DW9714Error::InvalidArgument.to_errno();
    };
    match dev.runtime_suspend() {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// `int dw9714_rust_runtime_resume(handle)` — port of
/// `dw9714_runtime_resume`.
///
/// # Safety
/// `handle` must be a live handle from [`dw9714_rust_create`].
#[no_mangle]
pub unsafe extern "C" fn dw9714_rust_runtime_resume(handle: *mut dw9714_handle) -> c_int {
    // SAFETY: caller contract documented above.
    let Some(dev) = (unsafe { handle_mut(handle) }) else {
        return crate::error::DW9714Error::InvalidArgument.to_errno();
    };
    match dev.runtime_resume() {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// `int dw9714_rust_current_val(handle)` — read back `current_val`, or a
/// negated `errno` if the handle is NULL.
///
/// # Safety
/// `handle` must be a live handle from [`dw9714_rust_create`].
#[no_mangle]
pub unsafe extern "C" fn dw9714_rust_current_val(handle: *const dw9714_handle) -> c_int {
    if handle.is_null() {
        return crate::error::DW9714Error::InvalidArgument.to_errno();
    }
    // SAFETY: caller guarantees `handle` is live; we take a shared borrow for
    // the duration of this read only.
    let dev = unsafe { &*handle };
    c_int::from(dev.current_val())
}
