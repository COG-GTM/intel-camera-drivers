//! The safe dw9714 device driver — the ported business logic.
//!
//! This is a faithful, 100% safe-Rust port of the logic in
//! `drivers/media/i2c/dw9714.c`:
//!
//! | C function                | Rust equivalent                         |
//! |---------------------------|-----------------------------------------|
//! | `dw9714_i2c_write`        | [`Dw9714::write_word`] (private)         |
//! | `dw9714_t_focus_vcm`      | [`Dw9714::set_position`]                 |
//! | `dw9714_set_ctrl`         | [`Dw9714::set_ctrl`]                      |
//! | `dw9714_init_controls`    | [`Dw9714::init`]                         |
//! | `dw9714_probe`            | [`Dw9714::probe`] / [`Dw9714::new`]      |
//! | `dw9714_remove`           | `Drop for Dw9714`                        |
//! | `dw9714_open`             | [`Dw9714::open`] (RAII)                   |
//! | `dw9714_close`            | `Drop for PowerGuard`                    |
//! | `dw9714_runtime_suspend`  | [`Dw9714::runtime_suspend`]              |
//! | `dw9714_runtime_resume`   | [`Dw9714::runtime_resume`]               |
//!
//! There is **no `unsafe` in this module** — all hardware access goes through
//! the [`Dw9714Bus`] trait.

use dw9714_sys::{DW9714_CTRL_DELAY_US, DW9714_CTRL_STEPS, V4L2_CID_FOCUS_ABSOLUTE};

use crate::bus::Dw9714Bus;
use crate::error::{DW9714Error, Result};
use crate::types::{LensPosition, RegisterValue, VcmStep};

/// Platform configuration for a dw9714 instance.
///
/// Mirrors the parts of `struct i2c_client` / `struct dw9714_platform_data`
/// the driver actually consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceConfig {
    /// I2C slave address (`client->addr`).
    pub i2c_addr: u16,
    /// xshutdown GPIO number, or `None` if unused (C: `gpio_xsd < 0`).
    pub gpio_xsd: Option<u32>,
    /// Whether a gating sensor device is present (`pdata->sensor_dev`).
    pub has_sensor_dev: bool,
}

impl DeviceConfig {
    /// A minimal config with just an I2C address and no platform data
    /// (equivalent to `pdata == NULL`).
    #[must_use]
    pub const fn new(i2c_addr: u16) -> Self {
        Self {
            i2c_addr,
            gpio_xsd: None,
            has_sensor_dev: false,
        }
    }
}

/// Safe RAII driver handle for a dw9714 VCM lens actuator.
///
/// Owns the [`Dw9714Bus`] it drives. Construction corresponds to `probe`;
/// dropping corresponds to `remove`. The power lifecycle (`open`/`close`,
/// runtime suspend/resume) is managed via [`Dw9714::open`] and the returned
/// [`PowerGuard`].
///
/// ## Thread-safety
///
/// `Dw9714<B>` derives `Send`/`Sync` automatically from `B` — it is `Send`
/// iff `B: Send` and `Sync` iff `B: Sync`. The kernel-backed bus holds raw
/// pointers and is therefore neither, which correctly prevents the handle from
/// crossing threads. No `unsafe impl` is used, so these bounds are only ever
/// granted when provably correct.
#[derive(Debug)]
pub struct Dw9714<B: Dw9714Bus> {
    bus: B,
    config: DeviceConfig,
    /// Last value programmed into the DAC (`dw9714_device.current_val`).
    current_val: u16,
    /// Whether `init`/`probe` has run (`pm_runtime_enable` done).
    initialized: bool,
    /// Whether the device is currently powered/resumed.
    powered: bool,
}

impl<B: Dw9714Bus> Dw9714<B> {
    /// Build an **un-initialized** handle (no hardware touched yet).
    ///
    /// Call [`init`](Self::init) before use, or use [`probe`](Self::probe) to
    /// do both at once.
    #[must_use]
    pub fn new(bus: B, config: DeviceConfig) -> Self {
        Self {
            bus,
            config,
            current_val: 0,
            initialized: false,
            powered: false,
        }
    }

    /// Initialize the device: set up the focus control and enable runtime PM.
    ///
    /// Port of `dw9714_init_controls` + `pm_runtime_enable` from
    /// `dw9714_probe`.
    ///
    /// # Errors
    /// Returns [`DW9714Error::AlreadyInitialized`] if called twice (the
    /// double-init guard the C code lacked).
    pub fn init(&mut self) -> Result<()> {
        if self.initialized {
            return Err(DW9714Error::AlreadyInitialized);
        }
        // The C driver registers a single V4L2_CID_FOCUS_ABSOLUTE control with
        // range [0, DW9714_MAX_FOCUS_POS], step 1, default 0. We model that as
        // the validated LensPosition domain plus current_val == 0.
        self.current_val = 0;
        self.initialized = true;
        Ok(())
    }

