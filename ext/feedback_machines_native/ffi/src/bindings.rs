use feedback_machines::{Band, Gains, Hysteresis, Limits, Pid, Refusal};
use magnus::typed_data::Obj;
use magnus::{Error, IntoValue, RModule, Ruby, Symbol, Value, function, method, prelude::*};
use matryona_machines::ruby::{Class, Registry, Symbols, duration, exact_fields, real};
use std::cell::RefCell;

/// The `Data` classes the binding builds and reads, resolved once at init.
struct Classes {
    gains: Class,
    limits: Class,
}

static CLASSES: Registry<Classes> = Registry::new("feedback_machines");

/// Resolves the classes `lib/feedback_machines.rb` defined before it loaded
/// the extension. Called once, from init.
fn resolve(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let classes = Classes {
        gains: Class::resolve(ruby, module, "Gains")?,
        limits: Class::resolve(ruby, module, "Limits")?,
    };
    CLASSES.set(ruby, classes).map(drop)
}

matryona_machines::symbols! {
    /// The core's refusals as Symbols: the snake case of each variant.
    Refusals for Refusal as "a refusal" {
        NotFinite => "not_finite",
        InvalidDuration => "invalid_duration",
        Overflow => "overflow",
    }
}

matryona_machines::symbols! {
    /// The hysteresis bands as Symbols.
    Bands for Band as "a band" {
        Low => "low",
        High => "high",
    }
}

/// The value the core's constructor built, or `RangeError` where it answered
/// `None`.
fn validated<T>(ruby: &Ruby, value: Option<T>, message: &'static str) -> Result<T, Error> {
    value.ok_or_else(|| Error::new(ruby.exception_range_error(), message))
}

/// The core's `Gains` of three reals (exactly a Float or an Integer each,
/// no `to_f` run; NaN and the infinities are the core's to refuse).
fn gains(ruby: &Ruby, p: Value, i: Value, d: Value) -> Result<Gains, Error> {
    let (p, i, d) = (real(p, "p")?, real(i, "i")?, real(d, "d")?);
    let message = "gains must be finite and nonnegative";
    validated(ruby, Gains::new(p, i, d), message)
}

/// The core's `Limits` of two reals, read like [`gains`].
fn limits(ruby: &Ruby, min: Value, max: Value) -> Result<Limits, Error> {
    let (min, max) = (real(min, "min")?, real(max, "max")?);
    let message = "limits must be finite and min < max";
    validated(ruby, Limits::new(min, max), message)
}

/// The core's `Gains` of a `FeedbackMachines::Gains`: `TypeError` for
/// anything but an instance of exactly that class, its members read from the
/// struct itself (no reader runs) and re-validated through `Gains::new`, so a
/// forged value never reaches the core.
fn gains_from(ruby: &Ruby, value: Value) -> Result<Gains, Error> {
    let class = CLASSES.get(ruby)?.gains.get(ruby);
    let message = "gains must be a FeedbackMachines::Gains";
    let [p, i, d] = exact_fields::<3>(ruby, value, class, message)?;
    gains(ruby, p, i, d)
}

/// The core's `Limits` of a `FeedbackMachines::Limits`, checked like
/// [`gains_from`].
fn limits_from(ruby: &Ruby, value: Value) -> Result<Limits, Error> {
    let class = CLASSES.get(ruby)?.limits.get(ruby);
    let message = "limits must be a FeedbackMachines::Limits";
    let [min, max] = exact_fields::<2>(ruby, value, class, message)?;
    limits(ruby, min, max)
}

/// A `FeedbackMachines::Limits` for `limits`. Callers copy the limits out
/// first: no borrow is held while `Limits.new` runs.
fn limits_value(ruby: &Ruby, limits: Limits) -> Result<Value, Error> {
    let class = CLASSES.get(ruby)?.limits.get(ruby);
    class.funcall("new", (limits.min(), limits.max()))
}

/// What `Gains.new` stores: the validated coefficients as Floats.
fn checked_gains(ruby: &Ruby, p: Value, i: Value, d: Value) -> Result<(f64, f64, f64), Error> {
    gains(ruby, p, i, d).map(|gains| (gains.p(), gains.i(), gains.d()))
}

/// What `Limits.new` stores: the validated range as Floats.
fn checked_limits(ruby: &Ruby, min: Value, max: Value) -> Result<(f64, f64), Error> {
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
        let class = CLASSES.get(ruby)?.gains.get(ruby);
        class.funcall("new", (gains.p(), gains.i(), gains.d()))
    }

    fn limits(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let limits = this.0.borrow().limits();
        limits_value(ruby, limits)
    }

    fn step(ruby: &Ruby, this: Obj<Self>, error: Value, elapsed: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let error = real(error, "error")?;
        let elapsed = duration(elapsed, "elapsed")?;
        let outcome = this.0.borrow_mut().step(error, elapsed);
        Ok(match outcome {
            Ok(output) => output.into_value_with(ruby),
            Err(refused) => Refusals::symbol(ruby, refused).as_value(),
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
        Bands::symbol(ruby, this.0.borrow().state())
    }

    fn observe(ruby: &Ruby, this: Obj<Self>, value: Value) -> Result<Symbol, Error> {
        this.check_frozen()?;
        let value = real(value, "value")?;
        let outcome = this.0.borrow_mut().observe(value);
        Ok(match outcome {
            Ok(band) => Bands::symbol(ruby, band),
            Err(refused) => Refusals::symbol(ruby, refused),
        })
    }
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("FeedbackMachines")?;
    resolve(ruby, module)?;
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
