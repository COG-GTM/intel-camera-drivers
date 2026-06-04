//! C ABI re-exports of the safe parser (enabled by the `c-abi` feature).
//!
//! These `#[no_mangle] extern "C"` functions let the safe Rust parser stand in
//! for the original C call sites. They form the **only** layer in this crate
//! that contains `unsafe`: each function must turn caller-supplied raw pointers
//! into slices, and every such `unsafe` block is documented inline with the
//! exact safety contract the caller must uphold.
//!
//! The functions are prefixed `ipu4_cpd_rs_` to avoid clashing with the kernel
//! symbols during side-by-side testing; a drop-in deployment would rename them
//! to the original `intel_ipu4_cpd_*` names.

use core::ffi::{c_int, c_uint, c_void};

use crate::parser::{validate_cpd_file_with_release, CpdFile};

/// Build a `&[u8]` from a raw pointer + length.
///
/// # Safety
/// `ptr` must either be null (handled by the caller) or point to at least `len`
/// bytes that remain valid and immutable for the duration of the returned
/// slice's use. This is the single unsafe primitive the shim relies on.
unsafe fn as_slice<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: caller guarantees `ptr` is valid for `len` bytes (see fn docs).
    Some(core::slice::from_raw_parts(ptr, len))
}

/// C ABI: validate a CPD file. Returns `0` on success, `-EINVAL` (-22) on any
/// validation failure or a null/invalid pointer.
///
/// Mirrors `intel_ipu4_cpd_validate_cpd_file`, with `expected_fw_pkg_release`
/// supplied explicitly in place of the compile-time `IA_CSS_FW_PKG_RELEASE`.
///
/// # Safety
/// `cpd_file` must point to `cpd_file_size` readable bytes (or be null).
#[no_mangle]
pub unsafe extern "C" fn ipu4_cpd_rs_validate_cpd_file(
    cpd_file: *const c_void,
    cpd_file_size: usize,
    expected_fw_pkg_release: u32,
) -> c_int {
    // SAFETY: forwarding the caller's pointer/length contract to `as_slice`.
    let buf = match unsafe { as_slice(cpd_file as *const u8, cpd_file_size) } {
        Some(b) => b,
        None => return -22, // -EINVAL
    };
    match validate_cpd_file_with_release(buf, expected_fw_pkg_release) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}

/// C ABI: port of `intel_ipu4_cpd_get_pg_icache_base`.
///
/// Returns the component's icache base offset, or `0` if parsing fails.
///
/// # Safety
/// `cpd_file` must point to `cpd_file_size` readable bytes (or be null).
#[no_mangle]
pub unsafe extern "C" fn ipu4_cpd_rs_get_pg_icache_base(
    idx: u8,
    cpd_file: *const c_void,
    cpd_file_size: c_uint,
) -> u32 {
    // SAFETY: forwarding the caller's pointer/length contract to `as_slice`.
    let buf = match unsafe { as_slice(cpd_file as *const u8, cpd_file_size as usize) } {
        Some(b) => b,
        None => return 0,
    };
    CpdFile::parse(buf)
        .and_then(|f| f.pg_icache_base(idx as u32))
        .unwrap_or(0)
}

/// C ABI: port of `intel_ipu4_cpd_get_pg_entry_point`.
///
/// # Safety
/// `cpd_file` must point to `cpd_file_size` readable bytes (or be null).
#[no_mangle]
pub unsafe extern "C" fn ipu4_cpd_rs_get_pg_entry_point(
    idx: u8,
    cpd_file: *const c_void,
    cpd_file_size: c_uint,
) -> u32 {
    // SAFETY: forwarding the caller's pointer/length contract to `as_slice`.
    let buf = match unsafe { as_slice(cpd_file as *const u8, cpd_file_size as usize) } {
        Some(b) => b,
        None => return 0,
    };
    CpdFile::parse(buf)
        .and_then(|f| f.pg_entry_point(idx as u32))
        .unwrap_or(0)
}

/// Borrow a pkg_dir as `&[u64]` from a raw pointer.
///
/// # Safety
/// `pkg_dir` must point to a contiguous run of `len` `u64` values (or be null).
unsafe fn pkg_dir_slice<'a>(pkg_dir: *const u64, len: usize) -> Option<&'a [u64]> {
    if pkg_dir.is_null() {
        return None;
    }
    // SAFETY: caller guarantees `pkg_dir` is valid for `len` u64s (see docs).
    Some(core::slice::from_raw_parts(pkg_dir, len))
}

/// C ABI: port of `intel_ipu4_cpd_pkg_dir_get_num_entries`.
///
/// Unlike the kernel original (which trusted the pointer), this takes an
/// explicit `len` so the read of `pkg_dir[1]` is bounds-checked.
///
/// # Safety
/// `pkg_dir` must point to `len` readable `u64` values (or be null).
#[no_mangle]
pub unsafe extern "C" fn ipu4_cpd_rs_pkg_dir_get_num_entries(
    pkg_dir: *const u64,
    len: usize,
) -> c_uint {
    // SAFETY: forwarding the caller's pointer/length contract.
    match unsafe { pkg_dir_slice(pkg_dir, len) } {
        Some(s) if s.len() > 1 => s[1] as c_uint,
        _ => 0,
    }
}

