#![allow(clippy::float_cmp)]
mod model;

use super::*;
use matryona_machines::testing::{describes, refuses};

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
        for error in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            refuses(&mut pid, Refusal::NotFinite, |pid| pid.step(error, second));
            // A bad sample is reported as such whatever the interval.
            refuses(&mut pid, Refusal::NotFinite, |pid| {
                pid.step(error, Duration::ZERO)
            });
        }
        refuses(&mut pid, Refusal::InvalidDuration, |pid| {
            pid.step(1.0, Duration::ZERO)
        });
        refuses(&mut pid, Refusal::Overflow, |pid| {
            pid.step(f64::MAX, Duration::MAX)
        });
    }
    // Derivative overflow: a finite swing over the shortest interval.
    let mut pid = self::pid(0.0, 0.0, 1.0, -1.0, 1.0);
    assert_eq!(pid.step(-1.0e300, second), Ok(0.0));
    refuses(&mut pid, Refusal::Overflow, |pid| {
        pid.step(1.0e300, Duration::from_nanos(1))
    });
}

#[test]
fn hysteresis_retains_state_within_the_band() {
    let thresholds = Thresholds::new(-2.0, 2.0).expect("thresholds");
    let mut band = Hysteresis::new(thresholds);
    assert_eq!((band.state(), band.thresholds()), (Band::Low, thresholds));
    assert_eq!((thresholds.low(), thresholds.high()), (-2.0, 2.0));
    assert_eq!(band.observe(2.0), Ok(Band::High));
    assert_eq!(band.observe(0.0), Ok(Band::High));
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        refuses(&mut band, Refusal::NotFinite, |band| band.observe(value));
    }
    assert_eq!(band.state(), Band::High);
    assert_eq!(band.observe(-2.0), Ok(Band::Low));
    assert_eq!(band.state(), Band::Low);
}

#[test]
fn thresholds_are_validated_and_may_be_equal() {
    for (low, high) in [
        (2.0, 1.0),
        (f64::NEG_INFINITY, 1.0),
        (0.0, f64::INFINITY),
        (f64::NAN, f64::NAN),
    ] {
        assert_eq!(Thresholds::new(low, high), None, "{low} {high}");
    }
    assert_eq!(Thresholds::new(2_u64, 1), None);
    assert!(Thresholds::new(1.0, 1.0).is_some());
    assert!(Thresholds::new(u128::MAX, u128::MAX).is_some());
}

