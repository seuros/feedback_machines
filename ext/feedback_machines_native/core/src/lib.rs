#![no_std]
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]
#![doc = include_str!("../README.md")]

use core::{fmt, time::Duration};

/// Why a call changed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The sample (a PID error or a hysteresis measurement) is NaN or infinite.
    NotFinite,
    /// The elapsed time is zero: no interval to integrate or differentiate over.
    InvalidDuration,
    /// The sample is finite but a term of the calculation is not, or a
    /// dwell's held span would pass `Duration::MAX`.
    Overflow,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFinite => "the sample is NaN or infinite",
            Self::InvalidDuration => "the elapsed time is zero",
            Self::Overflow => "a term of the calculation is not finite or representable",
        })
    }
}

impl core::error::Error for Refusal {}

/// Finite, nonnegative proportional/integral/derivative gains.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gains {
    p: f64,
    i: f64,
    d: f64,
}

impl Gains {
    /// Validate gain coefficients: `None` unless all are finite and
    /// nonnegative. A negative zero is stored as zero.
    #[must_use]
    pub const fn new(p: f64, i: f64, d: f64) -> Option<Self> {
        match (gain(p), gain(i), gain(d)) {
            (Some(p), Some(i), Some(d)) => Some(Self { p, i, d }),
            _ => None,
        }
    }

    /// Proportional gain.
    #[must_use]
    pub const fn p(&self) -> f64 {
        self.p
    }

    /// Integral gain.
    #[must_use]
    pub const fn i(&self) -> f64 {
        self.i
    }

    /// Derivative gain.
    #[must_use]
    pub const fn d(&self) -> f64 {
        self.d
    }
}

/// `value` as a gain: `None` unless finite and nonnegative, a negative zero
/// as zero.
const fn gain(value: f64) -> Option<f64> {
    if value.is_finite() && value >= 0.0 {
        Some(if value == 0.0 { 0.0 } else { value })
    } else {
        None
    }
}

/// Finite ordered output limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    min: f64,
    max: f64,
}

impl Limits {
    /// Validate a nonempty finite range: `None` unless `min < max`, both finite.
    #[must_use]
    pub const fn new(min: f64, max: f64) -> Option<Self> {
        if min.is_finite() && max.is_finite() && min < max {
            Some(Self { min, max })
        } else {
            None
        }
    }

    /// Lower limit.
    #[must_use]
    pub const fn min(&self) -> f64 {
        self.min
    }

    /// Upper limit.
    #[must_use]
    pub const fn max(&self) -> f64 {
        self.max
    }
}

/// Explicit-sampling PID with conditional-integration anti-windup.
#[derive(Debug, Clone, PartialEq)]
pub struct Pid {
    gains: Gains,
    limits: Limits,
    integral: f64,
    previous: Option<f64>,
}

impl Pid {
    /// Start with no accumulated error and no derivative history.
    #[must_use]
    pub const fn new(gains: Gains, limits: Limits) -> Self {
        Self {
            gains,
            limits,
            integral: 0.0,
            previous: None,
        }
    }

    /// A controller resuming saved state: the [`Pid::integral`] and
    /// [`Pid::previous_error`] read from one before it was put away.
    ///
    /// `None` unless `integral` and `previous_error` (when present) are
    /// finite. A negative zero integral is stored as zero, as `step` would
    /// have left it.
    #[must_use]
    pub const fn restored(
        gains: Gains,
        limits: Limits,
        integral: f64,
        previous_error: Option<f64>,
    ) -> Option<Self> {
        if !integral.is_finite() {
            return None;
        }
        if let Some(previous) = previous_error
            && !previous.is_finite()
        {
            return None;
        }
        Some(Self {
            gains,
            limits,
            integral: if integral == 0.0 { 0.0 } else { integral },
            previous: previous_error,
        })
    }

    /// The accumulated error-seconds the integral term multiplies.
    #[must_use]
    pub const fn integral(&self) -> f64 {
        self.integral
    }

    /// The error of the previous honoured sample, which the derivative term
    /// differences against: `None` before the first.
    #[must_use]
    pub const fn previous_error(&self) -> Option<f64> {
        self.previous
    }

