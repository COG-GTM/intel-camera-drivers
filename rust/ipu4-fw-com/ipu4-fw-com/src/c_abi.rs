//! Original C ABI re-exported on top of the safe implementation.
//!
//! Enabled by the `c-abi` cargo feature. Every exported symbol keeps the name
//! and signature of the original driver function (see `intel-ipu4-fw-com.h`) so
//! the safe Rust core can be dropped in as a binary-compatible replacement.
//!
//! This is the *only* module that contains `unsafe` code: it is the trust
//! boundary between untyped C pointers and the safe core. Each shim validates
//! its pointers, translates the call into a safe-core method, and maps the
//! `Result` back onto the driver's original return convention (`NULL` / negative
//! `errno`).
//!
//! Note: the token payload pointers returned here address per-queue *staging*
//! buffers owned by the context (not raw DMA memory, which does not exist
//! off-hardware). The get/fill/put protocol is preserved: `*_get_token` returns
//! a buffer to fill (or read), `*_put_token` commits it to / releases it from
//! the ring.

use core::ffi::{c_int, c_void};

use ipu4_fw_com_sys::{
    ia_css_syscom_queue_config, intel_ipu4_bus_device, intel_ipu4_fw_com_cfg, cell_ready_fn,
    cell_start_fn,
};

use crate::context::{Cell, FwComConfig, FwComContext, QueueConfig};

/// A [`Cell`] backed by raw C function pointers from `intel_ipu4_fw_com_cfg`.
struct FnPtrCell {
    adev: *mut intel_ipu4_bus_device,
    ready: cell_ready_fn,
    start: cell_start_fn,
}

// The C driver only ever touches `adev` from the controlled syscom call sites;
// we encapsulate that contract here at the FFI boundary.
unsafe impl Send for FnPtrCell {}
unsafe impl Sync for FnPtrCell {}

impl Cell for FnPtrCell {
    fn ready(&self) -> bool {
        match self.ready {
            // SAFETY: `ready` is a valid C callback supplied in the config and
            // `adev` is the opaque device handle the caller associated with it.
            Some(f) => unsafe { f(self.adev) != 0 },
            None => false,
        }
    }
    fn start(&self) {
        if let Some(f) = self.start {
            // SAFETY: see `ready` above.
            unsafe { f(self.adev) };
        }
    }
}

/// Context plus per-queue staging buffers backing the raw token pointers.
struct CAbiContext {
    inner: FwComContext,
    send_staging: Vec<Box<[u8]>>,
    recv_staging: Vec<Box<[u8]>>,
}

/// Safe analogue of `intel_ipu4_fw_com_prepare`.
///
/// # Safety
/// `cfg` must be a valid pointer to an `intel_ipu4_fw_com_cfg` whose `input` /
/// `output` arrays hold the advertised number of elements, or null.
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_fw_com_prepare(
    cfg: *mut intel_ipu4_fw_com_cfg,
    adev: *mut intel_ipu4_bus_device,
    _base: *mut c_void,
) -> *mut c_void {
    if cfg.is_null() {
        return core::ptr::null_mut();
    }
    let cfg = &*cfg;
    // Mirror the C guard: `!cfg->cell_start || !cfg->cell_ready`.
    if cfg.cell_ready.is_none() || cfg.cell_start.is_none() {
        return core::ptr::null_mut();
    }

    let input = read_queue_configs(cfg.input, cfg.num_input_queues);
    let output = read_queue_configs(cfg.output, cfg.num_output_queues);

    let specific = if cfg.specific_addr.is_null() || cfg.specific_size == 0 {
        Vec::new()
    } else {
        core::slice::from_raw_parts(cfg.specific_addr as *const u8, cfg.specific_size as usize)
            .to_vec()
    };

    let fw_cfg = FwComConfig {
        input,
        output,
        dmem_addr: cfg.dmem_addr,
        specific,
    };

    let cell = Box::new(FnPtrCell {
        adev,
        ready: cfg.cell_ready,
        start: cfg.cell_start,
    });

    let inner = match FwComContext::prepare(&fw_cfg, cell) {
        Ok(ctx) => ctx,
        Err(_) => return core::ptr::null_mut(),
    };

    let send_staging = (0..inner.num_input_queues())
        .map(|q| vec![0u8; inner.input_token_size(q).unwrap_or(0) as usize].into_boxed_slice())
        .collect();
    let recv_staging = (0..inner.num_output_queues())
        .map(|q| vec![0u8; inner.output_token_size(q).unwrap_or(0) as usize].into_boxed_slice())
        .collect();

    let boxed = Box::new(CAbiContext {
        inner,
        send_staging,
        recv_staging,
    });
    Box::into_raw(boxed) as *mut c_void
}

unsafe fn read_queue_configs(
    ptr: *mut ia_css_syscom_queue_config,
    count: u32,
) -> Vec<QueueConfig> {
    if ptr.is_null() || count == 0 {
        return Vec::new();
    }
    core::slice::from_raw_parts(ptr, count as usize)
        .iter()
        .map(|q| QueueConfig {
            queue_size: q.queue_size,
            token_size: q.token_size,
        })
        .collect()
}

