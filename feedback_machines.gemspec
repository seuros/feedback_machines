# frozen_string_literal: true
require_relative 'lib/feedback_machines/version'
Gem::Specification.new do |spec|
  spec.name = 'feedback_machines'
  spec.version = FeedbackMachines::VERSION
  spec.authors = ['Abdelkader Boudih']
  spec.email = ['oss@seuros.com']
  spec.summary = 'Native PID and hysteresis controllers for Ruby'
  spec.description = 'Thin Ruby bindings to the standalone feedback-machines core; ' \
                     'sampling, tuning, and actuation belong to the caller.'
  spec.homepage = 'https://github.com/seuros/feedback_machines'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 4.0'
  spec.metadata['source_code_uri'] = spec.homepage
  spec.metadata['rubygems_mfa_required'] = 'true'
  spec.metadata['cargo_crate_name'] = 'feedback_machines_native'
  spec.metadata['cargo_manifest_path'] = 'ext/feedback_machines_native/ffi/Cargo.toml'
  spec.files = Dir.glob(%w[
    lib/**/*.rb sig/**/*.rbs ext/**/*.{rb,rs,toml,md} Cargo.toml
    Cargo.lock README.md LICENSE.txt CHANGELOG.md
  ]).select { |path| File.file?(path) }
  spec.require_paths = ['lib']
  spec.extensions = ['ext/feedback_machines_native/extconf.rb']
  spec.add_dependency 'rb_sys', '~> 0.9', '>= 0.9.130'
end
