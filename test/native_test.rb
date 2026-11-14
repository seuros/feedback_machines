# frozen_string_literal: true
require 'test_helper'

class FeedbackNativeTest < Minitest::Test
  F = FeedbackMachines
  SECOND = 1_000_000_000

  def pid(p, i, d, min, max) = F::Pid.new(F::Gains.new(p, i, d), F::Limits.new(min, max))

  def test_pid_saturation_and_conditional_integration
    pid = pid(1.0, 1.0, 0.0, -10.0, 10.0)
    assert_in_delta 10.0, pid.step(100.0, SECOND)
    assert_in_delta 2.0, pid.step(1.0, SECOND)
    assert_equal :not_finite, pid.step(Float::NAN, SECOND)
    assert_equal :invalid_duration, pid.step(1.0, 0)
    assert_in_delta 3.0, pid.step(1.0, SECOND)
  end

  def test_elapsed_is_integer_nanoseconds
    pid = pid(0.0, 0.0, 1.0, -100.0, 100.0)
    assert_in_delta 0.0, pid.step(5.0, SECOND)
    assert_in_delta 2.0, pid.step(6.0, SECOND / 2)
    assert_in_delta 100.0, pid.step(7.0, 1)
    [1.0, 0.5, nil, '1', Rational(1, 2)].each do |invalid|
      assert_raises(TypeError) { pid.step(1.0, invalid) }
    end
    assert_raises(RangeError) { pid.step(1.0, -1) }
    assert_raises(RangeError) { pid.step(1.0, 2**64) }
    assert_equal 'elapsed must be an Integer', assert_raises(TypeError) { pid.step(1.0, 0.5) }.message
    assert_equal "elapsed must be in 0..=#{(2**64) - 1}", assert_raises(RangeError) { pid.step(1.0, -1) }.message
    assert_kind_of Float, pid.step(7.0, (2**64) - 1)
    assert_raises(ArgumentError) { pid.step(error: 1.0, elapsed: SECOND) }
    assert_raises(ArgumentError) { pid.step(1.0) }
  end

  def test_every_refusal_and_raise_changes_nothing
    pid = pid(1.0, 1.0, 1.0, -1000.0, 1000.0)
    twin = pid(1.0, 1.0, 1.0, -1000.0, 1000.0)
    [pid, twin].each { |controller| assert_in_delta 4.0, controller.step(2.0, SECOND) }
    [Float::NAN, Float::INFINITY, -Float::INFINITY].each do |error|
      assert_equal :not_finite, pid.step(error, SECOND)
      assert_equal :not_finite, pid.step(error, 0)
    end
    assert_equal :invalid_duration, pid.step(2.0, 0)
    assert_equal :overflow, pid.step(Float::MAX, (2**64) - 1)
    assert_equal :overflow, pid.step(1.0e300, 1)
    # The binding never loses work: whatever raises does so before the core
    # runs, so a call that raised stepped nothing either.
    assert_raises(TypeError) { pid.step('3', SECOND) }
    assert_raises(TypeError) { pid.step(3.0, 1.5) }
    assert_raises(RangeError) { pid.step(3.0, -1) }
    5.times { assert_equal twin.step(3.0, SECOND / 4), pid.step(3.0, SECOND / 4) }
    band = F::Hysteresis.new(F::Limits.new(0, 10))
    assert_raises(TypeError) { band.observe('11') }
    assert_equal :low, band.state
  end

  def test_configuration_mirrors_the_core
    gains = F::Gains.new(1, 0, 2)
    limits = F::Limits.new(-1, 1)
    assert_equal %i[p i d], F::Gains.members
    assert_equal %i[min max], F::Limits.members
    assert_equal [1.0, 0.0, 2.0], gains.to_h.values
    assert(gains.to_h.values.all?(Float))
    assert_equal F::Gains.new(p: 1.0, i: 0.0, d: 2.0), gains
    pid = F::Pid.new(gains, limits)
    assert_equal gains, pid.gains
    assert_equal limits, pid.limits
    assert_equal limits, F::Hysteresis.new(limits).limits
    assert_raises(ArgumentError) { F::Gains.new(1.0) }
    assert_raises(ArgumentError) { F::Pid.new(gains) }
    assert_raises(ArgumentError) { F::Pid.new(p: 1.0, min: 0.0, max: 1.0) }
    assert_raises(TypeError) { F::Pid.new([1.0, 0.0, 0.0], limits) }
    assert_raises(TypeError) { F::Pid.new(limits, gains) }
    assert_raises(TypeError) { F::Hysteresis.new(gains) }
    assert_raises(TypeError) { F::Pid.allocate }
    assert_raises(NoMethodError) { F.__gains(1.0, 0.0, 0.0) }
  end

  def test_invalid_configuration_raises_where_the_core_answers_none
    [[-1.0, 0.0, 0.0], [0.0, Float::NAN, 0.0], [0.0, 0.0, Float::INFINITY]].each do |p, i, d|
      assert_raises(RangeError) { F::Gains.new(p, i, d) }
    end
    [[1.0, 1.0], [2.0, 1.0], [-Float::INFINITY, 0.0], [0.0, Float::NAN]].each do |min, max|
      assert_raises(RangeError) { F::Limits.new(min, max) }
    end
    assert_raises(TypeError) { F::Gains.new(nil, 0.0, 0.0) }
    assert_raises(TypeError) { F::Gains.new('1', 0.0, 0.0) }
    assert_raises(TypeError) { F::Limits.new(0.0, nil) }
    assert_raises(RangeError) { F::Gains.new(1.0, 0.0, 0.0).with(p: -1.0) }
  end

  def test_derivative_history_and_independent_controllers_survive_gc
    pid = pid(0.0, 0.0, 1.0, -100.0, 100.0)
    other = pid(0.0, 0.0, 1.0, -100.0, 100.0)
    assert_in_delta 0.0, pid.step(5.0, SECOND)
    GC.compact
    assert_in_delta 2.0, pid.step(6.0, SECOND / 2)
    assert_in_delta 0.0, other.step(6.0, SECOND / 2)
    assert_raises(TypeError) { pid.dup }
    assert_raises(TypeError) { other.clone }
  end

  def test_hysteresis_retains_state_between_thresholds
    band = F::Hysteresis.new(F::Limits.new(2.0, 5.0))
    assert_equal :low, band.state
    assert_equal :low, band.observe(3.0)
    assert_equal :high, band.observe(5.0)
    assert_equal :high, band.observe(3)
    [Float::INFINITY, -Float::INFINITY, Float::NAN].each do |value|
      assert_equal :not_finite, band.observe(value)
      assert_equal :high, band.state
    end
    assert_equal :low, band.observe(2.0)
    assert_equal :low, band.state
    assert_raises(TypeError) { band.observe(nil) }
  end

  def test_freezing
    pid = pid(1.0, 0.0, 0.0, 0.0, 1.0).freeze
    assert_raises(FrozenError) { pid.step(1.0, SECOND) }
    # Frozen is checked first, before the arguments are read.
    assert_raises(FrozenError) { pid.step('1', 0.5) }
    assert_equal 1.0, pid.gains.p
    band = F::Hysteresis.new(F::Limits.new(0.0, 1.0)).freeze
    assert_raises(FrozenError) { band.observe(1.0) }
    assert_raises(FrozenError) { band.observe(nil) }
    assert_equal :low, band.state
  end

  def test_randomized_steps_match_a_ruby_reference
    random = Random.new(4208)
    20.times do
      gains = F::Gains.new(random.rand(4.0), random.rand(2.0), random.rand(2.0))
      limits = F::Limits.new(-random.rand(50.0) - 1, random.rand(50.0) + 1)
      pid = F::Pid.new(gains, limits)
      integral = 0.0
      previous = nil
      200.times do
        error = { 0 => Float::NAN, 1 => Float::INFINITY }.fetch(random.rand(12), random.rand(-50.0..50.0))
        elapsed = random.rand(5) == 0 ? 0 : random.rand(1..5_000_000_000)
        answer = pid.step(error, elapsed)
        if !error.finite?
          assert_equal :not_finite, answer
        elsif elapsed.zero?
          assert_equal :invalid_duration, answer
        else
          dt = elapsed / 1e9
          slope = previous ? (error - previous) / dt : 0.0
          candidate = integral + (error * dt)
          proposed = (gains.p * error) + (gains.d * slope) + (gains.i * candidate)
          winding = (proposed > limits.max && error.positive?) || (proposed < limits.min && error.negative?)
          integral = candidate unless winding
          previous = error
          assert_in_delta proposed.clamp(limits.min, limits.max), answer, 1e-6
        end
      end
    end
  end
end
