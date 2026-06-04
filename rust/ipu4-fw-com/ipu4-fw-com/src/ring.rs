//! Safe ring-buffer abstraction for a single syscom token queue.
//!
//! # Why this is the memory-safety hotspot
//!
//! In the original C driver each queue is a chunk of DMA memory shared between
//! the host CPU and the ISP firmware. The write/read indices live in ISP dmem
//! and are read back with `readl()`; the token payloads are addressed by raw
//! pointer arithmetic:
//!
//! ```c
//! return (void *)q->host_address + (index * q->token_size);
//! ```
//!
//! Two classes of bug are latent here:
//!
//! 1. **Unchecked index arithmetic** — a firmware that writes a bogus index into
//!    dmem turns the pointer math into an out-of-bounds access. The C code added
//!    an `is_index_valid()` guard, but the actual slot addressing is still raw.
//! 2. **Data races** — the producer advances `wr` while the consumer advances
//!    `rd` with no memory ordering on the payload.
//!
//! This module replaces both:
//!
//! * Slots are addressed by `counter % queue_size`, which is *always* in range —
//!   the out-of-bounds class is eliminated by construction, not merely guarded.
//! * The read/write positions are **free-running** [`AtomicU32`] counters
//!   (monotonic, wrapping only at `u32::MAX`) with acquire/release ordering, and
//!   every slot is an independently-locked buffer so a slot is never read while
//!   it is being written.
//!
//! ## Why free-running counters instead of wrapped dmem indices
//!
//! The C driver stores the indices *wrapped* into `0..size` (that is the dmem
//! register ABI) and recomputes occupancy with [`index::num_messages`]. That is
//! correct for the firmware because dmem is coherent and each index has a single
//! writer. It is **not** safe for a pure-software SPSC ring: an acquire load may
//! observe a *stale* value of the other side's index, and once that stale value
//! is interpreted through wrap-around modular arithmetic the producer can
//! mis-compute the free count and overrun the ring by a full lap (silently
//! losing `size` tokens). Free-running counters make a stale read always
//! *under*-estimate availability, which is always the safe direction. The
//! wrapped, dmem-style index is still exposed via [`SharedRing::write_index`] /
//! [`SharedRing::read_index`] for ABI parity, and the faithful C index math is
//! retained in [`index`] for descriptor construction and golden parity tests.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use crate::error::{FwComError, Result};

/// Pure, side-effect-free index arithmetic.
///
/// These functions are a faithful, overflow-safe port of the `static` helpers
/// in `intel-ipu4-fw-com.c` (`num_messages`, `num_free`, `inc_index`,
/// `is_index_valid`). They are deliberately free functions so they can be
/// exhaustively unit-tested in isolation.
pub mod index {
    /// Number of tokens currently queued between `wr` and `rd`.
    ///
    /// Port of:
    /// ```c
    /// if (wr < rd) wr += size;
    /// return wr - rd;
    /// ```
    /// Uses `u64` intermediates so `wr + size` cannot overflow.
    #[inline]
    pub fn num_messages(wr: u32, rd: u32, size: u32) -> u32 {
        let mut wr = wr as u64;
        let rd = rd as u64;
        if wr < rd {
            wr += size as u64;
        }
        (wr - rd) as u32
    }

    /// Number of free slots available for new tokens.
    ///
    /// Port of `return size - num_messages(wr, rd, size);`.
    #[inline]
    pub fn num_free(wr: u32, rd: u32, size: u32) -> u32 {
        size - num_messages(wr, rd, size)
    }

    /// Increment an index, wrapping back to zero at `size`.
    ///
    /// Port of `index = index + 1; return index >= size ? 0 : index;`.
    #[inline]
    pub fn inc_index(index: u32, size: u32) -> u32 {
        let next = index.wrapping_add(1);
        if next >= size {
            0
        } else {
            next
        }
    }

    /// Whether `index` is a valid slot index for a queue of `size` slots.
    ///
    /// Port of `is_index_valid()`.
    #[inline]
    pub fn is_index_valid(index: u32, size: u32) -> bool {
        index < size
    }
}

/// A single token queue backed by safe, bounds-checked storage.
///
/// A queue of `queue_size` slots can hold at most `queue_size - 1` tokens (one
/// slot is always kept free to disambiguate the full and empty states), exactly
/// as in the C ring buffer.
#[derive(Debug)]
pub struct SharedRing {
    /// Number of slots (`q->size`).
    queue_size: u32,
    /// Bytes per token (`q->token_size`).
    token_size: u32,
    /// Free-running write counter — advanced by the producer. The dmem `wr_reg`
    /// value is `wr % queue_size`.
    wr: AtomicU32,
    /// Free-running read counter — advanced by the consumer. The dmem `rd_reg`
    /// value is `rd % queue_size`.
    rd: AtomicU32,
    /// Per-slot payload storage. Each slot is independently locked so a slot is
    /// never read by the consumer while the producer is writing it.
    slots: Box<[Mutex<Box<[u8]>>]>,
}