    /// The gains this controller was built with.
    #[must_use]
    pub const fn gains(&self) -> Gains {
        self.gains
    }

    /// The output limits this controller was built with.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    /// Observe one sample taken `elapsed` after the previous one and answer
    /// the clamped output.
    ///
    /// # Errors
    ///
    /// [`Refusal::NotFinite`] for a NaN or infinite `error`,
    /// [`Refusal::InvalidDuration`] for a zero `elapsed`, [`Refusal::Overflow`]
    /// when a term of the calculation is not finite. Nothing changed: neither
    /// the integral nor the derivative history.
    pub fn step(&mut self, error: f64, elapsed: Duration) -> Result<f64, Refusal> {
        if !error.is_finite() {
            return Err(Refusal::NotFinite);
        }
        if elapsed.is_zero() {
            return Err(Refusal::InvalidDuration);
        }
        let dt = elapsed.as_secs_f64();
        let derivative = self
            .previous
            .map_or(0.0, |previous| (error - previous) / dt);
        let candidate = self.integral + error * dt;
        let base = self.gains.p * error + self.gains.d * derivative;
        let proposed = base + self.gains.i * candidate;
        if ![derivative, candidate, base, proposed]
            .iter()
            .all(|x| x.is_finite())
        {
            return Err(Refusal::Overflow);
        }
        let winding_up = (proposed > self.limits.max && error > 0.0)
            || (proposed < self.limits.min && error < 0.0);
        let integral = if winding_up { self.integral } else { candidate };
        self.integral = integral;
        self.previous = Some(error);
        Ok(proposed.clamp(self.limits.min, self.limits.max))
    }
}

mod sealed {
    pub trait Sealed {}
}

/// A measurement type a [`Hysteresis`] band compares: `f64`, or an unsigned
/// integer compared exactly. Sealed.
pub trait Level: Copy + PartialOrd + sealed::Sealed {
    /// Why a measurement can be refused: [`Refusal`] for `f64`,
    /// [`core::convert::Infallible`] for the integers, which are always
    /// measurements.
    type Refused;

    /// `self` as a measurement, or why it is not one.
    ///
    /// # Errors
    ///
    /// [`Refusal::NotFinite`] for a NaN or infinite `f64`.
    fn measured(self) -> Result<Self, Self::Refused>;
}

impl sealed::Sealed for f64 {}

impl Level for f64 {
    type Refused = Refusal;

    fn measured(self) -> Result<Self, Refusal> {
        if self.is_finite() {
            Ok(self)
        } else {
            Err(Refusal::NotFinite)
        }
    }
}

macro_rules! exact_levels {
    ($($integer:ty),+) => {$(
        impl sealed::Sealed for $integer {}

        impl Level for $integer {
            type Refused = core::convert::Infallible;

            fn measured(self) -> Result<Self, Self::Refused> {
                Ok(self)
            }
        }
    )+};
}

exact_levels!(u8, u16, u32, u64, u128, usize);

/// The two thresholds of a [`Hysteresis`] band, `low <= high`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds<T: Level = f64> {
    low: T,
    high: T,
}

impl<T: Level> Thresholds<T> {
    /// Validate a band: `None` unless `low <= high`, both measurements (finite,
    /// for `f64`). Equal thresholds are a band with no deadband, a plain
    /// threshold: at or above it is [`Band::High`], below it [`Band::Low`].
    #[must_use]
    pub fn new(low: T, high: T) -> Option<Self> {
        match (low.measured(), high.measured()) {
            (Ok(low), Ok(high)) if low <= high => Some(Self { low, high }),
            _ => None,
        }
    }

    /// At or below this, the band switches low.
    #[must_use]
    pub const fn low(&self) -> T {
        self.low
    }

    /// At or above this, the band switches high.
    #[must_use]
    pub const fn high(&self) -> T {
        self.high
    }
}

/// A generic two-state hysteresis band over a [`Level`] measurement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteresis<T: Level = f64> {
    thresholds: Thresholds<T>,
    state: Band,
}

