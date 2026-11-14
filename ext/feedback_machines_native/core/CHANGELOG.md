# Changelog

## [0.5.0](https://github.com/seuros/feedback_machines/compare/feedback-machines-v0.4.0...feedback-machines-v0.5.0) (2026-11-14)


### ⚠ BREAKING CHANGES

* build the binding on matryona-machines 0.2; read Gains and Limits from the struct; compiler-checked symbols; ship no test files

### Features

* build the binding on matryona-machines 0.2; read Gains and Limits from the struct; compiler-checked symbols; ship no test files ([19023a5](https://github.com/seuros/feedback_machines/commit/19023a591da9a54ecb606eab51a9cc035eccf7a7))

## [0.4.0](https://github.com/seuros/feedback_machines/compare/feedback-machines-v0.3.0...feedback-machines-v0.4.0) (2026-11-14)


### ⚠ BREAKING CHANGES

* seal Gains and Limits, re-validate them at the binding, hold no borrow while Limits.new runs

### Features

* seal Gains and Limits, re-validate them at the binding, hold no borrow while Limits.new runs ([8aee9ef](https://github.com/seuros/feedback_machines/commit/8aee9efe086af88530c672abd235f92f64d4fb32))

## [0.3.0](https://github.com/seuros/feedback_machines/compare/feedback-machines-v0.2.2...feedback-machines-v0.3.0) (2026-11-13)


### ⚠ BREAKING CHANGES

* converted to the fleet conventions. One exhaustive Refusal per crate, Result from every operation that can refuse, a refusal changes nothing, provable idempotent repeats are honoured, Ruby mirrors the core. No compatibility aliases. step and observe return Result; Ruby takes Gains and Limits values.

### Features

* refuse bad samples with Refusal and take elapsed as Integer nanoseconds in Ruby ([cc23741](https://github.com/seuros/feedback_machines/commit/cc23741e0f1309717910b8752b14e6bbef1dcf13))

## [0.2.2](https://github.com/seuros/feedback_machines/compare/feedback-machines-v0.2.1...feedback-machines-v0.2.2) (2026-11-06)


### Bug Fixes

* crates.io keywords and categories for the core ([fa5503e](https://github.com/seuros/feedback_machines/commit/fa5503e226b815de6da68bf3710f3b108c27ce2c))

## [0.2.1](https://github.com/seuros/feedback_machines/compare/feedback-machines-v0.2.0...feedback-machines-v0.2.1) (2026-11-06)


### Bug Fixes

* keep test sources out of the packaged gem and crate ([d507afd](https://github.com/seuros/feedback_machines/commit/d507afd8ea54ec317a9b3dccbeb0cb4e3d455f6f))

## [0.2.0](https://github.com/seuros/feedback_machines/compare/feedback-machines-v0.1.0...feedback-machines-v0.2.0) (2026-11-06)


### Features

* initial extraction ([d273a1e](https://github.com/seuros/feedback_machines/commit/d273a1e1a17979e275ded3d02618e316e45d34f3))
