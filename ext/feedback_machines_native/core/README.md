# feedback-machines

> **AMS Maxwell**, NCC-4208, *Maxwell*-class guidance tender.

Engine-room flight manual: turn measured drift into bounded control recommendations.

Dependency-free, allocation-free `no_std` controllers.

PID accepts explicit error and elapsed time, rejects nonfinite inputs, zero
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
use feedback_machines::{Gains, Limits, Pid};
# fn example() -> Option<()> {
let mut controller = Pid::new(Gains::new(2.0, 0.0, 0.0)?, Limits::new(0.0, 10.0)?);
assert_eq!(controller.step(2.0, Duration::from_secs(1)), Some(4.0));
# Some(())
# }
# assert_eq!(example(), Some(()));
```
