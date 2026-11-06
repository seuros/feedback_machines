# FeedbackMachines

> Proportion, memory, and anticipation. The three things a threshold doesn't have. 🎛️

Two small control loops for Ruby. A `Pid` turns "how far off are we" into "how much to adjust", bounded so it can never ask for more than the actuator can give. A `Hysteresis` band turns a noisy reading into a clean on/off decision that doesn't flap. You supply the measurements and the clock. They supply the arithmetic.

## Quick Start

```bash
gem 'feedback_machines'
```

```ruby
# Keep the job queue near 50 pending by adding or removing workers.
pid = FeedbackMachines::Pid.new(p: 0.5, i: 0.1, min: -4.0, max: 4.0)

loop do
  error = queue.depth - 50
  delta = pid.step(error: error, elapsed: seconds_since_last_tick)
  pool.resize(pool.size + delta.round) if delta
  sleep 1
end
```

```ruby
# Flip into shed mode above 80% CPU, back out below 60%, nothing in between.
band = FeedbackMachines::Hysteresis.new(min: 0.6, max: 0.8)

shed_mode = band.observe(cpu.utilization) == :high
```

## A Message to the Resistance

So the queue backed up and you wrote the autoscaler. If depth is over 100, add a worker. If it's under 10, remove one. Ship it.

Monday morning the depth hits 101. A worker spins up. It takes 20 seconds to boot, by which time depth is 140 and the rule has fired three more times. Four new workers drain the queue to zero in a minute. Now depth is under 10, so the rule removes them, one per tick, and the queue backs up again. Your dashboard looks like a seismograph and ops has named the pattern after you.

The problem isn't the rule. The problem is that a threshold has no memory and no sense of proportion. It can't tell "a little over" from "catastrophically over", and it can't tell "climbing" from "already falling". Control engineers solved this in the 1920s for steering ships. A proportional term scales the response to the size of the error. An integral term remembers how long you've been off. A derivative term notices which way you're heading. Clamp the output so you never ask for forty workers at once, and stop accumulating when you're already clamped so the controller doesn't owe the universe an impossible correction later.

That is what `Pid` does. `Hysteresis` is the same cure for on/off decisions: two thresholds instead of one, so a reading hovering at 79.9% doesn't toggle shed mode fifty times a minute.

## Chapter 1: Stardate 2025.186, the AMS Maxwell

> **AMS Maxwell**, NCC-4208, *Maxwell*-class guidance tender.
> Correct the heading without teaching the thrusters to oscillate forever.

The tender doesn't fly the ship. It takes a heading error and a sampling interval and returns a thruster command the engines can actually execute. The helm reads the compass and fires the thrusters. The tender just makes sure that when the ship is one degree off, the correction is one degree's worth, and when it has been two degrees off for ten minutes, somebody finally does something about it.

On the bridge, a cadet proposes:

```ruby
def correct_heading
  if compass.error > 0
    thrusters.fire(:port, 100)
  elsif compass.error < 0
    thrusters.fire(:starboard, 100)
  end
end
```

"That," says the navigator, "is how a ship ends up pointing everywhere except where it's going."

## The Flight Plan

The hysteresis band has two states and refuses to change its mind in the middle.

```mermaid
stateDiagram-v2
    [*] --> low
    low --> high: reading >= max
    low --> low: reading < max
    high --> low: reading <= min
    high --> high: reading > min
```

And one tick of the queue autoscaler:

```mermaid
sequenceDiagram
    participant Tick
    participant Queue
    participant Pid
    participant Pool

    Tick->>Queue: depth?
    Queue-->>Tick: 70
    Tick->>Pid: step(error: 20, elapsed: 1.0)
    Note over Pid: p: 0.5 × 20 = 10, clamped to 4
    Pid-->>Tick: 4.0
    Tick->>Pool: resize(+4)

    Tick->>Queue: depth?
    Queue-->>Tick: 54
    Tick->>Pid: step(error: 4, elapsed: 1.0)
    Note over Pid: 2 + integral 0.4
    Pid-->>Tick: 2.4
    Tick->>Pool: resize(+2)

    Tick->>Queue: depth?
    Queue-->>Tick: 48
    Tick->>Pid: step(error: -2, elapsed: 1.0)
    Pid-->>Tick: -0.8
    Tick->>Pool: resize(-1)
```

## How the Pid Works

### Proportional

With only `p` set, the output is the error times the gain, clamped to the limits.

