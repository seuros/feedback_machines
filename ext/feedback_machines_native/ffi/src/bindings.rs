use crate::sealed::Sealed;
use feedback_machines::{Band, Gains, Hysteresis, Limits, Pid, Refusal};
use magnus::typed_data::Obj;
use magnus::{
    Error, Integer, IntoValue, Ruby, Symbol, TryConvert, Value, function, method, prelude::*,
};
use std::{cell::RefCell, time::Duration};

/// `FeedbackMachines::Gains`, resolved at init.
static GAINS: Sealed = Sealed::new("Gains");
/// `FeedbackMachines::Limits`, resolved at init.
static LIMITS: Sealed = Sealed::new("Limits");

/// A span as Integer nanoseconds.
#[derive(Clone, Copy)]
struct Nanos(u64);

impl TryConvert for Nanos {
    fn try_convert(value: Value) -> Result<Self, Error> {
        if Integer::from_value(value).is_none() {
            return Err(Error::new(
                Ruby::get_with(value).exception_type_error(),
                "elapsed must be an Integer number of nanoseconds",
            ));
        }
        u64::try_convert(value).map(Self)
    }
}

const fn refusal(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::NotFinite => "not_finite",
        Refusal::InvalidDuration => "invalid_duration",
        Refusal::Overflow => "overflow",
    }
}

const fn band(band: Band) -> &'static str {
    match band {
        Band::Low => "low",
        Band::High => "high",
    }
}

/// The value the core's constructor built, or `RangeError` where it answered
/// `None`.
fn validated<T>(ruby: &Ruby, value: Option<T>, message: &'static str) -> Result<T, Error> {
    value.ok_or_else(|| Error::new(ruby.exception_range_error(), message))
}

fn gains(ruby: &Ruby, p: f64, i: f64, d: f64) -> Result<Gains, Error> {
    let message = "gains must be finite and nonnegative";
    validated(ruby, Gains::new(p, i, d), message)
}

fn limits(ruby: &Ruby, min: f64, max: f64) -> Result<Limits, Error> {
    let message = "limits must be finite and min < max";
    validated(ruby, Limits::new(min, max), message)
}

/// The core's `Gains` of a `FeedbackMachines::Gains`: `TypeError` for
/// anything but an instance of exactly that class, its members read natively
/// and re-validated through `Gains::new`, so a forged value never reaches the
/// core.
fn gains_from(ruby: &Ruby, value: Value) -> Result<Gains, Error> {
    let members = GAINS.read(ruby, value, "gains")?;
    gains(
        ruby,
        members.get("p")?,
        members.get("i")?,
        members.get("d")?,
    )
}

/// The core's `Limits` of a `FeedbackMachines::Limits`, checked like
/// [`gains_from`].
fn limits_from(ruby: &Ruby, value: Value) -> Result<Limits, Error> {
    let members = LIMITS.read(ruby, value, "limits")?;
    limits(ruby, members.get("min")?, members.get("max")?)
}

/// A `FeedbackMachines::Limits` for `limits`. Callers copy the limits out
/// first: no borrow is held while `Limits.new` runs.
fn limits_value(ruby: &Ruby, limits: Limits) -> Result<Value, Error> {
    LIMITS.build(ruby, (limits.min(), limits.max()))
}

/// What `Gains.new` stores: the validated coefficients as Floats.
fn checked_gains(ruby: &Ruby, p: f64, i: f64, d: f64) -> Result<(f64, f64, f64), Error> {
    gains(ruby, p, i, d).map(|gains| (gains.p(), gains.i(), gains.d()))
}

/// What `Limits.new` stores: the validated range as Floats.
fn checked_limits(ruby: &Ruby, min: f64, max: f64) -> Result<(f64, f64), Error> {
    limits(ruby, min, max).map(|limits| (limits.min(), limits.max()))
}

#[magnus::wrap(class = "FeedbackMachines::Pid", free_immediately, size)]
struct Controller(RefCell<Pid>);

impl Controller {
    fn new(ruby: &Ruby, gains: Value, limits: Value) -> Result<Self, Error> {
        let (gains, limits) = (gains_from(ruby, gains)?, limits_from(ruby, limits)?);
        Ok(Self(RefCell::new(Pid::new(gains, limits))))
    }

    fn gains(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let gains = this.0.borrow().gains();
        GAINS.build(ruby, (gains.p(), gains.i(), gains.d()))
    }

    fn limits(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let limits = this.0.borrow().limits();
        limits_value(ruby, limits)
    }

    fn step(ruby: &Ruby, this: Obj<Self>, error: f64, elapsed: Nanos) -> Result<Value, Error> {
        this.check_frozen()?;
        let outcome = this
            .0
            .borrow_mut()
            .step(error, Duration::from_nanos(elapsed.0));
        Ok(match outcome {
            Ok(output) => output.into_value_with(ruby),
            Err(refused) => ruby.to_symbol(refusal(refused)).as_value(),
        })
    }
}

#[magnus::wrap(class = "FeedbackMachines::Hysteresis", free_immediately, size)]
struct BandController(RefCell<Hysteresis>);

impl BandController {
    fn new(ruby: &Ruby, limits: Value) -> Result<Self, Error> {
        Ok(Self(RefCell::new(Hysteresis::new(limits_from(
            ruby, limits,
        )?))))
    }

    fn limits(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let limits = this.0.borrow().limits();
        limits_value(ruby, limits)
    }

    fn state(ruby: &Ruby, this: &Self) -> Symbol {
        ruby.to_symbol(band(this.0.borrow().state()))
    }

    fn observe(ruby: &Ruby, this: Obj<Self>, value: f64) -> Result<Symbol, Error> {
        this.check_frozen()?;
        let outcome = this.0.borrow_mut().observe(value);
        Ok(ruby.to_symbol(outcome.map_or_else(refusal, band)))
    }
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("FeedbackMachines")?;
    GAINS.resolve(ruby, module)?;
    LIMITS.resolve(ruby, module)?;
    module.define_singleton_method("__gains", function!(checked_gains, 3))?;
    module.define_singleton_method("__limits", function!(checked_limits, 2))?;
    let pid = module.define_class("Pid", ruby.class_object())?;
    pid.undef_default_alloc_func();
    pid.define_singleton_method("new", function!(Controller::new, 2))?;
    pid.define_method("gains", method!(Controller::gains, 0))?;
    pid.define_method("limits", method!(Controller::limits, 0))?;
    pid.define_method("step", method!(Controller::step, 2))?;
    let band = module.define_class("Hysteresis", ruby.class_object())?;
    band.undef_default_alloc_func();
    band.define_singleton_method("new", function!(BandController::new, 1))?;
    band.define_method("limits", method!(BandController::limits, 0))?;
    band.define_method("state", method!(BandController::state, 0))?;
    band.define_method("observe", method!(BandController::observe, 1))
}
