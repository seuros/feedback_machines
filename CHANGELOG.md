# Changelog

## [0.2.0](https://github.com/seuros/feedback_machines/compare/feedback_machines/v0.1.4...feedback_machines/v0.2.0) (2026-11-13)


### ⚠ BREAKING CHANGES

* converted to the fleet conventions. One exhaustive Refusal per crate, Result from every operation that can refuse, a refusal changes nothing, provable idempotent repeats are honoured, Ruby mirrors the core. No compatibility aliases. step and observe return Result; Ruby takes Gains and Limits values.

### Features

* refuse bad samples with Refusal and take elapsed as Integer nanoseconds in Ruby ([cc23741](https://github.com/seuros/feedback_machines/commit/cc23741e0f1309717910b8752b14e6bbef1dcf13))

## [0.1.4](https://github.com/seuros/feedback_machines/compare/feedback_machines/v0.1.3...feedback_machines/v0.1.4) (2026-11-06)


### Bug Fixes

* crates.io keywords and categories for the core ([fa5503e](https://github.com/seuros/feedback_machines/commit/fa5503e226b815de6da68bf3710f3b108c27ce2c))

## [0.1.3](https://github.com/seuros/feedback_machines/compare/feedback_machines/v0.1.2...feedback_machines/v0.1.3) (2026-11-06)


### Bug Fixes

* keep test sources out of the packaged gem and crate ([d507afd](https://github.com/seuros/feedback_machines/commit/d507afd8ea54ec317a9b3dccbeb0cb4e3d455f6f))

## [0.1.2](https://github.com/seuros/feedback_machines/compare/feedback_machines/v0.1.1...feedback_machines/v0.1.2) (2026-11-06)


### Bug Fixes

* cross builds no longer compile for the host ([c724a14](https://github.com/seuros/feedback_machines/commit/c724a1461d0e5e0b8a710ce12222a14f1c1f4013))

## [0.1.1](https://github.com/seuros/feedback_machines/compare/feedback_machines-v0.1.0...feedback_machines/v0.1.1) (2026-11-06)


### Features

* initial extraction ([d273a1e](https://github.com/seuros/feedback_machines/commit/d273a1e1a17979e275ded3d02618e316e45d34f3))
