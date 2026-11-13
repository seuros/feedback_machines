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
  # Stored as Floats. Output limits for a Pid, thresholds for a Hysteresis.
  Limits = Data.define(:min, :max) do
    def initialize(min:, max:)
      min, max = FeedbackMachines.__send__(:__limits, min, max)
      super
    end
  end
end

# Native is required: there is no pure-Ruby fallback. It validates and builds
# the Data values above.
require 'feedback_machines_native/feedback_machines_native'

module FeedbackMachines
  private_class_method :__gains, :__limits
end
