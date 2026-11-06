#![no_std]
#![doc = include_str!("../README.md")]

/// Finite, nonnegative proportional/integral/derivative gains.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gains {
    p: f64,
    i: f64,
    d: f64,
}

impl Gains {
    /// Validate gain coefficients.
    #[must_use]
    pub fn new(p: f64, i: f64, d: f64) -> Option<Self> {
        [p, i, d]
            .iter()
            .all(|x| x.is_finite() && *x >= 0.0)
            .then_some(Self { p, i, d })
    }
}

/// Finite ordered output limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    min: f64,
    max: f64,
}

impl Limits {
    /// Validate a nonempty finite output range.
    #[must_use]
    pub const fn new(min: f64, max: f64) -> Option<Self> {
        if min.is_finite() && max.is_finite() && min < max {
            Some(Self { min, max })
        } else {
            None
        }
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

    /// Observe one sample; invalid/nonfinite calculations leave state unchanged.
    #[must_use]
    pub fn step(&mut self, error: f64, elapsed: core::time::Duration) -> Option<f64> {
        let dt = elapsed.as_secs_f64();
        if !error.is_finite() || dt == 0.0 {
            return None;
        }
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
            return None;
        }
        let winding_up = (proposed > self.limits.max && error > 0.0)
            || (proposed < self.limits.min && error < 0.0);
        let integral = if winding_up { self.integral } else { candidate };
        self.integral = integral;
        self.previous = Some(error);
        Some(proposed.clamp(self.limits.min, self.limits.max))
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
#[non_exhaustive]
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

    /// Observe a finite measurement; invalid samples leave state unchanged.
    #[must_use]
    pub fn observe(&mut self, value: f64) -> Option<Band> {
        if !value.is_finite() {
            return None;
        }
        if value >= self.limits.max {
            self.state = Band::High;
        } else if value <= self.limits.min {
            self.state = Band::Low;
        }
        Some(self.state)
    }
}

#[cfg(test)]
mod tests;
