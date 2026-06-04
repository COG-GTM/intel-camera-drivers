//! RAII firmware-communication context: the safe analogue of
//! `struct intel_ipu4_fw_com_context` and its lifecycle functions
//! (`prepare` / `open` / `ready` / `close` / `release`).
//!
//! # Lifetime & ownership
//!
//! The C driver allocates one big DMA buffer in `prepare`, hands raw offsets
//! into it to the firmware, and frees it in `release` (with a `force` escape
//! hatch). Forgetting `release`, or releasing while the cell is still running,
//! are use-after-free / double-free hazards.
//!
//! Here the context *owns* its queues and buffers. Memory is reclaimed
//! deterministically by [`Drop`], so "forgetting to release" cannot leak the
//! shared buffers and "releasing twice" cannot compile. [`FwComContext::release`]
//! is preserved for ABI parity (it performs the cell-state check and then simply
//! consumes `self`).

use std::sync::atomic::{AtomicU32, Ordering};

use ipu4_fw_com_sys::{
    SYSCOM_COMMAND_INACTIVE, SYSCOM_COMMAND_UNINIT, SYSCOM_STATE_INACTIVE, SYSCOM_STATE_READY,
    SYSCOM_STATE_UNINIT,
};

use crate::error::{FwComError, Result};
use crate::ring::{RecvToken, SendToken, SharedRing};
use crate::serialize::{SysQueue, SyscomConfigFw};

/// Per-queue configuration, mirroring `struct ia_css_syscom_queue_config`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueConfig {
    /// Tokens per queue (`queue_size`).
    pub queue_size: u32,
    /// Bytes per token (`token_size`).
    pub token_size: u32,
}

impl QueueConfig {
    /// Total payload bytes for this queue (`token_queue_size()` in C).
    #[inline]
    pub fn token_queue_size(&self) -> u32 {
        self.queue_size.saturating_mul(self.token_size)
    }
}

/// Top-level prepare configuration, mirroring `struct intel_ipu4_fw_com_cfg`.
#[derive(Debug, Clone, Default)]
pub struct FwComConfig {
    /// Host->SP (input) queue configurations.
    pub input: Vec<QueueConfig>,
    /// SP->host (output) queue configurations.
    pub output: Vec<QueueConfig>,
    /// dmem base offset for the register file (informational, as in the C cfg).
    pub dmem_addr: u32,
    /// Firmware-specific configuration blob.
    pub specific: Vec<u8>,
}

/// The firmware "cell" (SP). In the C driver these were two function pointers;
/// here they are a trait so callers can plug in real hardware or a mock.
pub trait Cell: Send + Sync {
    /// Returns true when the SP is in a valid state (`cell_ready`).
    fn ready(&self) -> bool;
    /// Kick the SP into running (`cell_start`).
    fn start(&self);
}

fn roundup(value: u32, align: u32) -> u32 {
    // Port of the kernel `roundup()` macro used in prepare.
    value.div_ceil(align) * align
}

/// Safe, owning firmware-communication context.
pub struct FwComContext {
    input_queues: Vec<SharedRing>,
    output_queues: Vec<SharedRing>,
    /// Descriptors handed to the firmware, retained for parity / inspection.
    input_descriptors: Vec<SysQueue>,
    output_descriptors: Vec<SysQueue>,
    config_fw: SyscomConfigFw,
    specific: Vec<u8>,
    total_size: u32,
    /// Models the dmem `SYSCOM_STATE_REG` (written by the SP).
    state: AtomicU32,
    /// Models the dmem `SYSCOM_COMMAND_REG` (written by the host).
    command: AtomicU32,
    cell: Box<dyn Cell>,
}

impl std::fmt::Debug for FwComContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FwComContext")
            .field("num_input_queues", &self.input_queues.len())
            .field("num_output_queues", &self.output_queues.len())
            .field("total_size", &self.total_size)
            .field("state", &self.state.load(Ordering::Relaxed))
            .field("command", &self.command.load(Ordering::Relaxed))
            .finish()
    }
}

