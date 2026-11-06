# frozen_string_literal: true
require_relative 'feedback_machines/version'
require 'feedback_machines_native/feedback_machines_native'

module FeedbackMachines
  class Pid
    def self.new(p:, min:, max:, i: 0.0, d: 0.0) = _new(p, i, d, min, max)
    private_class_method :_new
    def step(error:, elapsed:) = _step(error, elapsed)
    private :_step
  end

  class Hysteresis
    def self.new(min:, max:) = _new(min, max)
    private_class_method :_new
  end
end