impl SharedRing {
    /// Create a new ring with `queue_size` slots of `token_size` bytes each.
    pub fn new(queue_size: u32, token_size: u32) -> Result<Self> {
        if queue_size == 0 {
            return Err(FwComError::ZeroQueueSize);
        }
        if token_size == 0 {
            return Err(FwComError::ZeroTokenSize);
        }
        let slots = (0..queue_size)
            .map(|_| Mutex::new(vec![0u8; token_size as usize].into_boxed_slice()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(SharedRing {
            queue_size,
            token_size,
            wr: AtomicU32::new(0),
            rd: AtomicU32::new(0),
            slots,
        })
    }

    /// Number of slots in the queue (`q->size`).
    #[inline]
    pub fn queue_size(&self) -> u32 {
        self.queue_size
    }

    /// Bytes per token (`q->token_size`).
    #[inline]
    pub fn token_size(&self) -> u32 {
        self.token_size
    }

    /// Maximum number of tokens the queue can hold simultaneously.
    #[inline]
    pub fn capacity(&self) -> u32 {
        self.queue_size - 1
    }

    /// Current write index in dmem form (`wr % queue_size`), matching the value
    /// the C driver would read from the `wr_reg` dmem register.
    #[inline]
    pub fn write_index(&self) -> u32 {
        self.wr.load(Ordering::Acquire) % self.queue_size
    }

    /// Current read index in dmem form (`rd % queue_size`).
    #[inline]
    pub fn read_index(&self) -> u32 {
        self.rd.load(Ordering::Acquire) % self.queue_size
    }

    /// Map a free-running counter to a slot index (always `< queue_size`).
    #[inline]
    fn slot(&self, counter: u32) -> usize {
        (counter % self.queue_size) as usize
    }

    /// Occupancy from a pair of free-running counters.
    ///
    /// The producer never lets occupancy exceed [`capacity`](Self::capacity), so
    /// `wr.wrapping_sub(rd)` is always in `0..queue_size` even across the
    /// `u32::MAX` wrap.
    #[inline]
    fn occupancy(wr: u32, rd: u32) -> u32 {
        wr.wrapping_sub(rd)
    }

    /// Number of tokens currently queued.
    #[inline]
    pub fn len(&self) -> u32 {
        Self::occupancy(
            self.wr.load(Ordering::Acquire),
            self.rd.load(Ordering::Acquire),
        )
    }

    /// Whether the queue currently holds no tokens.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the queue cannot accept another token.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity()
    }

    /// Copy a token into the queue. Equivalent to the C `send_get_token`
    /// (find a free slot) followed by `send_put_token` (publish it), but
    /// performed atomically from the caller's perspective.
    pub fn try_send(&self, token: &[u8]) -> Result<()> {
        if token.len() != self.token_size as usize {
            return Err(FwComError::TokenSizeMismatch {
                expected: self.token_size as usize,
                actual: token.len(),
            });
        }
        // Sole writer of `wr`, so a relaxed load is accurate. A stale (Acquire)
        // `rd` can only make the ring look *fuller*, which is the safe direction.
        let wr = self.wr.load(Ordering::Relaxed);
        let rd = self.rd.load(Ordering::Acquire);
        if Self::occupancy(wr, rd) >= self.capacity() {
            return Err(FwComError::QueueFull);
        }

        {
            // Lock only this slot: the consumer can keep draining other slots.
            let mut slot = self.slots[self.slot(wr)]
                .lock()
                .expect("ring slot mutex poisoned");
            slot.copy_from_slice(token);
        }

        // Publish the write: release so the consumer observes the payload.
        self.wr.store(wr.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Copy the oldest token out of the queue into `out`. Equivalent to the C
    /// `recv_get_token` + `recv_put_token` pair.
    pub fn try_recv(&self, out: &mut [u8]) -> Result<()> {
        if out.len() != self.token_size as usize {
            return Err(FwComError::TokenSizeMismatch {
                expected: self.token_size as usize,
                actual: out.len(),
            });
        }
        // Sole writer of `rd`; a stale (Acquire) `wr` can only make the ring look
        // *emptier*, which is the safe direction.
        let rd = self.rd.load(Ordering::Relaxed);
        let wr = self.wr.load(Ordering::Acquire);
        if Self::occupancy(wr, rd) == 0 {
            return Err(FwComError::QueueEmpty);
        }

        {
            let slot = self.slots[self.slot(rd)]
                .lock()
                .expect("ring slot mutex poisoned");
            out.copy_from_slice(&slot);
        }

        self.rd.store(rd.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Copy the oldest token out **without** consuming it (peek). Mirrors the C
    /// `recv_get_token`, which returns a pointer to `slot[rd]` but leaves `rd`
    /// untouched until `recv_put_token` is called.
    pub fn peek_front(&self, out: &mut [u8]) -> Result<()> {
        if out.len() != self.token_size as usize {
            return Err(FwComError::TokenSizeMismatch {
                expected: self.token_size as usize,
                actual: out.len(),
            });
        }
        let rd = self.rd.load(Ordering::Relaxed);
        let wr = self.wr.load(Ordering::Acquire);
        if Self::occupancy(wr, rd) == 0 {
            return Err(FwComError::QueueEmpty);
        }
        let slot = self.slots[self.slot(rd)]
            .lock()
            .expect("ring slot mutex poisoned");
        out.copy_from_slice(&slot);
        Ok(())
    }

    /// Advance the read index, releasing the oldest slot. Mirrors the C
    /// `recv_put_token`. Returns [`FwComError::QueueEmpty`] if there is nothing
    /// to release.
    pub fn advance_read(&self) -> Result<()> {
        let rd = self.rd.load(Ordering::Relaxed);
        let wr = self.wr.load(Ordering::Acquire);
        if Self::occupancy(wr, rd) == 0 {
            return Err(FwComError::QueueEmpty);
        }
        self.rd.store(rd.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Acquire a writable slot without publishing it — the RAII analogue of
    /// `send_get_token`. The token is only made visible to the consumer when
    /// [`SendToken::commit`] is called; dropping the guard abandons the slot,
    /// matching a C caller that calls `get_token` but never `put_token`.
    pub fn acquire_send(&self) -> Result<SendToken<'_>> {
        let wr = self.wr.load(Ordering::Relaxed);
        let rd = self.rd.load(Ordering::Acquire);
        if Self::occupancy(wr, rd) >= self.capacity() {
            return Err(FwComError::QueueFull);
        }
        let guard = self.slots[self.slot(wr)]
            .lock()
            .expect("ring slot mutex poisoned");
        Ok(SendToken {
            ring: self,
            counter: wr,
            guard,
        })
    }

    /// Acquire the oldest readable slot without consuming it — the RAII analogue
    /// of `recv_get_token`. The slot is only released back to the producer when
    /// [`RecvToken::commit`] is called.
    pub fn acquire_recv(&self) -> Result<RecvToken<'_>> {
        let rd = self.rd.load(Ordering::Relaxed);
        let wr = self.wr.load(Ordering::Acquire);
        if Self::occupancy(wr, rd) == 0 {
            return Err(FwComError::QueueEmpty);
        }
        let guard = self.slots[self.slot(rd)]
            .lock()
            .expect("ring slot mutex poisoned");
        Ok(RecvToken {
            ring: self,
            counter: rd,
            guard,
        })
    }
}

// `SharedRing` is `Send + Sync` purely because every field already is
// (`AtomicU32`, `Box<[Mutex<..>]>`). No raw pointers, no manual `unsafe impl`.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SharedRing>();
};

/// RAII write handle to a single queue slot. See [`SharedRing::acquire_send`].
#[derive(Debug)]
pub struct SendToken<'a> {
    ring: &'a SharedRing,
    counter: u32,
    guard: std::sync::MutexGuard<'a, Box<[u8]>>,
}

impl SendToken<'_> {
    /// Mutable view of the slot's payload bytes.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.guard
    }

    /// Publish the token to the consumer and advance the write counter.
    pub fn commit(self) {
        let ring = self.ring;
        let next = self.counter.wrapping_add(1);
        // Drop the slot lock before publishing the index.
        drop(self.guard);
        ring.wr.store(next, Ordering::Release);
    }
}

/// RAII read handle to a single queue slot. See [`SharedRing::acquire_recv`].
#[derive(Debug)]
pub struct RecvToken<'a> {
    ring: &'a SharedRing,
    counter: u32,
    guard: std::sync::MutexGuard<'a, Box<[u8]>>,
}

impl RecvToken<'_> {
    /// Read-only view of the slot's payload bytes.
    pub fn data(&self) -> &[u8] {
        &self.guard
    }

    /// Release the slot back to the producer and advance the read counter.
    pub fn commit(self) {
        let ring = self.ring;
        let next = self.counter.wrapping_add(1);
        drop(self.guard);
        ring.rd.store(next, Ordering::Release);
    }
}
