# feedback-machines

> **AMS Maxwell**, NCC-4208, *Maxwell*-class guidance tender.

Engine-room flight manual: turn measured drift into bounded control recommendations.

Dependency-free, allocation-free `no_std` controllers.

PID accepts explicit error and elapsed time, and refuses nonfinite inputs, zero
sampling intervals, and overflowing intermediate calculations without changing
state. Derivative is taken on error; the first sample has zero derivative.
Output is clamped and conditional integration prevents windup while saturation
and error point in the same direction. There is no derivative filtering,
auto-tuning, or stability guarantee. Gains must be finite and nonnegative;
reverse the error convention for inverse-action systems.

Hysteresis switches high at/above its upper threshold and low at/below its lower
threshold. It retains state inside the band. Unlike flow watermarks, these are
generic signed floating-point measurements, not queue occupancy.

```rust
use core::time::Duration;
use feedback_machines::{Gains, Limits, Pid, Refusal};
# fn example() -> Option<()> {
let mut controller = Pid::new(Gains::new(2.0, 0.0, 0.0)?, Limits::new(0.0, 10.0)?);
assert_eq!(controller.step(2.0, Duration::from_secs(1)), Ok(4.0));
assert_eq!(controller.step(f64::NAN, Duration::from_secs(1)), Err(Refusal::NotFinite));
assert_eq!(controller.step(2.0, Duration::ZERO), Err(Refusal::InvalidDuration));
# Some(())
# }
# assert_eq!(example(), Some(()));
```

## Contract

- **Configuration.** `Gains::new(p, i, d)` and `Limits::new(min, max)` are
  constructors that validate and answer `None`: gains must be finite and
  nonnegative, limits finite with `min < max`. `Pid::new(gains, limits)` and
  `Hysteresis::new(limits)` take validated values and cannot fail. Readers:
  `Gains::{p, i, d}`, `Limits::{min, max}`, `Pid::{gains, limits}`,
  `Hysteresis::{limits, state}`.
- **Refusals.** `Refusal` is the one refusal type. `Pid::step` answers
  `Result<f64, Refusal>` and `Hysteresis::observe` answers
  `Result<Band, Refusal>`; `Ok` is always an honoured sample.
  - `NotFinite`: the sample (`error` or `value`) is NaN or infinite. Checked
    first, so a bad sample is `NotFinite` whatever the interval.
  - `InvalidDuration`: `elapsed` is zero. `step` only.
  - `Overflow`: the sample is finite but the derivative, the integral or the
    output is not. `step` only.
- **A refusal changes nothing.** Not the integral, not the derivative history,
  not the band's side: the value compares equal to a clone taken before the
  call. Skip the sample and carry on.
- **No repeats.** Every honoured `step` is a new sample that advances the
  integral and the derivative history; the controller keeps no record of past
  calls, so there is no idempotent repeat to recognise. Observing the same
  measurement twice is simply two observations with the same answer.
- **Time.** `elapsed` is a `core::time::Duration`: the span since the previous
  sample, not an instant. The controllers read no clock and keep none. `error`,
  the gains, the limits and the output are `f64` control quantities, not time.
- **No recorded clock.** The rule "an honoured call records `now` exactly when
  its answer depends on `now`" has nothing to act on here: no method takes an
  instant, and neither controller keeps one.
- **No unreachable arms.** The crate has no `panic!`, `assert!`,
  `unreachable!` or `expect`, and no refusal that cannot occur.
- **Exhaustive.** `Band` and `Refusal` are exhaustive enums; a new variant is a
  breaking change.
- **No ids.** Nothing is issued, so there are no scopes and nothing to resume.
  A controller's state is two numbers and is not persisted for you.
