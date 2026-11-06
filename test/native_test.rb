# frozen_string_literal: true
require 'test_helper'

class FeedbackNativeTest < Minitest::Test
  def test_pid_saturation_and_conditional_integration
    pid = FeedbackMachines::Pid.new(p: 1.0, i: 1.0, min: -10.0, max: 10.0)
    assert_in_delta 10.0, pid.step(error: 100.0, elapsed: 1.0)
    assert_in_delta 2.0, pid.step(error: 1.0, elapsed: 1.0)
    assert_nil pid.step(error: Float::NAN, elapsed: 1.0)
    assert_nil pid.step(error: 1.0, elapsed: 0.0)
    assert_in_delta 3.0, pid.step(error: 1.0, elapsed: 1.0)
  end

  def test_derivative_history_and_independent_controllers_survive_gc
    pid = FeedbackMachines::Pid.new(p: 0.0, d: 1.0, min: -100.0, max: 100.0)
    other = FeedbackMachines::Pid.new(p: 0.0, d: 1.0, min: -100.0, max: 100.0)
    assert_in_delta 0.0, pid.step(error: 5.0, elapsed: 1.0)
    GC.compact
    assert_in_delta 2.0, pid.step(error: 6.0, elapsed: 0.5)
    assert_in_delta 0.0, other.step(error: 6.0, elapsed: 0.5)
    assert_raises(TypeError) { pid.dup }
    assert_raises(TypeError) { other.clone }
    assert_raises(ArgumentError) { pid.step(error: 1.0, elapsed: 1.0, typo: true) }
  end

  def test_hysteresis_retains_state_between_thresholds
    band = FeedbackMachines::Hysteresis.new(min: 2.0, max: 5.0)
    assert_equal :low, band.observe(3.0)
    assert_equal :high, band.observe(5.0)
    assert_equal :high, band.observe(3.0)
    assert_nil band.observe(Float::INFINITY)
    assert_equal :low, band.observe(2.0)
  end

  def test_invalid_configuration_time_and_freezing
    assert_raises(ArgumentError) { FeedbackMachines::Pid.new(p: -1.0, min: 0.0, max: 1.0) }
    assert_raises(ArgumentError) { FeedbackMachines::Hysteresis.new(min: 1.0, max: 1.0) }
    pid = FeedbackMachines::Pid.new(p: 1.0, min: 0.0, max: 1.0)
    assert_raises(ArgumentError) { pid.step(error: 1.0, elapsed: -1.0) }
    assert_raises(ArgumentError) { pid.step(error: 1.0, elapsed: Float::NAN) }
    pid.freeze
    assert_raises(FrozenError) { pid.step(error: 1.0, elapsed: 1.0) }
    band = FeedbackMachines::Hysteresis.new(min: 0.0, max: 1.0).freeze
    assert_raises(FrozenError) { band.observe(1.0) }
  end
end
