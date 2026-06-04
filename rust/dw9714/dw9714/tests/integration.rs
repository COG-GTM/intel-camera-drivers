//! End-to-end tests for the safe dw9714 driver, exercising every public
//! function of the device API plus the power state machine, using the
//! in-memory mock I2C backend (no hardware / kernel required).

use dw9714::mock::MockI2cBus;
use dw9714::{DW9714Error, DeviceConfig, Dw9714, LensPosition};

const ADDR: u16 = 0x0c;

fn probe_default() -> Dw9714<MockI2cBus> {
    Dw9714::probe(MockI2cBus::new(), DeviceConfig::new(ADDR)).unwrap()
}

fn full_config() -> DeviceConfig {
    DeviceConfig {
        i2c_addr: ADDR,
        gpio_xsd: Some(5),
        has_sensor_dev: true,
    }
}

// --- focus / set_position -------------------------------------------------

#[test]
fn set_position_writes_correct_register_word() {
    let mut dev = probe_default();
    dev.set_position(LensPosition::new(512).unwrap()).unwrap();

    assert_eq!(dev.current_val(), 512);
    assert_eq!(dev.position().get(), 512);
    // VCM_VAL(512, 0) == 512 << 4, big-endian, one write, to the right addr.
    assert_eq!(dev.bus().writes.len(), 1);
    assert_eq!(dev.bus().writes[0].0, ADDR);
    assert_eq!(dev.bus().last_word(), Some(512 << 4));
}

#[test]
fn set_position_raw_validates_then_writes() {
    let mut dev = probe_default();
    dev.set_position_raw(777).unwrap();
    assert_eq!(dev.current_val(), 777);
    assert_eq!(dev.bus().last_word(), Some(777 << 4));
}

#[test]
fn set_position_raw_rejects_out_of_range() {
    let mut dev = probe_default();
    assert_eq!(
        dev.set_position_raw(2000),
        Err(DW9714Error::PositionOutOfRange {
            requested: 2000,
            max: 1023
        })
    );
    // Nothing written on rejection.
    assert!(dev.bus().writes.is_empty());
    assert_eq!(dev.current_val(), 0);
}

#[test]
fn set_position_at_boundaries() {
    let mut dev = probe_default();
    dev.set_position(LensPosition::ZERO).unwrap();
    assert_eq!(dev.bus().last_word(), Some(0));
    dev.set_position(LensPosition::new(1023).unwrap()).unwrap();
    assert_eq!(dev.bus().last_word(), Some(1023 << 4));
}

// --- set_ctrl (V4L2 control bridge) --------------------------------------

#[test]
fn set_ctrl_focus_absolute_sets_position() {
    let mut dev = probe_default();
    // V4L2_CID_FOCUS_ABSOLUTE
    dev.set_ctrl(0x009a_090a, 333).unwrap();
    assert_eq!(dev.current_val(), 333);
}

#[test]
fn set_ctrl_rejects_unknown_id() {
    let mut dev = probe_default();
    assert_eq!(
        dev.set_ctrl(0xdead_beef, 100),
        Err(DW9714Error::InvalidArgument)
    );
    assert!(dev.bus().writes.is_empty());
}

#[test]
fn set_ctrl_rejects_negative_value() {
    let mut dev = probe_default();
    assert_eq!(
        dev.set_ctrl(0x009a_090a, -1),
        Err(DW9714Error::InvalidArgument)
    );
}

#[test]
fn set_ctrl_rejects_too_large_value() {
    let mut dev = probe_default();
    assert_eq!(
        dev.set_ctrl(0x009a_090a, 5000),
        Err(DW9714Error::PositionOutOfRange {
            requested: 5000,
            max: 1023
        })
    );
}

