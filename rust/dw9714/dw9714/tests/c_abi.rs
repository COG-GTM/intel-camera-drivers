//! Tests for the C-ABI shim (`c-abi` feature). Run with:
//! `cargo test --features c-abi`.
//!
//! These drive the `#[no_mangle] extern "C"` entry points exactly as the
//! kernel module would: a callback table plus an opaque user context.

#![cfg(feature = "c-abi")]

use core::ffi::{c_int, c_void};

use dw9714::ffi::{
    dw9714_c_ops, dw9714_handle, dw9714_rust_create, dw9714_rust_current_val, dw9714_rust_destroy,
    dw9714_rust_runtime_resume, dw9714_rust_runtime_suspend, dw9714_rust_set_ctrl,
    dw9714_rust_set_position, dw9714_rust_vcm_val,
};

const ADDR: u16 = 0x0c;
const EIO: c_int = -5;
const EINVAL: c_int = -22;
const FOCUS_ABSOLUTE: u32 = 0x009a_090a;

#[derive(Default)]
struct Ctx {
    writes: Vec<(u16, Vec<u8>)>,
    delays: Vec<u32>,
    gpio: Vec<bool>,
    power_get: usize,
    power_put: usize,
    fail_writes: bool,
}

unsafe extern "C" fn cb_i2c_write(
    user: *mut c_void,
    addr: u16,
    buf: *const u8,
    len: usize,
) -> c_int {
    let ctx = &mut *(user as *mut Ctx);
    let slice = std::slice::from_raw_parts(buf, len);
    ctx.writes.push((addr, slice.to_vec()));
    if ctx.fail_writes {
        EIO
    } else {
        0
    }
}

unsafe extern "C" fn cb_delay(user: *mut c_void, us: u32) {
    (*(user as *mut Ctx)).delays.push(us);
}

unsafe extern "C" fn cb_gpio(user: *mut c_void, on: c_int) -> c_int {
    (*(user as *mut Ctx)).gpio.push(on != 0);
    0
}

unsafe extern "C" fn cb_power_get(user: *mut c_void) -> c_int {
    (*(user as *mut Ctx)).power_get += 1;
    0
}

unsafe extern "C" fn cb_power_put(user: *mut c_void) {
    (*(user as *mut Ctx)).power_put += 1;
}

fn full_ops() -> dw9714_c_ops {
    dw9714_c_ops {
        i2c_write: Some(cb_i2c_write),
        delay_us: Some(cb_delay),
        set_gpio_xsd: Some(cb_gpio),
        sensor_power_get: Some(cb_power_get),
        sensor_power_put: Some(cb_power_put),
    }
}

fn create(ctx: &mut Ctx, gpio_xsd: c_int, has_sensor: c_int) -> *mut dw9714_handle {
    let ops = full_ops();
    unsafe {
        dw9714_rust_create(
            ADDR,
            gpio_xsd,
            has_sensor,
            &ops as *const dw9714_c_ops,
            ctx as *mut Ctx as *mut c_void,
        )
    }
}

#[test]
fn vcm_val_pure_export() {
    assert_eq!(dw9714_rust_vcm_val(512, 0), 512 << 4);
    assert_eq!(dw9714_rust_vcm_val(100, 0x5), (100 << 4) | 0x5);
    // Out-of-range data saturates rather than corrupting the word.
    assert_eq!(dw9714_rust_vcm_val(5000, 0), 1023 << 4);
}

#[test]
fn create_set_position_destroy() {
    let mut ctx = Ctx::default();
    let h = create(&mut ctx, -1, 0);
    assert!(!h.is_null());

    let rc = unsafe { dw9714_rust_set_position(h, 512) };
    assert_eq!(rc, 0);
    assert_eq!(unsafe { dw9714_rust_current_val(h) }, 512);

    unsafe { dw9714_rust_destroy(h) };

    // Big-endian VCM_VAL(512, 0).
    assert_eq!(ctx.writes.len(), 1);
    assert_eq!(ctx.writes[0].0, ADDR);
    assert_eq!(ctx.writes[0].1, (512u16 << 4).to_be_bytes().to_vec());
}

#[test]
fn set_position_out_of_range_returns_einval() {
    let mut ctx = Ctx::default();
    let h = create(&mut ctx, -1, 0);
    let rc = unsafe { dw9714_rust_set_position(h, 5000) };
    assert_eq!(rc, EINVAL);
    unsafe { dw9714_rust_destroy(h) };
    assert!(ctx.writes.is_empty());
}

#[test]
fn i2c_failure_returns_eio() {
    let mut ctx = Ctx {
        fail_writes: true,
        ..Ctx::default()
    };
    let h = create(&mut ctx, -1, 0);
    let rc = unsafe { dw9714_rust_set_position(h, 100) };
    assert_eq!(rc, EIO);
    unsafe { dw9714_rust_destroy(h) };
    // Original + one retry.
    assert_eq!(ctx.writes.len(), 2);
}

#[test]
fn set_ctrl_rejects_unknown_id() {
    let mut ctx = Ctx::default();
    let h = create(&mut ctx, -1, 0);
    assert_eq!(unsafe { dw9714_rust_set_ctrl(h, 0xdead, 1) }, EINVAL);
    assert_eq!(unsafe { dw9714_rust_set_ctrl(h, FOCUS_ABSOLUTE, 200) }, 0);
    unsafe { dw9714_rust_destroy(h) };
    assert_eq!(ctx.writes.len(), 1);
}

#[test]
fn null_handle_is_handled_gracefully() {
    assert_eq!(
        unsafe { dw9714_rust_set_position(core::ptr::null_mut(), 1) },
        EINVAL
    );
    assert_eq!(
        unsafe { dw9714_rust_current_val(core::ptr::null()) },
        EINVAL
    );
    // Destroying NULL is a no-op (must not crash).
    unsafe { dw9714_rust_destroy(core::ptr::null_mut()) };
}

#[test]
fn create_rejects_null_ops_and_missing_i2c_write() {
    let mut ctx = Ctx::default();
    // NULL ops pointer.
    let h = unsafe {
        dw9714_rust_create(
            ADDR,
            -1,
            0,
            core::ptr::null(),
            &mut ctx as *mut Ctx as *mut c_void,
        )
    };
    assert!(h.is_null());

    // ops with no i2c_write.
    let ops = dw9714_c_ops {
        i2c_write: None,
        delay_us: None,
        set_gpio_xsd: None,
        sensor_power_get: None,
        sensor_power_put: None,
    };
    let h = unsafe {
        dw9714_rust_create(
            ADDR,
            -1,
            0,
            &ops as *const dw9714_c_ops,
            &mut ctx as *mut Ctx as *mut c_void,
        )
    };
    assert!(h.is_null());
}

#[test]
fn runtime_suspend_resume_through_ffi() {
    let mut ctx = Ctx::default();
    let h = create(&mut ctx, 5, 1);
    assert_eq!(unsafe { dw9714_rust_set_position(h, 256) }, 0);
    assert_eq!(unsafe { dw9714_rust_runtime_resume(h) }, 0);
    assert_eq!(unsafe { dw9714_rust_runtime_suspend(h) }, 0);
    unsafe { dw9714_rust_destroy(h) };

    // Resume powered up (gpio high + power get), suspend powered down.
    assert!(ctx.power_get >= 1);
    assert!(ctx.power_put >= 1);
    assert!(ctx.gpio.contains(&true));
    assert!(!*ctx.gpio.last().unwrap());
    assert!(!ctx.delays.is_empty());
}
