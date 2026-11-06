# frozen_string_literal: true
require 'bundler/gem_tasks'
require 'rb_sys/extensiontask'
spec = Gem::Specification.load('feedback_machines.gemspec')
RbSys::ExtensionTask.new('feedback_machines_native', spec) do |extension|
  extension.lib_dir = 'lib/feedback_machines_native'
  extension.tmp_dir = 'tmp/rb_sys'
end
desc 'Compile and run native Ruby boundary tests'
task test: :compile do
  sh 'padded_cell'
end
desc 'Test the standalone Rust core'
task :cargo do
  sh 'cargo', 'test', '-p', 'feedback-machines'
end
desc 'Validate Ruby signatures'
task :rbs do
  sh 'rbs', '-I', 'sig', 'validate'
end
task default: %i[test cargo rbs]

Dir[File.join(__dir__, 'tasks/*.rake')].sort.each { |f| load f }