```ruby
pid = FeedbackMachines::Pid.new(p: 1.0, min: -10.0, max: 10.0)

pid.step(error: 3.0, elapsed: 1.0)    # => 3.0
pid.step(error: -3.0, elapsed: 1.0)   # => -3.0
pid.step(error: 30.0, elapsed: 1.0)   # => 10.0, clamped
```

### Integral, without windup

Add `i` and the controller starts remembering. Each step adds `error × elapsed` to a running total. But when the output is already pinned at a limit, the total stops growing. That's the part that keeps a saturated actuator from accumulating a debt it can never repay.

```ruby
pid = FeedbackMachines::Pid.new(p: 1.0, i: 1.0, min: -10.0, max: 10.0)

pid.step(error: 100.0, elapsed: 1.0)  # => 10.0, clamped, integral frozen
pid.step(error: 1.0, elapsed: 1.0)    # => 2.0, not 101.0
pid.step(error: 1.0, elapsed: 1.0)    # => 3.0, integral climbing normally now
```

Without that guard, the second step would try to pay back a hundred units of accumulated error and overshoot for the next minute.

### Derivative

Add `d` and the controller reacts to the *rate* of change. The first step has nothing to compare against, so the derivative term is zero.

```ruby
pid = FeedbackMachines::Pid.new(p: 0.0, d: 1.0, min: -100.0, max: 100.0)

pid.step(error: 5.0, elapsed: 1.0)    # => 0.0, no history yet
pid.step(error: 6.0, elapsed: 0.5)    # => 2.0, rose by 1 over 0.5s
pid.step(error: 6.0, elapsed: 0.5)    # => 0.0, holding steady
```

### Bad samples don't poison the history

A zero interval, a NaN reading, or an infinite one returns `nil` and leaves the controller exactly as it was. Skip that tick and carry on.

```ruby
pid = FeedbackMachines::Pid.new(p: 1.0, i: 1.0, min: -10.0, max: 10.0)

pid.step(error: 2.0, elapsed: 1.0)          # => 4.0
pid.step(error: 2.0, elapsed: 0.0)          # => nil
pid.step(error: Float::NAN, elapsed: 1.0)   # => nil
pid.step(error: 2.0, elapsed: 1.0)          # => 6.0, as if the two bad ticks never happened
```

## How the Band Works

Two thresholds. The band starts `:low` and only goes `:high` when a reading reaches `max`. It only comes back down when a reading falls to `min`. Everything in between keeps whatever it was.

```ruby
band = FeedbackMachines::Hysteresis.new(min: 0.6, max: 0.8)

band.observe(0.7)    # => :low, started low, not at max yet
band.observe(0.85)   # => :high
band.observe(0.7)    # => :high, in the dead zone, holds
band.observe(0.79)   # => :high, still holds
band.observe(0.6)    # => :low, reached min
band.observe(0.7)    # => :low, dead zone again, holds
```

A non-finite reading returns `nil` and the band keeps its state.

## The Rules of the Tender

- **Gains are finite and nonnegative.** Negative or infinite `p`, `i`, or `d` raises `ArgumentError`. Flip the sign of your error instead.
- **Limits need `min < max`.** Equal or reversed raises `ArgumentError`. Same for the band.
- **`elapsed` is a duration in seconds**, not a timestamp. Negative or NaN raises `ArgumentError`. Zero returns `nil`.
- **Integers are fine.** `p: 1, min: 0, max: 1` works. Everything is treated as a float.
- **Frozen means frozen.** `freeze` a controller or a band and every `step` or `observe` raises `FrozenError`.
- **Each controller is its own loop.** Two `Pid` instances share nothing, and they can't be `dup`ed or `clone`d.

## What You Think You're Doing vs Reality

### You Think: "I'll just add a worker when the queue is over 100."
### Reality: You built an oscillator with a twenty-second period.

A threshold reacts to where you are. It has no idea how fast you got there or how long you've been stuck. By the time the action lands, the situation has moved, so the action is wrong, so the next action overcorrects. Proportion, memory, and anticipation are the three things a threshold lacks, and they're the three letters in PID.

## What's Yours

The controller doesn't read sensors, run timers, or touch actuators. You decide what "error" means, how often to sample, what to do with the number that comes back, and what gains make sense for your system. Start with `p` alone, add `i` when the system settles short of the target, add `d` only if it overshoots.

## License

MIT License. See [LICENSE.txt](LICENSE.txt) for details.

**Remember: A threshold tells you that you're off course. A controller tells you by how much, for how long, and which way you're drifting.**
