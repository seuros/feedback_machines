#![allow(clippy::expect_used, clippy::float_cmp)]
mod model;

use super::*;

fn pid(p: f64, i: f64, d: f64, min: f64, max: f64) -> Pid {
    Pid::new(
        Gains::new(p, i, d).expect("gains"),
        Limits::new(min, max).expect("limits"),
    )
}

#[test]
fn proportional_derivative_and_sampling_are_explicit() {
    let mut pid = pid(2.0, 0.0, 1.0, -100.0, 100.0);
    assert_eq!(pid.step(3.0, Duration::from_secs(1)), Ok(6.0));
    assert_eq!(pid.step(5.0, Duration::from_secs(2)), Ok(11.0));
    assert_eq!(
        (pid.gains().p(), pid.gains().i(), pid.gains().d()),
        (2.0, 0.0, 1.0)
    );
    assert_eq!((pid.limits().min(), pid.limits().max()), (-100.0, 100.0));
    // The shortest interval is an interval: one nanosecond integrates.
    let mut pid = self::pid(1.0, 1.0, 0.0, -10.0, 10.0);
    assert_eq!(pid.step(2.0, Duration::from_nanos(1)), Ok(2.0 + 2.0e-9));
}

#[test]
fn saturation_does_not_wind_up_the_integrator() {
    let mut pid = pid(0.0, 1.0, 0.0, -5.0, 5.0);
    for _ in 0..100 {
        assert_eq!(pid.step(10.0, Duration::from_secs(1)), Ok(5.0));
    }
    assert_eq!(pid.step(1.0, Duration::from_secs(1)), Ok(1.0));
    // Conditional integration rejects a saturating increment instead of storing it.
    assert_eq!(pid.integral, 1.0);
}

#[test]
fn configuration_is_validated_by_the_constructors() {
    for (p, i, d) in [
        (f64::NAN, 0.0, 0.0),
        (-1.0, 0.0, 0.0),
        (0.0, f64::INFINITY, 0.0),
    ] {
        assert_eq!(Gains::new(p, i, d), None, "{p} {i} {d}");
    }
    assert!(Gains::new(0.0, 0.0, 0.0).is_some());
    for (min, max) in [
        (1.0, 1.0),
        (2.0, 1.0),
        (f64::NEG_INFINITY, 1.0),
        (0.0, f64::NAN),
    ] {
        assert_eq!(Limits::new(min, max), None, "{min} {max}");
    }
}

#[test]
fn every_pid_refusal_names_its_reason_and_changes_nothing() {
    let mut pid = pid(1.0, 1.0, 1.0, -1.0, 1.0);
    let second = Duration::from_secs(1);
    for warmed in [false, true] {
        if warmed {
            assert_eq!(pid.step(0.5, second), Ok(1.0));
        }
        let before = pid.clone();
        for error in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(pid.step(error, second), Err(Refusal::NotFinite));
            // A bad sample is reported as such whatever the interval.
            assert_eq!(pid.step(error, Duration::ZERO), Err(Refusal::NotFinite));
            assert_eq!(pid, before);
        }
        assert_eq!(pid.step(1.0, Duration::ZERO), Err(Refusal::InvalidDuration));
        assert_eq!(pid, before);
        assert_eq!(pid.step(f64::MAX, Duration::MAX), Err(Refusal::Overflow));
        assert_eq!(pid, before);
    }
    // Derivative overflow: a finite swing over the shortest interval.
    let mut pid = self::pid(0.0, 0.0, 1.0, -1.0, 1.0);
    assert_eq!(pid.step(-1.0e300, second), Ok(0.0));
    let before = pid.clone();
    assert_eq!(
        pid.step(1.0e300, Duration::from_nanos(1)),
        Err(Refusal::Overflow)
    );
    assert_eq!(pid, before);
}

#[test]
fn hysteresis_retains_state_within_the_band() {
    let limits = Limits::new(-2.0, 2.0).expect("limits");
    let mut band = Hysteresis::new(limits);
    assert_eq!((band.state(), band.limits()), (Band::Low, limits));
    assert_eq!(band.observe(2.0), Ok(Band::High));
    assert_eq!(band.observe(0.0), Ok(Band::High));
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let before = band.clone();
        assert_eq!(band.observe(value), Err(Refusal::NotFinite));
        assert_eq!(band, before);
    }
    assert_eq!(band.state(), Band::High);
    assert_eq!(band.observe(-2.0), Ok(Band::Low));
    assert_eq!(band.state(), Band::Low);
}

#[test]
fn refusals_describe_themselves() {
    extern crate std;
    use std::string::ToString;
    for refusal in [
        Refusal::NotFinite,
        Refusal::InvalidDuration,
        Refusal::Overflow,
    ] {
        let error: &dyn core::error::Error = &refusal;
        assert_ne!(error.to_string(), "");
    }
}