#[test]
fn set_ctrl_rejects_values_above_u16_without_truncating() {
    let mut dev = probe_default();
    // 65536 % 65536 == 0 and 66000 % 65536 == 464 would both silently slip
    // through a bare `as u16` cast; they must be rejected instead.
    for v in [65536, 66000, i32::MAX] {
        assert!(
            matches!(
                dev.set_ctrl(0x009a_090a, v),
                Err(DW9714Error::PositionOutOfRange { .. })
            ),
            "value {v} must be rejected, not truncated"
        );
    }
    // Nothing was programmed.
    assert!(dev.bus().writes.is_empty());
    assert_eq!(dev.current_val(), 0);
}

// --- I2C error + retry path ----------------------------------------------

#[test]
fn persistent_i2c_error_returns_io_after_retry() {
    let mut dev = Dw9714::probe(MockI2cBus::failing(), DeviceConfig::new(ADDR)).unwrap();
    assert_eq!(
        dev.set_position(LensPosition::new(100).unwrap()),
        Err(DW9714Error::Io)
    );
    // Original write + exactly one retry == 2 attempts.
    assert_eq!(dev.bus().writes.len(), 2);
    // current_val is still updated (faithful to C ordering).
    assert_eq!(dev.current_val(), 100);
}

#[test]
fn transient_i2c_error_recovers_on_retry() {
    let mut dev = Dw9714::probe(MockI2cBus::failing_first(1), DeviceConfig::new(ADDR)).unwrap();
    dev.set_position(LensPosition::new(100).unwrap()).unwrap();
    // First attempt failed, retry succeeded.
    assert_eq!(dev.bus().writes.len(), 2);
}

// --- init / double-init / not-initialized --------------------------------

#[test]
fn double_init_is_rejected() {
    let mut dev = Dw9714::new(MockI2cBus::new(), DeviceConfig::new(ADDR));
    assert!(dev.init().is_ok());
    assert_eq!(dev.init(), Err(DW9714Error::AlreadyInitialized));
}

#[test]
fn operations_before_init_are_rejected() {
    let mut dev = Dw9714::new(MockI2cBus::new(), DeviceConfig::new(ADDR));
    assert!(!dev.is_initialized());
    assert_eq!(
        dev.set_position(LensPosition::new(10).unwrap()),
        Err(DW9714Error::NotInitialized)
    );
    assert_eq!(dev.runtime_resume(), Err(DW9714Error::NotInitialized));
    assert_eq!(dev.runtime_suspend(), Err(DW9714Error::NotInitialized));
}

#[test]
fn probe_yields_initialized_zeroed_device() {
    let dev = probe_default();
    assert!(dev.is_initialized());
    assert_eq!(dev.current_val(), 0);
    assert!(!dev.is_powered());
}

// --- power state machine: suspend ramp-down ------------------------------

fn expected_suspend_words(current_val: i32) -> Vec<u16> {
    // Mirror dw9714_runtime_suspend.
    let steps = 16i32;
    let mut out = Vec::new();
    let mut val = current_val & !(steps - 1);
    while val >= 0 {
        out.push((val as u16) << 4);
        val -= steps;
    }
    out
}

#[test]
fn runtime_suspend_ramps_lens_down_to_zero() {
    let mut dev = Dw9714::probe(MockI2cBus::new(), full_config()).unwrap();
    dev.set_position(LensPosition::new(512).unwrap()).unwrap();
    dev.bus_clear();
    dev.runtime_suspend().unwrap();

    assert_eq!(dev.bus().words(), expected_suspend_words(512));
    // Ends with GPIO low and one sensor power_put.
    assert!(!*dev.bus().gpio_states.last().unwrap());
    assert_eq!(dev.bus().power_put_count, 1);
    // One delay per ramp write.
    assert_eq!(dev.bus().delays_us.len(), expected_suspend_words(512).len());
    assert!(!dev.is_powered());
}

#[test]
fn runtime_suspend_from_zero_writes_single_zero() {
    let mut dev = Dw9714::probe(MockI2cBus::new(), DeviceConfig::new(ADDR)).unwrap();
    dev.runtime_suspend().unwrap();
    assert_eq!(dev.bus().words(), vec![0]);
}