#[test]
fn a_zero_width_band_switches_on_the_threshold_itself() {
    let mut band = Hysteresis::new(Thresholds::new(5.0, 5.0).expect("thresholds"));
    for (value, side) in [
        (4.999, Band::Low),
        (5.0, Band::High),
        (5.001, Band::High),
        (5.0, Band::High),
        (4.999, Band::Low),
        (5.0, Band::High),
    ] {
        assert_eq!(band.observe(value), Ok(side), "{value}");
    }
    let mut band = Hysteresis::restored(Thresholds::new(7_u8, 7).expect("thresholds"), Band::High);
    for (value, side) in [
        (7, Band::High),
        (6, Band::Low),
        (7, Band::High),
        (0, Band::Low),
    ] {
        assert_eq!(band.observe(value), Ok(side), "{value}");
    }
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn integer_bands_compare_exactly_past_f64_precision() {
    let high = (1_u128 << 53) + 1;
    assert_eq!(high as f64, (high - 1) as f64, "f64 cannot tell them apart");
    let mut band = Hysteresis::new(Thresholds::new(high, high).expect("thresholds"));
    let Ok(below) = band.observe(high - 1);
    assert_eq!(below, Band::Low);
    let Ok(at) = band.observe(high);
    assert_eq!(at, Band::High);
    let Ok(below) = band.observe(high - 1);
    assert_eq!(below, Band::Low);
    let reserve = u128::MAX - 1;
    let mut band = Hysteresis::new(Thresholds::new(reserve, u128::MAX).expect("thresholds"));
    assert_eq!(band.observe(u128::MAX), Ok(Band::High));
    assert_eq!(band.observe(reserve), Ok(Band::Low));
    let mut band = Hysteresis::new(Thresholds::new(0_u64, u64::MAX).expect("thresholds"));
    assert_eq!(band.observe(u64::MAX - 1), Ok(Band::Low));
    assert_eq!(band.observe(u64::MAX), Ok(Band::High));
    assert_eq!(band.observe(1), Ok(Band::High));
    assert_eq!(band.observe(0), Ok(Band::Low));
}

#[test]
fn refusals_describe_themselves() {
    describes(&[
        (Refusal::NotFinite, "the sample is NaN or infinite"),
        (Refusal::InvalidDuration, "the elapsed time is zero"),
        (
            Refusal::Overflow,
            "a term of the calculation is not finite or representable",
        ),
    ]);
}

#[test]
fn a_negative_zero_gain_is_stored_as_zero() {
    let gains = Gains::new(-0.0, -0.0, -0.0).expect("zero gains");
    for gain in [gains.p(), gains.i(), gains.d()] {
        assert!(gain.is_sign_positive(), "{gain}");
    }
}

#[test]
fn a_saved_pid_resumes_where_it_stopped() {
    let mut live = pid(1.0, 0.5, 0.25, -10.0, 10.0);
    assert_eq!((live.integral(), live.previous_error()), (0.0, None));
    let second = Duration::from_secs(1);
    for error in [3.0, -1.5, 2.0] {
        live.step(error, second).expect("step");
    }
    assert_eq!((live.integral(), live.previous_error()), (3.5, Some(2.0)));
    let mut resumed = Pid::restored(
        live.gains(),
        live.limits(),
        live.integral(),
        live.previous_error(),
    )
    .expect("restored");
    assert_eq!(resumed, live);
    assert_eq!(resumed.step(1.0, second), live.step(1.0, second));
    let fresh = pid(1.0, 0.5, 0.25, -10.0, 10.0);
    let restored = Pid::restored(fresh.gains(), fresh.limits(), -0.0, None).expect("fresh");
    assert_eq!(restored, fresh);
    assert!(restored.integral().is_sign_positive());
}

#[test]
fn a_saved_pid_must_be_finite() {
    let (gains, limits) = (
        Gains::new(1.0, 1.0, 1.0).expect("gains"),
        Limits::new(-1.0, 1.0).expect("limits"),
    );
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(Pid::restored(gains, limits, bad, None), None);
        assert_eq!(Pid::restored(gains, limits, 0.0, Some(bad)), None);
    }
}

#[test]
fn a_saved_band_resumes_on_its_side() {
    let thresholds = Thresholds::new(-2.0, 2.0).expect("thresholds");
    let mut live = Hysteresis::new(thresholds);
    assert_eq!(Hysteresis::restored(thresholds, live.state()), live);
    assert_eq!(live.observe(3.0), Ok(Band::High));
    let mut resumed = Hysteresis::restored(live.thresholds(), live.state());
    assert_eq!(resumed, live);
    assert_eq!(resumed.observe(0.0), live.observe(0.0));
    assert_eq!(resumed.state(), Band::High);
}

#[test]
fn a_dwell_counts_continuous_holding() {
    let (gap, secs) = (Duration::from_secs(60), Duration::from_secs);
    let mut dwell = Dwell::new(gap);
    assert_eq!(dwell.observe(true, secs(5), secs(60)), Ok(Hold::Holding));
    assert_eq!(dwell.held(), Some(Duration::ZERO));
    assert_eq!(dwell.observe(true, secs(60), secs(60)), Ok(Hold::Satisfied));
    // A longer requirement holds again: no latch.
    assert_eq!(dwell.observe(true, secs(1), secs(120)), Ok(Hold::Holding));
    // A silence past the gap restarts the count; so does an interruption.
    assert_eq!(dwell.observe(true, secs(61), secs(0)), Ok(Hold::Satisfied));
    assert_eq!(dwell.held(), Some(Duration::ZERO));
    assert_eq!(
        dwell.observe(false, secs(1), secs(0)),
        Ok(Hold::Interrupted)
    );
    assert_eq!(dwell.held(), None);
}

#[test]
fn a_dwell_refuses_overflow_and_resumes_from_saved_state() {
    let mut dwell = Dwell::restored(Duration::MAX, Some(Duration::MAX)).expect("reachable");
    let before = dwell.clone();
    assert_eq!(
        dwell.observe(true, Duration::from_nanos(1), Duration::ZERO),
        Err(Refusal::Overflow)
    );
    assert_eq!(dwell, before);
    // A zero gap never holds past zero.
    assert_eq!(
        Dwell::restored(Duration::ZERO, Some(Duration::from_nanos(1))),
        None
    );
}