    /// Build and initialize in one step (the common `probe` path).
    ///
    /// # Errors
    /// Propagates [`init`](Self::init) failures.
    pub fn probe(bus: B, config: DeviceConfig) -> Result<Self> {
        let mut dev = Self::new(bus, config);
        dev.init()?;
        Ok(dev)
    }

    /// The last position programmed into the DAC.
    #[must_use]
    pub fn position(&self) -> LensPosition {
        // current_val is always a value that previously passed LensPosition
        // validation (or 0), so saturation never actually clamps.
        LensPosition::new_saturating(self.current_val)
    }

    /// The raw `current_val` register field.
    #[must_use]
    pub fn current_val(&self) -> u16 {
        self.current_val
    }

    /// Whether the device has been initialized (`probe`d).
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Whether the device is currently powered/resumed.
    #[must_use]
    pub fn is_powered(&self) -> bool {
        self.powered
    }

    /// Borrow the underlying bus (useful for inspecting a mock in tests).
    #[must_use]
    pub fn bus(&self) -> &B {
        &self.bus
    }

    /// Mutably borrow the underlying bus (e.g. to reconfigure a mock between
    /// test phases). Does not affect driver state.
    #[must_use]
    pub fn bus_mut(&mut self) -> &mut B {
        &mut self.bus
    }

    /// The device configuration.
    #[must_use]
    pub fn config(&self) -> &DeviceConfig {
        &self.config
    }

    fn ensure_initialized(&self) -> Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(DW9714Error::NotInitialized)
        }
    }

    /// Port of `dw9714_i2c_write`: write a 16-bit word big-endian with a
    /// single retry, mapping a persistent failure to `-EIO`.
    fn write_word(&mut self, word: RegisterValue) -> Result<()> {
        let bytes = word.to_be_bytes();
        let addr = self.config.i2c_addr;
        if self.bus.i2c_write(addr, &bytes).is_ok() {
            return Ok(());
        }
        // One retry, exactly like the C driver.
        if self.bus.i2c_write(addr, &bytes).is_ok() {
            return Ok(());
        }
        Err(DW9714Error::Io)
    }

    /// Port of `dw9714_t_focus_vcm`: record the new position and program it.
    ///
    /// Faithful to the C ordering: `current_val` is updated *before* the I2C
    /// write and remains updated even if the write fails.
    ///
    /// # Errors
    /// [`DW9714Error::NotInitialized`] if not yet `probe`d, or
    /// [`DW9714Error::Io`] on a persistent I2C failure.
    pub fn set_position(&mut self, val: LensPosition) -> Result<()> {
        self.ensure_initialized()?;
        self.current_val = val.get();
        self.write_word(val.default_register_value())
    }

    /// Convenience wrapper validating a raw `u16` into a [`LensPosition`]
    /// first.
    ///
    /// # Errors
    /// [`DW9714Error::PositionOutOfRange`] if `pos > LensPosition::MAX`, plus
    /// any error from [`set_position`](Self::set_position).
    pub fn set_position_raw(&mut self, pos: u16) -> Result<()> {
        let pos = LensPosition::new(pos)?;
        self.set_position(pos)
    }

    /// Port of `dw9714_set_ctrl`: only `V4L2_CID_FOCUS_ABSOLUTE` is accepted.
    ///
    /// Unlike the C code, the value is range-validated here instead of relying
    /// on the V4L2 core to pre-clamp it.
    ///
    /// # Errors
    /// [`DW9714Error::InvalidArgument`] for an unsupported control id or a
    /// negative value; [`DW9714Error::PositionOutOfRange`] for a value above
    /// the maximum.
    pub fn set_ctrl(&mut self, id: u32, val: i32) -> Result<()> {
        if id != V4L2_CID_FOCUS_ABSOLUTE {
            return Err(DW9714Error::InvalidArgument);
        }
        if val < 0 {
            return Err(DW9714Error::InvalidArgument);
        }
        // Reject (rather than truncate) anything that doesn't fit a u16; values
        // in `(u16::MAX, i32::MAX]` would otherwise wrap via `as u16` and
        // silently program the wrong position. `requested` is saturated to
        // `u16::MAX` since the error field can't hold the full i32.
        let raw = u16::try_from(val).map_err(|_| DW9714Error::PositionOutOfRange {
            requested: u16::MAX,
            max: LensPosition::MAX,
        })?;
        let pos = LensPosition::new(raw)?;
        self.set_position(pos)
    }

    /// Pack and write an internally-generated ramp value (always in range).
    fn write_ramp_value(&mut self, val: u16) -> Result<()> {
        let word = LensPosition::new_saturating(val).register_value(VcmStep::DEFAULT);
        self.write_word(word)
    }

    /// Port of `dw9714_runtime_suspend`: ramp the lens down to 0 in
    /// `DW9714_CTRL_STEPS` decrements, then drop power.
    ///
    /// Mirrors the C semantics exactly, including that per-write I2C errors are
    /// logged-and-ignored and the function still returns success.
    ///
    /// # Errors
    /// Returns `Ok(())` in normal operation; reserved `Result` for API
    /// symmetry and future PM-error propagation.
    pub fn runtime_suspend(&mut self) -> Result<()> {
        self.ensure_initialized()?;
        let steps = i32::from(DW9714_CTRL_STEPS);
        // val = current_val & ~(DW9714_CTRL_STEPS - 1)
        let start = i32::from(self.current_val) & !(steps - 1);
        let mut val = start;
        while val >= 0 {
            // Per the C driver, ignore individual write failures here.
            let _ = self.write_ramp_value(val as u16);
            self.bus.delay_us(DW9714_CTRL_DELAY_US);
            val -= steps;
        }

        if self.config.gpio_xsd.is_some() {
            let _ = self.bus.set_gpio_xsd(false);
        }
        if self.config.has_sensor_dev {
            self.bus.sensor_power_put();
        }
        self.powered = false;
        Ok(())
    }

    /// Port of `dw9714_runtime_resume`: power up, ramp the lens up from the
    /// nearest step boundary, then re-apply `current_val`.
    ///
    /// # Errors
    /// [`DW9714Error::Io`] if the gating sensor could not be powered up; in
    /// that case power is released again (mirroring the C `out:` cleanup).
    pub fn runtime_resume(&mut self) -> Result<()> {
        self.ensure_initialized()?;

        if self.config.has_sensor_dev && self.bus.sensor_power_get().is_err() {
            // C: pm_runtime_get_sync(sensor_dev) < 0 -> goto out, where `ret`
            // is negative (truthy) and sensor_dev is set, so pm_runtime_put_sync
            // *is* called. get_sync increments the usage count even on failure,
            // so the matching put is required to avoid leaking a PM reference.
            self.bus.sensor_power_put();
            return Err(DW9714Error::Io);
        }
        if self.config.gpio_xsd.is_some() && self.bus.set_gpio_xsd(true).is_err() {
            // GPIO failure: undo the sensor power we just took, mirroring the
            // C cleanup path that releases sensor_dev on the error exit.
            if self.config.has_sensor_dev {
                self.bus.sensor_power_put();
            }
            return Err(DW9714Error::Io);
        }

        let steps = i32::from(DW9714_CTRL_STEPS);
        let target = i32::from(self.current_val);
        // val = current_val % DW9714_CTRL_STEPS
        let mut val = target % steps;
        // val < current_val + DW9714_CTRL_STEPS - 1
        while val < target + steps - 1 {
            let _ = self.write_ramp_value(val as u16);
            self.bus.delay_us(DW9714_CTRL_DELAY_US);
            val += steps;
        }

        // C: v4l2_ctrl_handler_setup() re-applies the current control value,
        // i.e. re-programs current_val. Reproduce by re-writing current_val.
        let restore = LensPosition::new_saturating(self.current_val);
        let ret = self.write_word(restore.default_register_value());
        if let Err(e) = ret {
            if self.config.has_sensor_dev {
                self.bus.sensor_power_put();
            }
            return Err(e);
        }
        self.powered = true;
        Ok(())
    }

    /// Port of `dw9714_open`: resume the device and return an RAII
    /// [`PowerGuard`] that suspends it again on drop (`dw9714_close`).
    ///
    /// # Errors
    /// Propagates [`runtime_resume`](Self::runtime_resume) failures.
    pub fn open(&mut self) -> Result<PowerGuard<'_, B>> {
        self.runtime_resume()?;
        Ok(PowerGuard { device: self })
    }
}

