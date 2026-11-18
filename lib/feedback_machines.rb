# frozen_string_literal: true
require_relative 'feedback_machines/version'

module FeedbackMachines
  # The core's `Gains`: proportional, integral and derivative coefficients.
  # Like `Gains::new`, it holds only finite, nonnegative values: anything else
  # raises RangeError (TypeError for a non-number). Stored as Floats.
  Gains = Data.define(:p, :i, :d) do
    def initialize(p:, i:, d:)
      p, i, d = FeedbackMachines.__send__(:__gains, p, i, d)
      super
    end
  end

  # The core's `Limits`: a finite range with min < max, as `Limits::new`
  # requires: anything else raises RangeError (TypeError for a non-number).
  # Stored as Floats. Output limits for a Pid.
  Limits = Data.define(:min, :max) do
    def initialize(min:, max:)
      min, max = FeedbackMachines.__send__(:__limits, min, max)
      super
    end
  end

  # The core's `Thresholds`: a Hysteresis band, finite with low <= high, as
  # `Thresholds::new` requires: anything else raises RangeError (TypeError for
  # a non-number). Equal thresholds switch on the threshold itself. Stored as
  # Floats.
  Thresholds = Data.define(:low, :high) do
    def initialize(low:, high:)
      low, high = FeedbackMachines.__send__(:__thresholds, low, high)
      super
    end
  end
end

# Native is required: there is no pure-Ruby fallback. It validates and builds
# the Data values above.
require 'feedback_machines_native/feedback_machines_native'

module FeedbackMachines
  private_class_method :__gains, :__limits, :__thresholds

  # The values are final. `initialize` is where each validates, and a subclass
  # could override it, or `allocate` build an instance that never ran it:
  # subclassing raises TypeError and `allocate` is gone. This stops accidents,
  # not reflection, so the binding also checks every value it reads.
  [Gains, Limits, Thresholds].each do |value|
    value.singleton_class.undef_method(:allocate)
    value.define_singleton_method(:inherited) { |_| raise TypeError, "#{value} cannot be subclassed" }
  end

  # The controllers wrap native state and have no allocator: a subclass's
  # `new` would build the base class anyway, so subclassing raises too.
  [Pid, Hysteresis, Dwell].each do |controller|
    controller.define_singleton_method(:inherited) { |_| raise TypeError, "#{controller} cannot be subclassed" }
  end
end
