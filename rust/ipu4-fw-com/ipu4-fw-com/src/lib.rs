//! Safe, idiomatic Rust port of the Intel IPU4 firmware communication (syscom)
//! layer originally implemented in `drivers/media/pci/intel-ipu4/`.
//!
//! The syscom layer is a shared resource between the host driver and the ISP
//! firmware: a set of token ring-buffers (queues) in DMA memory whose read/write
//! indices live in ISP dmem. The original C code addresses tokens with raw
//! pointer arithmetic and bare `readl`/`writel` on the indices, which makes it a
//! classic memory-safety and data-race hotspot.
//!
//! This crate keeps the protocol and on-the-wire layout identical while making
//! the implementation safe:
//!
//! * [`ring`] — bounds-checked ring buffer with [`AtomicU32`](std::sync::atomic::AtomicU32)
//!   indices and per-slot locking. The index arithmetic is a faithful port of
//!   the C helpers, isolated as pure functions in [`ring::index`].
//! * [`context`] — [`FwComContext`], an owning, RAII analogue of
//!   `intel_ipu4_fw_com_context`, reproducing the prepare/open/ready/close/
//!   release lifecycle.
//! * [`serialize`] — byte-exact serialization of the firmware-shared structs for
//!   golden parity testing.
//! * [`c_abi`] — (feature `c-abi`) the original C ABI re-exported on top of the
//!   safe core; the only module containing `unsafe`.
//!
//! All business logic is 100% safe Rust; the sole `unsafe` lives at the C ABI
//! boundary in [`c_abi`].
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub mod context;
pub mod error;
pub mod ring;
pub mod serialize;

#[cfg(feature = "c-abi")]
pub mod c_abi;

pub use context::{Cell, FwComConfig, FwComContext, MockCell, QueueConfig};
pub use error::{FwComError, Result};
pub use ring::{index, RecvToken, SendToken, SharedRing};
pub use serialize::{serialize_token, SyscomConfigFw, SysQueue};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ring::index::{inc_index, is_index_valid, num_free, num_messages};

    #[test]
    fn num_messages_no_wrap() {
        assert_eq!(num_messages(5, 2, 8), 3);
        assert_eq!(num_messages(2, 2, 8), 0);
    }

    #[test]
    fn num_messages_wraps() {
        // wr has wrapped past the end and is now behind rd.
        assert_eq!(num_messages(1, 6, 8), 3); // 1 + 8 - 6
        assert_eq!(num_messages(0, 7, 8), 1); // 0 + 8 - 7
    }

    #[test]
    fn num_free_complements_messages() {
        for size in [2u32, 4, 8, 16] {
            for wr in 0..size {
                for rd in 0..size {
                    assert_eq!(num_free(wr, rd, size) + num_messages(wr, rd, size), size);
                }
            }
        }
    }

    #[test]
    fn inc_index_wraps_at_size() {
        assert_eq!(inc_index(0, 4), 1);
        assert_eq!(inc_index(2, 4), 3);
        assert_eq!(inc_index(3, 4), 0); // wrap
    }

    #[test]
    fn inc_index_handles_u32_max_without_overflow() {
        // Bogus index from corrupt dmem must not panic.
        assert_eq!(inc_index(u32::MAX, 4), 0);
    }

    #[test]
    fn is_index_valid_bounds() {
        assert!(is_index_valid(0, 4));
        assert!(is_index_valid(3, 4));
        assert!(!is_index_valid(4, 4));
        assert!(!is_index_valid(u32::MAX, 4));
    }

    #[test]
    fn ring_full_and_empty_edges() {
        let ring = SharedRing::new(4, 8).unwrap();
        assert!(ring.is_empty());
        assert!(!ring.is_full());
        // Capacity is size - 1.
        assert_eq!(ring.capacity(), 3);
        for i in 0..3u8 {
            ring.try_send(&[i; 8]).unwrap();
        }
        assert!(ring.is_full());
        assert_eq!(ring.len(), 3);
        // Fourth send must be rejected.
        assert_eq!(ring.try_send(&[9u8; 8]), Err(FwComError::QueueFull));

        // Drain and verify FIFO order.
        for i in 0..3u8 {
            let mut out = [0u8; 8];
            ring.try_recv(&mut out).unwrap();
            assert_eq!(out, [i; 8]);
        }
        assert!(ring.is_empty());
        let mut out = [0u8; 8];
        assert_eq!(ring.try_recv(&mut out), Err(FwComError::QueueEmpty));
    }

    #[test]
    fn ring_wraps_around_many_times() {
        let ring = SharedRing::new(3, 4).unwrap(); // capacity 2
        let mut counter = 0u8;
        for _ in 0..100 {
            ring.try_send(&[counter; 4]).unwrap();
            let mut out = [0u8; 4];
            ring.try_recv(&mut out).unwrap();
            assert_eq!(out, [counter; 4]);
            counter = counter.wrapping_add(1);
        }
        assert!(ring.is_empty());
    }

    #[test]
    fn ring_rejects_wrong_token_size() {
        let ring = SharedRing::new(4, 8).unwrap();
        assert_eq!(
            ring.try_send(&[0u8; 4]),
            Err(FwComError::TokenSizeMismatch {
                expected: 8,
                actual: 4
            })
        );
        let mut out = [0u8; 16];
        assert_eq!(
            ring.try_recv(&mut out),
            Err(FwComError::TokenSizeMismatch {
                expected: 8,
                actual: 16
            })
        );
    }

    #[test]
    fn raii_token_commit_and_abandon() {
        let ring = SharedRing::new(4, 4).unwrap();
        // Acquire a send token, fill it, commit.
        let mut tok = ring.acquire_send().unwrap();
        tok.data_mut().copy_from_slice(&[1, 2, 3, 4]);
        tok.commit();
        assert_eq!(ring.len(), 1);

        // Acquire then drop without commit -> nothing published.
        {
            let mut tok = ring.acquire_send().unwrap();
            tok.data_mut().copy_from_slice(&[9, 9, 9, 9]);
            // dropped here
        }
        assert_eq!(ring.len(), 1);

        let tok = ring.acquire_recv().unwrap();
        assert_eq!(tok.data(), &[1, 2, 3, 4]);
        tok.commit();
        assert!(ring.is_empty());
    }

    #[test]
    fn zero_sized_config_rejected() {
        assert!(matches!(
            SharedRing::new(0, 8),
            Err(FwComError::ZeroQueueSize)
        ));
        assert!(matches!(
            SharedRing::new(4, 0),
            Err(FwComError::ZeroTokenSize)
        ));
    }
}