impl FwComContext {
    /// Allocate and lay out the context. Safe analogue of
    /// `intel_ipu4_fw_com_prepare`.
    ///
    /// The DMA-buffer layout arithmetic from the C driver is reproduced so the
    /// firmware-facing [`SyscomConfigFw`] offsets stay byte-compatible, but the
    /// payloads are owned Rust allocations rather than a single raw DMA chunk.
    pub fn prepare(cfg: &FwComConfig, cell: Box<dyn Cell>) -> Result<Self> {
        for q in cfg.input.iter().chain(cfg.output.iter()) {
            if q.queue_size == 0 {
                return Err(FwComError::ZeroQueueSize);
            }
            if q.token_size == 0 {
                return Err(FwComError::ZeroTokenSize);
            }
        }

        let num_in = cfg.input.len() as u32;
        let num_out = cfg.output.len() as u32;
        let specific_size = cfg.specific.len() as u32;

        let size_input: u32 = cfg
            .input
            .iter()
            .map(QueueConfig::token_queue_size)
            .try_fold(0u32, |a, b| a.checked_add(b))
            .ok_or(FwComError::SizeOverflow)?;
        let size_output: u32 = cfg
            .output
            .iter()
            .map(QueueConfig::token_queue_size)
            .try_fold(0u32, |a, b| a.checked_add(b))
            .ok_or(FwComError::SizeOverflow)?;

        let sq = SysQueue::SERIALIZED_LEN as u32;
        // sizeall, matching the C accumulation exactly, with checked arithmetic.
        let size_all = (|| -> Option<u32> {
            let mut total = roundup(SyscomConfigFw::SERIALIZED_LEN as u32, 8);
            total = total.checked_add(num_in.checked_mul(sq)?)?;
            total = total.checked_add(num_out.checked_mul(sq)?)?;
            total = total.checked_add(roundup(specific_size, 8))?;
            total = total.checked_add(size_input)?;
            total = total.checked_add(size_output)?;
            Some(total)
        })()
        .ok_or(FwComError::SizeOverflow)?;

        // Walk the offsets exactly as prepare() does. dma_addr base is 0, so the
        // firmware-visible addresses are pure offsets into the buffer.
        let mut offset = roundup(SyscomConfigFw::SERIALIZED_LEN as u32, 8);

        let input_queue_addr = offset;
        offset += num_in * sq;
        let output_queue_addr = offset;
        offset += num_out * sq;
        let specific_addr = offset;
        offset += specific_size;
        let ibuf_addr = offset;
        let obuf_addr = ibuf_addr + size_input;

        // Build queues + descriptors.
        let mut input_queues = Vec::with_capacity(cfg.input.len());
        let mut input_descriptors = Vec::with_capacity(cfg.input.len());
        let mut queue_offset = 0u32;
        let mut dmem_index = 0u32;
        for q in &cfg.input {
            input_queues.push(SharedRing::new(q.queue_size, q.token_size)?);
            input_descriptors.push(SysQueue::init(
                dmem_index,
                q.queue_size,
                q.token_size,
                (ibuf_addr + queue_offset) as u64,
                ibuf_addr + queue_offset,
            ));
            queue_offset += q.token_queue_size();
            dmem_index += 1;
        }

        let mut output_queues = Vec::with_capacity(cfg.output.len());
        let mut output_descriptors = Vec::with_capacity(cfg.output.len());
        queue_offset = 0;
        for q in &cfg.output {
            output_queues.push(SharedRing::new(q.queue_size, q.token_size)?);
            output_descriptors.push(SysQueue::init(
                dmem_index,
                q.queue_size,
                q.token_size,
                (obuf_addr + queue_offset) as u64,
                obuf_addr + queue_offset,
            ));
            queue_offset += q.token_queue_size();
            dmem_index += 1;
        }

        let config_fw = SyscomConfigFw {
            firmware_address: 0,
            num_input_queues: num_in,
            num_output_queues: num_out,
            input_queue: input_queue_addr,
            output_queue: output_queue_addr,
            specific_addr,
            specific_size,
        };

        Ok(FwComContext {
            input_queues,
            output_queues,
            input_descriptors,
            output_descriptors,
            config_fw,
            specific: cfg.specific.clone(),
            total_size: size_all,
            state: AtomicU32::new(SYSCOM_STATE_UNINIT),
            command: AtomicU32::new(SYSCOM_COMMAND_UNINIT),
            cell,
        })
    }

    /// Number of host->SP (input) queues.
    pub fn num_input_queues(&self) -> usize {
        self.input_queues.len()
    }

    /// Number of SP->host (output) queues.
    pub fn num_output_queues(&self) -> usize {
        self.output_queues.len()
    }

    /// Firmware-facing config descriptor.
    pub fn config_fw(&self) -> &SyscomConfigFw {
        &self.config_fw
    }

    /// Input queue descriptors handed to the firmware.
    pub fn input_descriptors(&self) -> &[SysQueue] {
        &self.input_descriptors
    }

    /// Output queue descriptors handed to the firmware.
    pub fn output_descriptors(&self) -> &[SysQueue] {
        &self.output_descriptors
    }

    /// Firmware-specific blob copied during prepare.
    pub fn specific(&self) -> &[u8] {
        &self.specific
    }

    /// Total size of the equivalent C DMA allocation.
    pub fn total_size(&self) -> u32 {
        self.total_size
    }

    /// Open the syscom channel. Safe analogue of `intel_ipu4_fw_com_open`.
    pub fn open(&self) -> Result<()> {
        if !self.cell.ready() {
            return Err(FwComError::CellNotReady);
        }
        self.state.store(SYSCOM_STATE_UNINIT, Ordering::Release);
        self.command.store(SYSCOM_COMMAND_UNINIT, Ordering::Release);
        self.cell.start();
        Ok(())
    }

    /// Whether the SP is ready to handle messages. Safe analogue of
    /// `intel_ipu4_fw_com_ready` (`Ok(())` ⇔ C returns `0`).
    pub fn ready(&self) -> Result<()> {
        if self.state.load(Ordering::Acquire) != SYSCOM_STATE_READY {
            return Err(FwComError::Busy);
        }
        Ok(())
    }