/// # Safety
/// `ptr` must be a context returned by [`intel_ipu4_fw_com_prepare`] and not yet
/// released.
unsafe fn ctx_ref<'a>(ptr: *mut c_void) -> Option<&'a mut CAbiContext> {
    (ptr as *mut CAbiContext).as_mut()
}

/// Safe analogue of `intel_ipu4_fw_com_open`.
///
/// # Safety
/// See [`ctx_ref`].
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_fw_com_open(ctx: *mut c_void) -> c_int {
    let Some(cabi) = ctx_ref(ctx) else {
        return -22; // -EINVAL
    };
    match cabi.inner.open() {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// Safe analogue of `intel_ipu4_fw_com_ready`.
///
/// # Safety
/// See [`ctx_ref`].
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_fw_com_ready(ctx: *mut c_void) -> c_int {
    let Some(cabi) = ctx_ref(ctx) else {
        return -22;
    };
    match cabi.inner.ready() {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// Safe analogue of `intel_ipu4_fw_com_close`.
///
/// # Safety
/// See [`ctx_ref`].
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_fw_com_close(ctx: *mut c_void) -> c_int {
    let Some(cabi) = ctx_ref(ctx) else {
        return -22;
    };
    match cabi.inner.close() {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// Safe analogue of `intel_ipu4_fw_com_release`. Consumes the context.
///
/// # Safety
/// `ctx` must be a context from [`intel_ipu4_fw_com_prepare`]; it must not be
/// used again after this call.
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_fw_com_release(ctx: *mut c_void, force: u32) -> c_int {
    if ctx.is_null() {
        return -22;
    }
    let cabi = Box::from_raw(ctx as *mut CAbiContext);
    match cabi.inner.release(force != 0) {
        Ok(()) => 0,
        Err(e) => {
            // Release failed (cell busy): the box is dropped here, matching the
            // safe API. The C caller should treat a non-zero return as "not
            // freed by me", but our RAII guarantees no leak regardless.
            e.to_errno()
        }
    }
}

/// Safe analogue of `intel_ipu4_send_get_token`.
///
/// # Safety
/// See [`ctx_ref`].
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_send_get_token(ctx: *mut c_void, q_nbr: c_int) -> *mut c_void {
    let Some(cabi) = ctx_ref(ctx) else {
        return core::ptr::null_mut();
    };
    if q_nbr < 0 {
        return core::ptr::null_mut();
    }
    let q = q_nbr as usize;
    // A successful (dropped, uncommitted) acquire proves there is a free slot.
    match cabi.inner.send_get_token(q) {
        Ok(tok) => drop(tok),
        Err(_) => return core::ptr::null_mut(),
    }
    match cabi.send_staging.get_mut(q) {
        Some(buf) => buf.as_mut_ptr() as *mut c_void,
        None => core::ptr::null_mut(),
    }
}

/// Safe analogue of `intel_ipu4_send_put_token`.
///
/// # Safety
/// See [`ctx_ref`].
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_send_put_token(ctx: *mut c_void, q_nbr: c_int) {
    let Some(cabi) = ctx_ref(ctx) else {
        return;
    };
    if q_nbr < 0 {
        return;
    }
    let q = q_nbr as usize;
    if let Some(buf) = cabi.send_staging.get(q) {
        let token = buf.clone();
        let _ = cabi.inner.send(q, &token);
    }
}

/// Safe analogue of `intel_ipu4_recv_get_token`.
///
/// # Safety
/// See [`ctx_ref`].
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_recv_get_token(ctx: *mut c_void, q_nbr: c_int) -> *mut c_void {
    let Some(cabi) = ctx_ref(ctx) else {
        return core::ptr::null_mut();
    };
    if q_nbr < 0 {
        return core::ptr::null_mut();
    }
    let q = q_nbr as usize;
    // Peek the front token into the staging buffer without consuming it.
    let mut tmp = match cabi.recv_staging.get(q) {
        Some(buf) => buf.clone(),
        None => return core::ptr::null_mut(),
    };
    if cabi.inner.recv_peek(q, &mut tmp).is_err() {
        return core::ptr::null_mut();
    }
    if let Some(buf) = cabi.recv_staging.get_mut(q) {
        buf.copy_from_slice(&tmp);
        buf.as_mut_ptr() as *mut c_void
    } else {
        core::ptr::null_mut()
    }
}

/// Safe analogue of `intel_ipu4_recv_put_token`.
///
/// # Safety
/// See [`ctx_ref`].
#[no_mangle]
pub unsafe extern "C" fn intel_ipu4_recv_put_token(ctx: *mut c_void, q_nbr: c_int) {
    let Some(cabi) = ctx_ref(ctx) else {
        return;
    };
    if q_nbr < 0 {
        return;
    }
    let _ = cabi.inner.recv_advance(q_nbr as usize);
}