impl<B: Dw9714Bus> Drop for Dw9714<B> {
    /// Port of `dw9714_remove`: disable runtime PM and tear down. Rust's
    /// ownership frees the rest; this just records the lifecycle transition.
    fn drop(&mut self) {
        self.initialized = false;
    }
}

/// RAII guard representing an *open* (powered/resumed) dw9714 device.
///
/// Obtained from [`Dw9714::open`]. While held, the device is powered and lens
/// positions can be set. Dropping the guard runs the equivalent of
/// `dw9714_close` (`pm_runtime_put` → `runtime_suspend`), ramping the lens
/// down and dropping power — so power can never be leaked by a forgotten
/// `close`.
#[derive(Debug)]
pub struct PowerGuard<'a, B: Dw9714Bus> {
    device: &'a mut Dw9714<B>,
}

impl<B: Dw9714Bus> core::ops::Deref for PowerGuard<'_, B> {
    type Target = Dw9714<B>;
    fn deref(&self) -> &Self::Target {
        self.device
    }
}

impl<B: Dw9714Bus> core::ops::DerefMut for PowerGuard<'_, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.device
    }
}

impl<B: Dw9714Bus> Drop for PowerGuard<'_, B> {
    fn drop(&mut self) {
        // dw9714_close -> pm_runtime_put -> runtime_suspend. Best-effort:
        // Drop cannot propagate errors, and the C close path also ignores
        // per-write failures.
        let _ = self.device.runtime_suspend();
    }
}