    /// Request the channel to close. Safe analogue of
    /// `intel_ipu4_fw_com_close`.
    pub fn close(&self) -> Result<()> {
        if self.state.load(Ordering::Acquire) != SYSCOM_STATE_READY {
            return Err(FwComError::Busy);
        }
        self.command.store(SYSCOM_COMMAND_INACTIVE, Ordering::Release);
        Ok(())
    }

    /// Tear down the context. Safe analogue of `intel_ipu4_fw_com_release`.
    ///
    /// Returns `Err(Busy)` if `force` is false and the cell is still running,
    /// exactly like the C driver. On success the context is consumed and all
    /// owned buffers are freed by `Drop`.
    pub fn release(self, force: bool) -> Result<()> {
        if !force && !self.cell.ready() {
            return Err(FwComError::Busy);
        }
        drop(self);
        Ok(())
    }

    // --- token / message routines ----------------------------------------

    fn input_queue(&self, q_nbr: usize) -> Result<&SharedRing> {
        self.input_queues
            .get(q_nbr)
            .ok_or(FwComError::QueueIndexOutOfRange {
                index: q_nbr,
                count: self.input_queues.len(),
            })
    }

    fn output_queue(&self, q_nbr: usize) -> Result<&SharedRing> {
        self.output_queues
            .get(q_nbr)
            .ok_or(FwComError::QueueIndexOutOfRange {
                index: q_nbr,
                count: self.output_queues.len(),
            })
    }

    /// Send a token on input queue `q_nbr` (host -> SP).
    pub fn send(&self, q_nbr: usize, token: &[u8]) -> Result<()> {
        self.input_queue(q_nbr)?.try_send(token)
    }

    /// Receive a token from output queue `q_nbr` (SP -> host).
    pub fn recv(&self, q_nbr: usize, out: &mut [u8]) -> Result<()> {
        self.output_queue(q_nbr)?.try_recv(out)
    }

    /// RAII send handle, analogue of `intel_ipu4_send_get_token`.
    pub fn send_get_token(&self, q_nbr: usize) -> Result<SendToken<'_>> {
        self.input_queue(q_nbr)?.acquire_send()
    }

    /// RAII receive handle, analogue of `intel_ipu4_recv_get_token`.
    pub fn recv_get_token(&self, q_nbr: usize) -> Result<RecvToken<'_>> {
        self.output_queue(q_nbr)?.acquire_recv()
    }

    /// Peek the oldest token on output queue `q_nbr` without consuming it.
    pub fn recv_peek(&self, q_nbr: usize, out: &mut [u8]) -> Result<()> {
        self.output_queue(q_nbr)?.peek_front(out)
    }

    /// Release the oldest token on output queue `q_nbr` (advance read index).
    pub fn recv_advance(&self, q_nbr: usize) -> Result<()> {
        self.output_queue(q_nbr)?.advance_read()
    }

    /// Token size (bytes) of input queue `q_nbr`.
    pub fn input_token_size(&self, q_nbr: usize) -> Result<u32> {
        Ok(self.input_queue(q_nbr)?.token_size())
    }

    /// Token size (bytes) of output queue `q_nbr`.
    pub fn output_token_size(&self, q_nbr: usize) -> Result<u32> {
        Ok(self.output_queue(q_nbr)?.token_size())
    }
}

impl Drop for FwComContext {
    fn drop(&mut self) {
        // RAII teardown: mark the shared state inactive before the owned
        // buffers are reclaimed. Equivalent to the bookkeeping the C driver
        // would rely on the firmware observing after `dma_free_attrs`/`kfree`.
        self.command.store(SYSCOM_COMMAND_INACTIVE, Ordering::Release);
        self.state.store(SYSCOM_STATE_INACTIVE, Ordering::Release);
    }
}

/// A simple [`Cell`] whose readiness is controlled by an atomic flag — handy for
/// tests and for driving the lifecycle state machine off-hardware.
#[derive(Debug, Default)]
pub struct MockCell {
    ready: std::sync::atomic::AtomicBool,
    started: std::sync::atomic::AtomicBool,
}

impl MockCell {
    /// Create a mock cell with the given initial readiness.
    pub fn new(ready: bool) -> Self {
        MockCell {
            ready: std::sync::atomic::AtomicBool::new(ready),
            started: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Toggle readiness (simulates the SP coming up / going down).
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
    }

    /// Whether `start()` has been invoked.
    pub fn was_started(&self) -> bool {
        self.started.load(Ordering::Acquire)
    }
}

impl Cell for MockCell {
    fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    fn start(&self) {
        self.started.store(true, Ordering::Release);
    }
}

impl FwComContext {
    /// Test/simulation hook: emulate the SP writing `SYSCOM_STATE_READY` into
    /// dmem so `ready()`/`close()` can be exercised off-hardware.
    pub fn simulate_sp_state(&self, state: u32) {
        self.state.store(state, Ordering::Release);
    }

    /// Current value of the (SP-owned) state register.
    pub fn state(&self) -> u32 {
        self.state.load(Ordering::Acquire)
    }

    /// Current value of the (host-owned) command register.
    pub fn command(&self) -> u32 {
        self.command.load(Ordering::Acquire)
    }
}
