//! Randomized comparison against a spelled-out reference controller.
use crate::{Band, Gains, Hysteresis, Limits, Pid, Refusal};
use core::time::Duration;
use matryona_machines::testing::SplitMix64;

/// Controller inputs drawn from a `SplitMix64` stream.
struct Draws(SplitMix64);

impl Draws {
    const fn new(seed: u64) -> Self {
        Self(SplitMix64::new(seed))
    }

    const fn below(&mut self, bound: u64) -> u64 {
        self.0.below(bound)
    }

    /// A value in `-scale..=scale`, in steps of a thousandth of `scale`.
    fn signed(&mut self, scale: f64) -> f64 {
        (f64::from(u32::try_from(self.below(2001)).expect("small")) - 1000.0) / 1000.0 * scale
    }

    /// A sample: mostly ordinary, sometimes huge, sometimes not a number.
    fn sample(&mut self) -> f64 {
        match self.below(20) {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => f64::NEG_INFINITY,
            3 => self.signed(f64::MAX),
            _ => self.signed(50.0),
        }
    }

    fn elapsed(&mut self) -> Duration {
        match self.below(12) {
            0 => Duration::ZERO,
            1 => Duration::from_nanos(1),
            2 => Duration::MAX,
            _ => Duration::from_millis(1 + self.below(5_000)),
        }
    }
}

/// The controller written out term by term, with its own state.
struct Reference {
    gains: (f64, f64, f64),
    limits: (f64, f64),
    integral: f64,
    previous: Option<f64>,
}

impl Reference {
    fn step(&mut self, error: f64, elapsed: Duration) -> Result<f64, Refusal> {
        if error.is_nan() || error.is_infinite() {
            return Err(Refusal::NotFinite);
        }
        if elapsed == Duration::ZERO {
            return Err(Refusal::InvalidDuration);
        }
        let (p, i, d) = self.gains;
        let (min, max) = self.limits;
        let dt = elapsed.as_secs_f64();
        let slope = self
            .previous
            .map_or(0.0, |previous| (error - previous) / dt);
        let integral = self.integral + error * dt;
        let unclamped = p * error + d * slope + i * integral;
        let terms = [slope, integral, p * error + d * slope, unclamped];
        if terms.iter().any(|term| !term.is_finite()) {
            return Err(Refusal::Overflow);
        }
        let pinned_high = unclamped > max && error > 0.0;
        let pinned_low = unclamped < min && error < 0.0;
        if !pinned_high && !pinned_low {
            self.integral = integral;
        }
        self.previous = Some(error);
        Ok(unclamped.max(min).min(max))
    }
}

#[test]
fn random_samples_match_the_reference_controller() {
    let mut draws = Draws::new(4208);
    let mut refusals = [0_u32; 3];
    for _ in 0..300 {
        let gains = (
            draws.signed(4.0).abs(),
            draws.signed(2.0).abs(),
            draws.signed(2.0).abs(),
        );
        let min = draws.signed(100.0);
        let limits = (min, min + 1.0 + draws.signed(100.0).abs());
        let mut pid = Pid::new(
            Gains::new(gains.0, gains.1, gains.2).expect("gains"),
            Limits::new(limits.0, limits.1).expect("limits"),
        );
        let mut reference = Reference {
            gains,
            limits,
            integral: 0.0,
            previous: None,
        };
        for _ in 0..200 {
            let (error, elapsed) = (draws.sample(), draws.elapsed());
            let before = pid.clone();
            let answer = pid.step(error, elapsed);
            assert_eq!(answer, reference.step(error, elapsed));
            match answer {
                Ok(output) => assert!((limits.0..=limits.1).contains(&output)),
                Err(refusal) => {
                    assert_eq!(pid, before);
                    refusals[refusal as usize] += 1;
                }
            }
            assert_eq!(pid.integral, reference.integral);
            assert_eq!(pid.previous, reference.previous);
        }
    }
    assert!(refusals.iter().all(|count| *count > 100), "{refusals:?}");
}

#[test]
fn random_measurements_match_the_reference_band() {
    let mut draws = Draws::new(1865);
    for _ in 0..300 {
        let min = draws.signed(10.0);
        let max = min + 0.5 + draws.signed(10.0).abs();
        let mut band = Hysteresis::new(Limits::new(min, max).expect("limits"));
        let mut high = false;
        for _ in 0..200 {
            let value = draws.sample();
            let before = band.clone();
            let answer = band.observe(value);
            if value.is_nan() || value.is_infinite() {
                assert_eq!(answer, Err(Refusal::NotFinite));
                assert_eq!(band, before);
            } else {
                high = if high { value > min } else { value >= max };
                assert_eq!(answer, Ok(if high { Band::High } else { Band::Low }));
            }
            assert_eq!(band.state() == Band::High, high);
        }
    }
}
