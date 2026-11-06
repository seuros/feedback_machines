use feedback_machines::{Band, Gains, Hysteresis, Limits, Pid};
use magnus::typed_data::Obj;
use magnus::{Error, Ruby, Symbol, function, method, prelude::*};
use std::{cell::RefCell, time::Duration};

fn limits(ruby: &Ruby, min: f64, max: f64) -> Result<Limits, Error> {
    Limits::new(min, max).ok_or_else(|| {
        Error::new(
            ruby.exception_arg_error(),
            "limits must be finite and min < max",
        )
    })
}

#[magnus::wrap(class = "FeedbackMachines::Pid", free_immediately, size)]
struct Controller(RefCell<Pid>);

impl Controller {
    fn new(ruby: &Ruby, p: f64, i: f64, d: f64, min: f64, max: f64) -> Result<Self, Error> {
        let gains = Gains::new(p, i, d).ok_or_else(|| {
            Error::new(
                ruby.exception_arg_error(),
                "gains must be finite and nonnegative",
            )
        })?;
        Ok(Self(RefCell::new(Pid::new(gains, limits(ruby, min, max)?))))
    }
    fn step(ruby: &Ruby, this: Obj<Self>, error: f64, elapsed: f64) -> Result<Option<f64>, Error> {
        this.check_frozen()?;
        let elapsed = Duration::try_from_secs_f64(elapsed).map_err(|_| {
            Error::new(
                ruby.exception_arg_error(),
                "elapsed must be finite, nonnegative, and representable",
            )
        })?;
        Ok(this.0.borrow_mut().step(error, elapsed))
    }
}

#[magnus::wrap(class = "FeedbackMachines::Hysteresis", free_immediately, size)]
struct BandController(RefCell<Hysteresis>);

impl BandController {
    fn new(ruby: &Ruby, min: f64, max: f64) -> Result<Self, Error> {
        Ok(Self(RefCell::new(Hysteresis::new(limits(ruby, min, max)?))))
    }
    fn observe(ruby: &Ruby, this: Obj<Self>, value: f64) -> Result<Option<Symbol>, Error> {
        this.check_frozen()?;
        let result = this.0.borrow_mut().observe(value);
        Ok(result.map(|band| ruby.to_symbol(if band == Band::High { "high" } else { "low" })))
    }
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("FeedbackMachines")?;
    let pid = module.define_class("Pid", ruby.class_object())?;
    pid.define_singleton_method("_new", function!(Controller::new, 5))?;
    pid.define_method("_step", method!(Controller::step, 2))?;
    let band = module.define_class("Hysteresis", ruby.class_object())?;
    band.define_singleton_method("_new", function!(BandController::new, 2))?;
    band.define_method("observe", method!(BandController::observe, 1))
}
