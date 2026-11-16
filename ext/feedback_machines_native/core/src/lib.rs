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
    /// The sample is finite but a term of the calculation is not.
    Overflow,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFinite => "the sample is NaN or infinite",
            Self::InvalidDuration => "the elapsed time is zero",
            Self::Overflow => "a term of the calculation is not finite",
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

/// A generic two-state hysteresis band.
#[derive(Debug, Clone, PartialEq)]
pub struct Hysteresis {
    limits: Limits,
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

impl Hysteresis {
    /// Start on the low side.
    #[must_use]
    pub const fn new(limits: Limits) -> Self {
        Self {
            limits,
            state: Band::Low,
        }
    }

    /// A band resuming saved state: on `state`, the [`Hysteresis::state`]
    /// read from one before it was put away. Every side is valid for every
    /// band.
    #[must_use]
    pub const fn restored(limits: Limits, state: Band) -> Self {
        Self { limits, state }
    }

    /// The thresholds this band was built with.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
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
    /// [`Refusal::NotFinite`] for a NaN or infinite `value`; the band keeps
    /// its side.
    pub fn observe(&mut self, value: f64) -> Result<Band, Refusal> {
        if !value.is_finite() {
            return Err(Refusal::NotFinite);
        }
        if value >= self.limits.max {
            self.state = Band::High;
        } else if value <= self.limits.min {
            self.state = Band::Low;
        }
        Ok(self.state)
    }
}

#[cfg(test)]
mod tests;
