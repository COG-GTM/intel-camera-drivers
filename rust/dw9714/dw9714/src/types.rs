//! Strong, validated value types for the dw9714 driver.
//!
//! The C driver passed bare `u16`s around for both "lens positions" and
//! "register words", relying on macros (`VCM_VAL`) and implicit truncation to
//! keep them straight. These newtypes make the distinction explicit and make
//! out-of-range positions unrepresentable.

use dw9714_sys::{DW9714_MAX_FOCUS_POS, VCM_DEFAULT_S, VCM_VAL};

use crate::error::DW9714Error;

/// A validated absolute lens position in the range `0..=1023`.
///
/// Construction is fallible, so an out-of-range position can never reach the
/// register-packing logic — eliminating the silent truncation present in the C
/// `VCM_VAL` macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LensPosition(u16);

impl LensPosition {
    /// The maximum representable position (`DW9714_MAX_FOCUS_POS`).
    pub const MAX: u16 = DW9714_MAX_FOCUS_POS;

    /// The fully-retracted position (`0`).
    pub const ZERO: LensPosition = LensPosition(0);

    /// Create a validated position, rejecting anything above [`Self::MAX`].
    ///
    /// # Errors
    /// Returns [`DW9714Error::PositionOutOfRange`] if `value > MAX`.
    #[inline]
    pub const fn new(value: u16) -> Result<Self, DW9714Error> {
        if value > Self::MAX {
            Err(DW9714Error::PositionOutOfRange {
                requested: value,
                max: Self::MAX,
            })
        } else {
            Ok(LensPosition(value))
        }
    }

    /// Create a position, saturating at [`Self::MAX`] instead of failing.
    #[inline]
    #[must_use]
    pub const fn new_saturating(value: u16) -> Self {
        if value > Self::MAX {
            LensPosition(Self::MAX)
        } else {
            LensPosition(value)
        }
    }

    /// The raw 10-bit DAC value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// Pack this position into the 16-bit register word using the supplied
    /// mode/step nibble, faithfully reproducing the C `VCM_VAL` macro.
    #[inline]
    #[must_use]
    pub const fn register_value(self, step: VcmStep) -> RegisterValue {
        RegisterValue(VCM_VAL(self.0, step.0))
    }

    /// Pack this position with the default mode/step ([`VcmStep::DEFAULT`]).
    #[inline]
    #[must_use]
    pub const fn default_register_value(self) -> RegisterValue {
        self.register_value(VcmStep::DEFAULT)
    }
}

/// The 4-bit mode/step (`S`) nibble of the dw9714 register word.
///
/// Only the low nibble is meaningful; higher bits are masked off on
/// construction so a bad value cannot corrupt the position field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VcmStep(u16);

impl VcmStep {
    /// `VCM_DEFAULT_S` (`0x0`).
    pub const DEFAULT: VcmStep = VcmStep(VCM_DEFAULT_S);

    /// Create a step nibble, masking to the low 4 bits.
    #[inline]
    #[must_use]
    pub const fn new(value: u16) -> Self {
        VcmStep(value & 0x0f)
    }

    /// The raw nibble value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl Default for VcmStep {
    fn default() -> Self {
        VcmStep::DEFAULT
    }
}

/// A fully-packed 16-bit dw9714 register word ready for an I2C write.
///
/// Distinct from [`LensPosition`] so that "a position" and "the bytes that
/// encode it" can never be confused at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterValue(u16);

impl RegisterValue {
    /// The raw 16-bit word.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// The big-endian byte encoding sent over I2C.
    ///
    /// Mirrors the C driver's `cpu_to_be16(data)` followed by writing the
    /// raw bytes.
    #[inline]
    #[must_use]
    pub const fn to_be_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DW9714Error;

    #[test]
    fn lens_position_accepts_full_range() {
        assert_eq!(LensPosition::new(0).unwrap().get(), 0);
        assert_eq!(LensPosition::new(1023).unwrap().get(), 1023);
    }

    #[test]
    fn lens_position_rejects_out_of_range() {
        assert_eq!(
            LensPosition::new(1024),
            Err(DW9714Error::PositionOutOfRange {
                requested: 1024,
                max: 1023
            })
        );
        assert_eq!(
            LensPosition::new(u16::MAX),
            Err(DW9714Error::PositionOutOfRange {
                requested: u16::MAX,
                max: 1023
            })
        );
    }

    #[test]
    fn lens_position_saturates() {
        assert_eq!(LensPosition::new_saturating(5000).get(), 1023);
        assert_eq!(LensPosition::new_saturating(100).get(), 100);
    }

    #[test]
    fn vcm_step_masks_to_nibble() {
        assert_eq!(VcmStep::new(0xff).get(), 0x0f);
        assert_eq!(VcmStep::new(0x3).get(), 0x3);
        assert_eq!(VcmStep::DEFAULT.get(), 0);
    }

    #[test]
    fn register_value_matches_c_macro() {
        // VCM_VAL(data, s) == (data << 4) | s
        let pos = LensPosition::new(512).unwrap();
        assert_eq!(pos.default_register_value().get(), 512 << 4);
        assert_eq!(
            pos.register_value(VcmStep::new(0x5)).get(),
            (512 << 4) | 0x5
        );
        // Big-endian encoding, like cpu_to_be16.
        assert_eq!(
            pos.default_register_value().to_be_bytes(),
            (512u16 << 4).to_be_bytes()
        );
    }

    #[test]
    fn register_value_for_max_position_fits_u16() {
        let pos = LensPosition::new(1023).unwrap();
        assert_eq!(pos.default_register_value().get(), 1023 << 4); // 16368
    }
}