/// C ABI: port of `intel_ipu4_cpd_pkg_dir_get_address` (truncated to
/// `unsigned int`, matching the C return type).
///
/// # Safety
/// `pkg_dir` must point to `len` readable `u64` values (or be null).
#[no_mangle]
pub unsafe extern "C" fn ipu4_cpd_rs_pkg_dir_get_address(
    pkg_dir: *const u64,
    len: usize,
    pkg_dir_idx: c_int,
) -> c_uint {
    pkg_dir_field(pkg_dir, len, pkg_dir_idx, Field::Address)
}

/// C ABI: port of `intel_ipu4_cpd_pkg_dir_get_size`.
///
/// # Safety
/// `pkg_dir` must point to `len` readable `u64` values (or be null).
#[no_mangle]
pub unsafe extern "C" fn ipu4_cpd_rs_pkg_dir_get_size(
    pkg_dir: *const u64,
    len: usize,
    pkg_dir_idx: c_int,
) -> c_uint {
    pkg_dir_field(pkg_dir, len, pkg_dir_idx, Field::Size)
}

/// C ABI: port of `intel_ipu4_cpd_pkg_dir_get_type`.
///
/// # Safety
/// `pkg_dir` must point to `len` readable `u64` values (or be null).
#[no_mangle]
pub unsafe extern "C" fn ipu4_cpd_rs_pkg_dir_get_type(
    pkg_dir: *const u64,
    len: usize,
    pkg_dir_idx: c_int,
) -> c_uint {
    pkg_dir_field(pkg_dir, len, pkg_dir_idx, Field::Type)
}

enum Field {
    Address,
    Size,
    Type,
}

/// # Safety
/// `pkg_dir` must point to `len` readable `u64` values (or be null).
unsafe fn pkg_dir_field(
    pkg_dir: *const u64,
    len: usize,
    pkg_dir_idx: c_int,
    field: Field,
) -> c_uint {
    if pkg_dir_idx < 0 {
        return 0;
    }
    // SAFETY: forwarding the caller's pointer/length contract.
    let words = match unsafe { pkg_dir_slice(pkg_dir, len) } {
        Some(s) => s,
        None => return 0,
    };
    // Reconstruct a borrowed PkgDir view by reusing the accessor math.
    let idx = pkg_dir_idx as usize;
    let base = match idx.checked_add(1).and_then(|v| v.checked_mul(2)) {
        Some(b) if b + 1 < words.len() => b,
        _ => return 0,
    };
    match field {
        Field::Address => words[base] as c_uint,
        Field::Size => (words[base + 1] & crate::consts::PKG_DIR_SIZE_MASK) as c_uint,
        Field::Type => {
            ((words[base + 1] >> crate::consts::PKG_DIR_ID_SHIFT) & crate::consts::PKG_DIR_ID_MASK)
                as c_uint
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_pointers_are_handled() {
        // SAFETY: passing a null pointer is part of the documented contract.
        unsafe {
            assert_eq!(ipu4_cpd_rs_validate_cpd_file(core::ptr::null(), 0, 0), -22);
            assert_eq!(ipu4_cpd_rs_get_pg_icache_base(0, core::ptr::null(), 0), 0);
            assert_eq!(ipu4_cpd_rs_get_pg_entry_point(0, core::ptr::null(), 0), 0);
            assert_eq!(
                ipu4_cpd_rs_pkg_dir_get_num_entries(core::ptr::null(), 0),
                0
            );
            assert_eq!(ipu4_cpd_rs_pkg_dir_get_address(core::ptr::null(), 0, 0), 0);
        }
    }

    #[test]
    fn pkg_dir_accessors_match_safe_api() {
        // header pair + one entry pair: addr=0x1000, size=0x20, type/id=5
        let words: [u64; 4] = [
            crate::consts::PKG_DIR_HDR_MARK,
            2,
            0x1000,
            0x20 | (5u64 << crate::consts::PKG_DIR_ID_SHIFT),
        ];
        // SAFETY: `words` is a valid array of 4 u64s for the duration of the call.
        unsafe {
            let p = words.as_ptr();
            assert_eq!(ipu4_cpd_rs_pkg_dir_get_num_entries(p, words.len()), 2);
            assert_eq!(ipu4_cpd_rs_pkg_dir_get_address(p, words.len(), 0), 0x1000);
            assert_eq!(ipu4_cpd_rs_pkg_dir_get_size(p, words.len(), 0), 0x20);
            assert_eq!(ipu4_cpd_rs_pkg_dir_get_type(p, words.len(), 0), 5);
            // Out-of-range index returns 0 rather than overreading.
            assert_eq!(ipu4_cpd_rs_pkg_dir_get_address(p, words.len(), 9), 0);
        }
    }
}
