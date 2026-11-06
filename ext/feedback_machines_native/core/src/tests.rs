#![allow(clippy::expect_used, clippy::float_cmp)]
use super::*;
use core::time::Duration;

#[test]
fn proportional_derivative_and_sampling_are_explicit() {
    let mut pid = Pid::new(
        Gains::new(2.0, 0.0, 1.0).expect("gains"),
        Limits::new(-100.0, 100.0).expect("limits"),
    );
    assert_eq!(pid.step(3.0, Duration::from_secs(1)), Some(6.0));
    assert_eq!(pid.step(5.0, Duration::from_secs(2)), Some(11.0));
}

#[test]
fn saturation_does_not_wind_up_the_integrator() {
    let mut pid = Pid::new(
        Gains::new(0.0, 1.0, 0.0).expect("gains"),
        Limits::new(-5.0, 5.0).expect("limits"),
    );
    for _ in 0..100 {
        assert_eq!(pid.step(10.0, Duration::from_secs(1)), Some(5.0));
    }
    assert_eq!(pid.step(1.0, Duration::from_secs(1)), Some(1.0));
    // Conditional integration rejects a saturating increment instead of storing it.
    assert_eq!(pid.integral, 1.0);
}

#[test]
fn invalid_inputs_do_not_corrupt_history() {
    assert!(Gains::new(f64::NAN, 0.0, 0.0).is_none());
    assert!(Gains::new(-1.0, 0.0, 0.0).is_none());
    assert!(Limits::new(1.0, 1.0).is_none());
    let mut pid = Pid::new(
        Gains::new(1.0, 1.0, 1.0).expect("gains"),
        Limits::new(-1.0, 1.0).expect("limits"),
    );
    let before = pid.clone();
    assert_eq!(pid.step(1.0, Duration::ZERO), None);
    assert_eq!(pid.step(f64::INFINITY, Duration::from_secs(1)), None);
    assert_eq!(pid.step(f64::MAX, Duration::MAX), None);
    assert_eq!(pid, before);
}

#[test]
fn hysteresis_retains_state_within_the_band() {
    let mut band = Hysteresis::new(Limits::new(-2.0, 2.0).expect("limits"));
    assert_eq!(band.observe(2.0), Some(Band::High));
    assert_eq!(band.observe(0.0), Some(Band::High));
    assert_eq!(band.observe(f64::NAN), None);
    assert_eq!(band.observe(-2.0), Some(Band::Low));
}
