# frozen_string_literal: true

require 'minitest/autorun'
require 'feedback_machines'

# Gains and Limits are sealed, and sealing stops accidents, not reflection:
# the binding checks the exact class of every value it reads and re-validates
# its members through the core's constructor.
class SealedTest < Minitest::Test
  F = FeedbackMachines

  # A named subclass raises, but Ruby binds the constant before `inherited`
  # runs, so `ForgedLimits` leaks; reopened, it skips Limits' validation.
  begin
    class ForgedLimits < FeedbackMachines::Limits; end
  rescue TypeError => e
    NAMED_SUBCLASS_ERROR = e
  end

  class ForgedLimits
    def initialize(min:, max:) = Data.instance_method(:initialize).bind_call(self, min:, max:)
  end

  def test_values_are_sealed
    [F::Gains, F::Limits].each do |value|
      assert_raises(NoMethodError) { value.allocate }
      assert_raises(TypeError) { Class.new(value) }
    end
    assert_kind_of TypeError, NAMED_SUBCLASS_ERROR
    assert_equal F::Limits, ForgedLimits.superclass, 'the constant leaks'
  end

  def test_forged_limits_never_reach_the_core
    gains = F::Gains.new(1, 0, 0)
    [ForgedLimits.new(0.0, 1.0), ForgedLimits.new(1.0, 0.0)].each do |forged|
      error = assert_raises(TypeError) { F::Pid.new(gains, forged) }
      assert_equal 'limits must be a FeedbackMachines::Limits', error.message
      assert_raises(TypeError) { F::Hysteresis.new(forged) }
    end
  end

  def test_marshalled_values_are_revalidated
    inverted = Marshal.load(Marshal.dump(F::Limits.new(0.0, 1.0)).sub("minf\x060".b, "minf\x062".b))
    assert_instance_of F::Limits, inverted
    assert_equal [2.0, 1.0], [inverted.min, inverted.max]
    assert_raises(RangeError) { F::Hysteresis.new(inverted) }
    assert_raises(RangeError) { F::Pid.new(F::Gains.new(1, 0, 0), inverted) }
    negative = Marshal.load(Marshal.dump(F::Gains.new(1.0, 0.0, 0.0)).sub("pf\x061".b, "pf\x07-1".b))
    assert_equal(-1.0, negative.p)
    assert_raises(RangeError) { F::Pid.new(negative, F::Limits.new(0, 1)) }
  end

  def test_a_value_with_a_singleton_class_is_still_that_value
    limits = F::Limits.new(0, 1)
    limits.singleton_class
    assert_equal limits, F::Hysteresis.new(limits).limits
    gains = F::Gains.new(1, 0, 0).tap(&:singleton_class)
    assert_equal F::Gains.new(1, 0, 0), F::Pid.new(gains.clone, limits.clone).gains
  end

  # Runs `hook` inside `Limits.new` and `Gains.new` while it is set.
  module ValueHook
    attr_accessor :hook

    def new(...)
      hook&.call
      super
    end
  end
  [F::Gains, F::Limits].each { |value| value.singleton_class.prepend(ValueHook) }

  # No borrow is held while a reader builds its answer: re-entering from
  # `new` steps the controller instead of panicking into `fatal`.
  def test_readers_hold_no_borrow_while_building
    pid = F::Pid.new(F::Gains.new(1, 0, 0), F::Limits.new(0, 1))
    band = F::Hysteresis.new(F::Limits.new(0, 1))
    F::Limits.hook = -> { pid.step(0.5, 1) && band.observe(2.0) }
    F::Gains.hook = -> { pid.step(0.25, 1) }
    assert_equal F::Limits.new(0, 1), pid.limits
    assert_equal F::Limits.new(0, 1), band.limits
    assert_equal F::Gains.new(1, 0, 0), pid.gains
    assert_equal :high, band.state
  ensure
    F::Limits.hook = F::Gains.hook = nil
  end

  def test_redefined_readers_are_ignored
    gains = F::Gains.new(1, 0, 0)
    limits = F::Limits.new(0, 1)
    with_raising_readers(F::Gains, F::Limits) do
      assert_equal 0.5, F::Pid.new(gains, limits).step(0.5, 1)
      assert_equal :low, F::Hysteresis.new(limits).observe(-1.0)
    end
  end

  # Every reader of `classes` raises while the block runs.
  def with_raising_readers(*classes)
    saved = classes.flat_map { |klass| klass.members.map { |name| [klass, name, klass.instance_method(name)] } }
    verbose = $VERBOSE
    $VERBOSE = nil
    saved.each { |klass, name, _| klass.define_method(name) { raise 'the binding ran a reader' } }
    $VERBOSE = verbose
    yield
  ensure
    $VERBOSE = nil
    saved.each { |klass, name, method| klass.define_method(name, method) }
    $VERBOSE = verbose
  end
end
