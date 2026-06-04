//! In-memory mock I2C backend.
//!
//! [`MockI2cBus`] implements [`Dw9714Bus`](crate::bus::Dw9714Bus) entirely in
//! software, recording every hardware action so tests and benchmarks can run
//! without real hardware or kernel headers. It can also be programmed to fail
//! transfers, which is how the I2C-error and retry paths are exercised.

use crate::bus::{BusError, Dw9714Bus};

/// A recorded I2C write: `(slave_addr, payload_bytes)`.
pub type RecordedWrite = (u16, Vec<u8>);

/// Software I2C backend that records activity and can inject failures.
#[derive(Debug, Default, Clone)]
pub struct MockI2cBus {
    /// Every successful *and* attempted write, in order. A write is recorded
    /// on every attempt (including ones forced to fail) so retry behaviour is
    /// observable.
    pub writes: Vec<RecordedWrite>,
    /// Microsecond delays requested, in order.
    pub delays_us: Vec<u32>,
    /// xshutdown GPIO transitions requested, in order.
    pub gpio_states: Vec<bool>,
    /// Number of `sensor_power_get` calls.
    pub power_get_count: usize,
    /// Number of `sensor_power_put` calls.
    pub power_put_count: usize,

    /// If `true`, every `i2c_write` fails.
    fail_all: bool,
    /// Number of leading `i2c_write` calls to fail before succeeding. Useful
    /// for exercising the single-retry path (set to `1`).
    fail_first_n: u32,
    /// Total `i2c_write` attempts seen so far.
    attempts: u32,
    /// If `true`, `sensor_power_get` fails.
    fail_sensor_power: bool,
    /// If `true`, `set_gpio_xsd` fails.
    fail_gpio: bool,
}

impl MockI2cBus {
    /// A pristine, always-succeeding bus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: make every I2C write fail (permanent bus fault).
    #[must_use]
    pub fn failing() -> Self {
        Self {
            fail_all: true,
            ..Self::default()
        }
    }

    /// Builder: fail the first `n` I2C writes, then succeed. With `n == 1`
    /// this models a transient glitch recovered by the driver's single retry.
    #[must_use]
    pub fn failing_first(n: u32) -> Self {
        Self {
            fail_first_n: n,
            ..Self::default()
        }
    }

    /// Builder: make `sensor_power_get` fail (resume power-up failure).
    #[must_use]
    pub fn with_failing_sensor_power(mut self) -> Self {
        self.fail_sensor_power = true;
        self
    }

    /// Builder: make `set_gpio_xsd` fail.
    #[must_use]
    pub fn with_failing_gpio(mut self) -> Self {
        self.fail_gpio = true;
        self
    }

    /// The most recently written register word (big-endian decoded), if any.
    #[must_use]
    pub fn last_word(&self) -> Option<u16> {
        self.writes
            .last()
            .filter(|(_, b)| b.len() == 2)
            .map(|(_, b)| u16::from_be_bytes([b[0], b[1]]))
    }

    /// All written register words, big-endian decoded (2-byte writes only).
    #[must_use]
    pub fn words(&self) -> Vec<u16> {
        self.writes
            .iter()
            .filter(|(_, b)| b.len() == 2)
            .map(|(_, b)| u16::from_be_bytes([b[0], b[1]]))
            .collect()
    }

    /// Clear all recorded activity (but keep failure configuration).
    pub fn clear(&mut self) {
        self.writes.clear();
        self.delays_us.clear();
        self.gpio_states.clear();
        self.power_get_count = 0;
        self.power_put_count = 0;
        self.attempts = 0;
    }

    fn should_fail_write(&self) -> bool {
        self.fail_all || self.attempts <= self.fail_first_n
    }
}

impl Dw9714Bus for MockI2cBus {
    fn i2c_write(&mut self, addr: u16, bytes: &[u8]) -> Result<(), BusError> {
        self.attempts += 1;
        // Record the attempt regardless of outcome so retries are visible.
        self.writes.push((addr, bytes.to_vec()));
        if self.should_fail_write() {
            Err(BusError)
        } else {
            Ok(())
        }
    }

    fn delay_us(&mut self, us: u32) {
        self.delays_us.push(us);
    }

    fn set_gpio_xsd(&mut self, on: bool) -> Result<(), BusError> {
        if self.fail_gpio {
            return Err(BusError);
        }
        self.gpio_states.push(on);
        Ok(())
    }

    fn sensor_power_get(&mut self) -> Result<(), BusError> {
        if self.fail_sensor_power {
            return Err(BusError);
        }
        self.power_get_count += 1;
        Ok(())
    }

    fn sensor_power_put(&mut self) {
        self.power_put_count += 1;
    }
}
