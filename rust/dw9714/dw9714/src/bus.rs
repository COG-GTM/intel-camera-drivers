//! Hardware abstraction for the dw9714 driver.
//!
//! The original C driver reached straight for kernel symbols (`i2c_transfer`,
//! `gpio_set_value`, `pm_runtime_*`, `usleep_range`). To keep the ported
//! business logic in 100% safe Rust — and to make it testable without a kernel
//! — all hardware access is funnelled through the [`Dw9714Bus`] trait. The
//! kernel-backed implementation lives behind the FFI shim; tests use the
//! in-memory [`MockI2cBus`](crate::mock::MockI2cBus).

/// A low-level transfer error reported by a [`Dw9714Bus`] implementation.
///
/// Deliberately opaque: the driver only distinguishes "the transfer worked"
/// from "it did not". Mapping to the kernel `-EIO` happens one level up, in
/// the device logic, after the retry policy has been applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusError;

impl core::fmt::Display for BusError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "i2c bus transfer error")
    }
}

impl core::error::Error for BusError {}

/// Abstraction over the I2C bus and platform controls the dw9714 needs.
///
/// Implementors must provide [`i2c_write`](Dw9714Bus::i2c_write); the platform
/// hooks default to no-ops so a minimal mock (or a board without an xshutdown
/// GPIO / gating sensor) only implements what it actually has.
///
/// Each call corresponds to exactly one hardware action, mirroring the
/// granularity of the C driver so the retry/ramp logic ports verbatim.
pub trait Dw9714Bus {
    /// Perform a single I2C write of `bytes` to slave `addr`.
    ///
    /// This is **one** bus transaction with no retry — the retry policy from
    /// the C `dw9714_i2c_write` is implemented by the device layer on top of
    /// this primitive.
    ///
    /// # Errors
    /// Returns [`BusError`] if the transfer did not complete.
    fn i2c_write(&mut self, addr: u16, bytes: &[u8]) -> Result<(), BusError>;

    /// Busy/sleep for approximately `us` microseconds.
    ///
    /// Mirrors `usleep_range(us, us + 10)`. Defaults to a no-op, which is what
    /// tests and host builds want.
    fn delay_us(&mut self, us: u32) {
        let _ = us;
    }

    /// Drive the xshutdown GPIO (`gpio_xsd`). `on == true` corresponds to
    /// `gpio_set_value(gpio, 1)`.
    ///
    /// # Errors
    /// Returns [`BusError`] if the GPIO could not be driven.
    fn set_gpio_xsd(&mut self, on: bool) -> Result<(), BusError> {
        let _ = on;
        Ok(())
    }

    /// Acquire a runtime-PM reference on the gating sensor device
    /// (`pm_runtime_get_sync(sensor_dev)`).
    ///
    /// # Errors
    /// Returns [`BusError`] if the sensor could not be powered up.
    fn sensor_power_get(&mut self) -> Result<(), BusError> {
        Ok(())
    }

    /// Release a runtime-PM reference on the gating sensor device
    /// (`pm_runtime_put(sensor_dev)`).
    fn sensor_power_put(&mut self) {}
}
