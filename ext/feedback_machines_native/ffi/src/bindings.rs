use feedback_machines::{Band, Dwell, Gains, Hold, Hysteresis, Limits, Pid, Refusal, Thresholds};
use magnus::typed_data::Obj;
use magnus::{Error, IntoValue, RModule, Ruby, Symbol, Value, function, method, prelude::*};
use matryona_machines::ruby::{
    Class, Registry, Symbols, boolean, duration, exact_fields, instant, nanoseconds, real,
};
use std::cell::RefCell;

/// The `Data` classes the binding builds and reads, resolved once at init.
struct Classes {
    gains: Class,
    limits: Class,
    thresholds: Class,
}

static CLASSES: Registry<Classes> = Registry::new("feedback_machines");

/// Resolves the classes `lib/feedback_machines.rb` defined before it loaded
/// the extension. Called once, from init.
fn resolve(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let classes = Classes {
        gains: Class::resolve(ruby, module, "Gains")?,
        limits: Class::resolve(ruby, module, "Limits")?,
        thresholds: Class::resolve(ruby, module, "Thresholds")?,
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

matryona_machines::symbols! {
    /// How a dwell stands, as Symbols.
    Holds for Hold as "a hold" {
        Interrupted => "interrupted",
        Holding => "holding",
        Satisfied => "satisfied",
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

/// The core's `Thresholds` of two reals, read like [`gains`].
fn thresholds(ruby: &Ruby, low: Value, high: Value) -> Result<Thresholds, Error> {
    let (low, high) = (real(low, "low")?, real(high, "high")?);
    let message = "thresholds must be finite and low <= high";
    validated(ruby, Thresholds::new(low, high), message)
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

/// The core's `Thresholds` of a `FeedbackMachines::Thresholds`, checked like
/// [`gains_from`].
fn thresholds_from(ruby: &Ruby, value: Value) -> Result<Thresholds, Error> {
    let class = CLASSES.get(ruby)?.thresholds.get(ruby);
    let message = "thresholds must be a FeedbackMachines::Thresholds";
    let [low, high] = exact_fields::<2>(ruby, value, class, message)?;
    thresholds(ruby, low, high)
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

/// What `Thresholds.new` stores: the validated band as Floats.
fn checked_thresholds(ruby: &Ruby, low: Value, high: Value) -> Result<(f64, f64), Error> {
    thresholds(ruby, low, high).map(|thresholds| (thresholds.low(), thresholds.high()))
}

#[magnus::wrap(class = "FeedbackMachines::Pid", free_immediately, size)]
struct Controller(RefCell<Pid>);

impl Controller {
    fn new(ruby: &Ruby, gains: Value, limits: Value) -> Result<Self, Error> {
        let (gains, limits) = (gains_from(ruby, gains)?, limits_from(ruby, limits)?);
        Ok(Self(RefCell::new(Pid::new(gains, limits))))
    }

    /// `Pid.restored(gains, limits, integral, previous_error)`: `RangeError`
    /// where the core's `Pid::restored` answers `None`.
    fn restored(
        ruby: &Ruby,
        gains: Value,
        limits: Value,
        integral: Value,
        previous_error: Value,
    ) -> Result<Self, Error> {
        let (gains, limits) = (gains_from(ruby, gains)?, limits_from(ruby, limits)?);
        let integral = real(integral, "integral")?;
        let previous_error = if previous_error.is_nil() {
            None
        } else {
            Some(real(previous_error, "previous_error")?)
        };
        let message = "integral and previous_error must be finite";
        let pid = Pid::restored(gains, limits, integral, previous_error);
        validated(ruby, pid, message).map(|pid| Self(RefCell::new(pid)))
    }

    fn integral(&self) -> f64 {
        self.0.borrow().integral()
    }

    fn previous_error(&self) -> Option<f64> {
        self.0.borrow().previous_error()
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
    fn new(ruby: &Ruby, thresholds: Value) -> Result<Self, Error> {
        let thresholds = thresholds_from(ruby, thresholds)?;
        Ok(Self(RefCell::new(Hysteresis::new(thresholds))))
    }

    /// `Hysteresis.restored(thresholds, state)`: every side is valid for
    /// every band, so only a non-Symbol (`TypeError`) or an unknown one
    /// (`RangeError`) raises.
    fn restored(ruby: &Ruby, thresholds: Value, state: Value) -> Result<Self, Error> {
        let thresholds = thresholds_from(ruby, thresholds)?;
        let state = Bands::parse(ruby, state)?;
        Ok(Self(RefCell::new(Hysteresis::restored(thresholds, state))))
    }

    /// A `FeedbackMachines::Thresholds` for the band's thresholds, copied out
    /// first: no borrow is held while `Thresholds.new` runs.
    fn thresholds(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let thresholds = this.0.borrow().thresholds();
        let class = CLASSES.get(ruby)?.thresholds.get(ruby);
        class.funcall("new", (thresholds.low(), thresholds.high()))
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

#[magnus::wrap(class = "FeedbackMachines::Dwell", free_immediately, size)]
struct DwellTimer(RefCell<Dwell>);

impl DwellTimer {
    fn new(max_gap: Value) -> Result<Self, Error> {
        let max_gap = duration(max_gap, "max_gap")?;
        Ok(Self(RefCell::new(Dwell::new(max_gap))))
    }

    /// `Dwell.restored(max_gap, held)`: `RangeError` where the core's
    /// `Dwell::restored` answers `None`. `held` is read with the full-range
    /// reader: the dwell can answer a span past `u64` nanoseconds.
    fn restored(ruby: &Ruby, max_gap: Value, held: Value) -> Result<Self, Error> {
        let max_gap = duration(max_gap, "max_gap")?;
        let held = if held.is_nil() {
            None
        } else {
            Some(instant(held, "held")?)
        };
        let message = "a zero max_gap never holds past zero";
        let dwell = Dwell::restored(max_gap, held);
        validated(ruby, dwell, message).map(|dwell| Self(RefCell::new(dwell)))
    }

    fn max_gap(ruby: &Ruby, this: &Self) -> Value {
        nanoseconds(ruby, this.0.borrow().max_gap()).as_value()
    }

    fn held(ruby: &Ruby, this: &Self) -> Value {
        let held = this.0.borrow().held();
        held.map_or_else(
            || ruby.qnil().as_value(),
            |held| nanoseconds(ruby, held).as_value(),
        )
    }

    fn observe(
        ruby: &Ruby,
        this: Obj<Self>,
        holds: Value,
        elapsed: Value,
        required: Value,
    ) -> Result<Symbol, Error> {
        this.check_frozen()?;
        let holds = boolean(holds, "holds")?;
        let elapsed = duration(elapsed, "elapsed")?;
        let required = instant(required, "required")?;
        let outcome = this.0.borrow_mut().observe(holds, elapsed, required);
        Ok(match outcome {
            Ok(hold) => Holds::symbol(ruby, hold),
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
    module.define_singleton_method("__thresholds", function!(checked_thresholds, 2))?;
    let pid = module.define_class("Pid", ruby.class_object())?;
    pid.undef_default_alloc_func();
    pid.define_singleton_method("new", function!(Controller::new, 2))?;
    pid.define_singleton_method("restored", function!(Controller::restored, 4))?;
    pid.define_method("integral", method!(Controller::integral, 0))?;
    pid.define_method("previous_error", method!(Controller::previous_error, 0))?;
    pid.define_method("gains", method!(Controller::gains, 0))?;
    pid.define_method("limits", method!(Controller::limits, 0))?;
    pid.define_method("step", method!(Controller::step, 2))?;
    let band = module.define_class("Hysteresis", ruby.class_object())?;
    band.undef_default_alloc_func();
    band.define_singleton_method("new", function!(BandController::new, 1))?;
    band.define_singleton_method("restored", function!(BandController::restored, 2))?;
    band.define_method("thresholds", method!(BandController::thresholds, 0))?;
    band.define_method("state", method!(BandController::state, 0))?;
    band.define_method("observe", method!(BandController::observe, 1))?;
    let dwell = module.define_class("Dwell", ruby.class_object())?;
    dwell.undef_default_alloc_func();
    dwell.define_singleton_method("new", function!(DwellTimer::new, 1))?;
    dwell.define_singleton_method("restored", function!(DwellTimer::restored, 2))?;
    dwell.define_method("max_gap", method!(DwellTimer::max_gap, 0))?;
    dwell.define_method("held", method!(DwellTimer::held, 0))?;
    dwell.define_method("observe", method!(DwellTimer::observe, 3))?;
    // `new` and `restored` answer these: their classes are fixed now, so
    // building a controller never runs `Object.const_get`.
    Class::pin_wrapped::<Controller>(ruby, pid)?;
    Class::pin_wrapped::<BandController>(ruby, band)?;
    Class::pin_wrapped::<DwellTimer>(ruby, dwell).map(drop)
}