#[test]
fn runtime_suspend_ignores_i2c_errors() {
    // C logs but ignores per-write failures during suspend.
    let mut dev = Dw9714::probe(MockI2cBus::failing(), full_config()).unwrap();
    dev.runtime_suspend().unwrap();
    assert!(!dev.is_powered());
}

// --- power state machine: resume ramp-up ---------------------------------

fn expected_resume_words(current_val: i32) -> Vec<u16> {
    // Mirror dw9714_runtime_resume (ramp + final ctrl restore).
    let steps = 16i32;
    let mut out = Vec::new();
    let mut val = current_val % steps;
    while val < current_val + steps - 1 {
        out.push((val as u16) << 4);
        val += steps;
    }
    out.push((current_val as u16) << 4); // v4l2_ctrl_handler_setup re-apply
    out
}

#[test]
fn runtime_resume_ramps_lens_up_and_restores() {
    let mut dev = Dw9714::probe(MockI2cBus::new(), full_config()).unwrap();
    dev.set_position(LensPosition::new(512).unwrap()).unwrap();
    dev.bus_clear();
    dev.runtime_resume().unwrap();

    assert_eq!(dev.bus().words(), expected_resume_words(512));
    // Power up first: one sensor power_get and GPIO high.
    assert_eq!(dev.bus().power_get_count, 1);
    assert!(*dev.bus().gpio_states.last().unwrap());
    assert!(dev.is_powered());
}

#[test]
fn runtime_resume_fails_when_sensor_power_fails() {
    let bus = MockI2cBus::new().with_failing_sensor_power();
    let mut dev = Dw9714::probe(bus, full_config()).unwrap();
    assert_eq!(dev.runtime_resume(), Err(DW9714Error::Io));
    assert!(!dev.is_powered());
    // Faithful to C `out:` cleanup: pm_runtime_get_sync increments the usage
    // count even on failure, so the matching put must still run (no PM leak).
    assert_eq!(dev.bus().power_put_count, 1);
}

#[test]
fn runtime_resume_without_platform_data_skips_power_calls() {
    let mut dev = Dw9714::probe(MockI2cBus::new(), DeviceConfig::new(ADDR)).unwrap();
    dev.runtime_resume().unwrap();
    assert_eq!(dev.bus().power_get_count, 0);
    assert!(dev.bus().gpio_states.is_empty());
    assert!(dev.is_powered());
}

// --- RAII open / close ----------------------------------------------------

#[test]
fn open_resumes_and_drop_suspends() {
    let mut dev = Dw9714::probe(MockI2cBus::new(), full_config()).unwrap();
    dev.set_position(LensPosition::new(256).unwrap()).unwrap();
    dev.bus_clear();

    {
        let mut guard = dev.open().unwrap();
        assert!(guard.is_powered());
        // Can drive the lens through the RAII guard.
        guard.set_position(LensPosition::new(300).unwrap()).unwrap();
        assert_eq!(guard.current_val(), 300);
    } // guard dropped here -> runtime_suspend

    assert!(!dev.is_powered());
    // After close, GPIO ended low and power was released.
    assert!(!*dev.bus().gpio_states.last().unwrap());
    assert_eq!(dev.bus().power_put_count, 1);
}

// --- accessors ------------------------------------------------------------

#[test]
fn config_accessor_reports_platform_data() {
    let dev = Dw9714::probe(MockI2cBus::new(), full_config()).unwrap();
    assert_eq!(dev.config().i2c_addr, ADDR);
    assert_eq!(dev.config().gpio_xsd, Some(5));
    assert!(dev.config().has_sensor_dev);
}

// Small helper extension so tests can reset recorded bus activity between the
// setup write and the action under test, without exposing internals publicly.
trait BusClear {
    fn bus_clear(&mut self);
}
impl BusClear for Dw9714<MockI2cBus> {
    fn bus_clear(&mut self) {
        // SAFETY-free: goes through the public mutable-bus hook below.
        self.bus_mut().clear();
    }
}
