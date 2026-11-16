# frozen_string_literal: true

require 'minitest/autorun'
require 'open3'
require 'rbconfig'

# The classes of the values the extension wraps (Pid, Hysteresis) are fixed
# when it loads. Nothing a caller does later, redefining a constant or
# `Object.const_get`, reaches a controller being built. Each case runs in a
# fresh process: the class is fixed once per process, at load.
class ClassLookupTest < Minitest::Test
  LIB = File.expand_path('../lib', __dir__)

  PRELUDE = <<~RUBY
    require 'feedback_machines'
    F = FeedbackMachines
    Pid = F::Pid
    Hysteresis = F::Hysteresis
    gains = F::Gains.new(1.0, 0.0, 0.0)
    limits = F::Limits.new(-10.0, 10.0)
  RUBY

  # Runs `body` after PRELUDE in a new Ruby; answers its last line of output.
  def isolated(body)
    out, err, status = Open3.capture3(RbConfig.ruby, '-I', LIB, '-e', PRELUDE + body)
    assert status.success?, "exited #{status.exitstatus}: #{err}"
    out.lines.last&.chomp
  end

  # Bug: a Pid constant redefined before the first controller made it an
  # instance of the new class, which has none of the controller's methods.
  def test_a_redefined_pid_constant_does_not_change_the_class
    assert_equal 'true true 5.0 0.0', isolated(<<~RUBY)
      F.send(:remove_const, :Pid)
      F.const_set(:Pid, Class.new)
      pid = Pid.new(gains, limits)
      restored = Pid.restored(gains, limits, 0.0, nil)
      puts [pid.instance_of?(Pid), restored.instance_of?(Pid), pid.step(5.0, 1), restored.integral].join(' ')
    RUBY
  end

  # Bug: the same for a Hysteresis.
  def test_a_redefined_hysteresis_constant_does_not_change_the_class
    assert_equal 'true true high high', isolated(<<~RUBY)
      F.send(:remove_const, :Hysteresis)
      F.const_set(:Hysteresis, Class.new)
      band = Hysteresis.new(limits)
      restored = Hysteresis.restored(limits, :high)
      puts [band.instance_of?(Hysteresis), restored.instance_of?(Hysteresis),
            band.observe(20.0), restored.state].join(' ')
    RUBY
  end

  # Bug: building the first controller ran `Object.const_get`, Ruby code in
  # the middle of a native call. No Ruby runs while one is built now.
  def test_building_a_controller_runs_no_const_get
    assert_equal '0 Pid Hysteresis', isolated(<<~RUBY)
      hits = 0
      Object.define_singleton_method(:const_get) do |*args|
        hits += 1
        super(*args)
      end
      pid = Pid.new(gains, limits)
      band = Hysteresis.new(limits)
      puts [hits, pid.class.name.split('::').last, band.class.name.split('::').last].join(' ')
    RUBY
  end

  # Bug: a redefined `Object.const_get` that raised hit an `unwrap` in the
  # extension: an uncatchable fatal, and the process exited.
  def test_a_raising_const_get_does_not_kill_the_process
    assert_equal '5.0 high', isolated(<<~RUBY)
      Object.define_singleton_method(:const_get) { |*| raise IOError, 'lookup ran in a native call' }
      pid = Pid.new(gains, limits)
      band = Hysteresis.new(limits)
      puts [pid.step(5.0, 1), band.observe(20.0)].join(' ')
    RUBY
  end
end
