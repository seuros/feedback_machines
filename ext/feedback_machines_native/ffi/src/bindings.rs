use feedback_machines::{Band, Gains, Hysteresis, Limits, Pid, Refusal};
use magnus::typed_data::Obj;
use magnus::{
    Error, Integer, IntoValue, RClass, Ruby, Symbol, TryConvert, Value, function, method,
    prelude::*,
};
use std::{cell::RefCell, time::Duration};

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

fn gains(ruby: &Ruby, p: f64, i: f64, d: f64) -> Result<Gains, Error> {
    Gains::new(p, i, d).ok_or_else(|| {
        Error::new(
            ruby.exception_range_error(),
            "gains must be finite and nonnegative",
        )
    })
}

fn limits(ruby: &Ruby, min: f64, max: f64) -> Result<Limits, Error> {
    Limits::new(min, max).ok_or_else(|| {
        Error::new(
            ruby.exception_range_error(),
            "limits must be finite and min < max",
        )
    })
}

/// `FeedbackMachines::Gains` or `FeedbackMachines::Limits`.
fn data_class(ruby: &Ruby, name: &str) -> Result<RClass, Error> {
    ruby.define_module("FeedbackMachines")?.const_get(name)
}

/// A `Data` value of exactly the class `name`, or `TypeError`.
fn data(ruby: &Ruby, value: Value, name: &str) -> Result<Value, Error> {
    if value.is_kind_of(data_class(ruby, name)?) {
        return Ok(value);
    }
    Err(Error::new(
        ruby.exception_type_error(),
        format!("expected a FeedbackMachines::{name}"),
    ))
}

fn gains_from(ruby: &Ruby, value: Value) -> Result<Gains, Error> {
    let value = data(ruby, value, "Gains")?;
    gains(
        ruby,
        value.funcall("p", ())?,
        value.funcall("i", ())?,
        value.funcall("d", ())?,
    )
}

fn limits_from(ruby: &Ruby, value: Value) -> Result<Limits, Error> {
    let value = data(ruby, value, "Limits")?;
    limits(ruby, value.funcall("min", ())?, value.funcall("max", ())?)
}

fn limits_value(ruby: &Ruby, limits: Limits) -> Result<Value, Error> {
    data_class(ruby, "Limits")?.funcall("new", (limits.min(), limits.max()))
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
        data_class(ruby, "Gains")?.funcall("new", (gains.p(), gains.i(), gains.d()))
    }

    fn limits(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        limits_value(ruby, this.0.borrow().limits())
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
        limits_value(ruby, this.0.borrow().limits())
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