/// Which side of the hysteresis band is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// Below the band.
    Low,
    /// Above the band.
    High,
}

impl<T: Level> Hysteresis<T> {
    /// Start on the low side.
    #[must_use]
    pub const fn new(thresholds: Thresholds<T>) -> Self {
        Self {
            thresholds,
            state: Band::Low,
        }
    }

    /// A band resuming saved state: on `state`, the [`Hysteresis::state`]
    /// read from one before it was put away. Every side is valid for every
    /// band.
    #[must_use]
    pub const fn restored(thresholds: Thresholds<T>, state: Band) -> Self {
        Self { thresholds, state }
    }

    /// The thresholds this band was built with.
    #[must_use]
    pub const fn thresholds(&self) -> Thresholds<T> {
        self.thresholds
    }

    /// The side the band is on now.
    #[must_use]
    pub const fn state(&self) -> Band {
        self.state
    }

    /// Observe a measurement and answer the side the band is on after it.
    ///
    /// # Errors
    ///
    /// [`Refusal::NotFinite`] for a NaN or infinite `f64` `value`; the band
    /// keeps its side. An integer band never refuses.
    pub fn observe(&mut self, value: T) -> Result<Band, T::Refused> {
        let value = value.measured()?;
        if value >= self.thresholds.high {
            self.state = Band::High;
        } else if value <= self.thresholds.low {
            self.state = Band::Low;
        }
        Ok(self.state)
    }
}

/// A condition that must hold continuously for a span before it counts.
///
/// Each observation says whether the condition holds now and how long since
/// the previous one. A sample where it does not hold, or a silence longer than
/// `max_gap`, restarts the count: nobody was watching, so continuity is not
/// assumed. The first holding sample after a restart has held for zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dwell {
    max_gap: Duration,
    held: Option<Duration>,
}

/// How a dwell stands after an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// The condition does not hold; the count is cleared.
    Interrupted,
    /// The condition holds, for less than the required span.
    Holding,
    /// The condition has held for at least the required span.
    Satisfied,
}

impl Dwell {
    /// Start with nothing held. `max_gap` is the longest silence between
    /// observations that still counts as continuous.
    #[must_use]
    pub const fn new(max_gap: Duration) -> Self {
        Self {
            max_gap,
            held: None,
        }
    }

    /// A dwell resuming saved state: `held`, the [`Dwell::held`] read from
    /// one before it was put away. `None` for the one unreachable state: a
    /// zero `max_gap` restarts on every advance, so it never holds past zero.
    #[must_use]
    pub const fn restored(max_gap: Duration, held: Option<Duration>) -> Option<Self> {
        match held {
            Some(span) if max_gap.is_zero() && !span.is_zero() => None,
            _ => Some(Self { max_gap, held }),
        }
    }

    /// The longest silence that still counts as continuous.
    #[must_use]
    pub const fn max_gap(&self) -> Duration {
        self.max_gap
    }

    /// How long the condition has held, or `None` when it is not holding.
    #[must_use]
    pub const fn held(&self) -> Option<Duration> {
        self.held
    }

    /// Observe the condition `elapsed` after the previous observation and
    /// answer whether it has held for `required`. `required` is per call, so
    /// a later, longer requirement can answer [`Hold::Holding`] again: a dwell
    /// does not latch. `elapsed` is ignored on the first observation and
    /// whenever the condition does not hold.
    ///
    /// # Errors
    ///
    /// [`Refusal::Overflow`] when the held span would pass `Duration::MAX`;
    /// the dwell keeps its count.
    pub fn observe(
        &mut self,
        holds: bool,
        elapsed: Duration,
        required: Duration,
    ) -> Result<Hold, Refusal> {
        if !holds {
            self.held = None;
            return Ok(Hold::Interrupted);
        }
        let held = match self.held {
            Some(held) if elapsed <= self.max_gap => {
                held.checked_add(elapsed).ok_or(Refusal::Overflow)?
            }
            _ => Duration::ZERO,
        };
        self.held = Some(held);
        Ok(if held >= required {
            Hold::Satisfied
        } else {
            Hold::Holding
        })
    }
}

#[cfg(test)]
mod tests;
