# Changelog

## [0.1.0-alpha.2](https://github.com/hop-top/poly-c12n/compare/c12n/v0.1.0-alpha.1...c12n/v0.1.0-alpha.2) (2026-08-06)


### ⚠ BREAKING CHANGES

* **go:** `PipelineError` is now a string type, not a struct. Consumers reading `.SignalFailed` / `.Timeout` must read the message string instead.
* **go:** exported field `PipelineResult.DurationNs` renamed to `DurationMs`; unit is milliseconds.

### Bug Fixes

* **cli:** write schema-typed scalars in `config set` ([411d521](https://github.com/hop-top/poly-c12n/commit/411d5217ce00e7634992b1a9967351fec911e624))
* **cli:** write schema-typed scalars in `config set` ([688a0e5](https://github.com/hop-top/poly-c12n/commit/688a0e5103a403ec66c0a4aead7b3914e8f0e6ea))
* **go:** decode `errors` as strings to match core wire format ([419e932](https://github.com/hop-top/poly-c12n/commit/419e93210abf59e2013e177a7523ec375d22d5d3))
* **go:** drop duplicate global flag registrations that panicked CLI at startup ([51acc5b](https://github.com/hop-top/poly-c12n/commit/51acc5b2c0bc6da4f6996903888b94846b7017a8))
* **go:** drop duplicate global flag registrations that panicked CLI at startup ([ce531c0](https://github.com/hop-top/poly-c12n/commit/ce531c096e063ec062a2fc5762c0123086bdd071))
* **go:** parse `duration_ms` from core, not `duration_ns` ([e1f79e1](https://github.com/hop-top/poly-c12n/commit/e1f79e1565e14025d7c18f0984a83ef4548c1338))
* **go:** satisfy kit cli conformance so binary starts ([1235f10](https://github.com/hop-top/poly-c12n/commit/1235f1007fde0922659b58dc4fe38f08055413cf))
* **go:** satisfy kit cli conformance so binary starts ([28a99f1](https://github.com/hop-top/poly-c12n/commit/28a99f1f59d78379cbb47438f3b45af67094bae7))

## [0.1.0-alpha.1](https://github.com/hop-top/poly-c12n/compare/c12n/v0.1.0-alpha.0...c12n/v0.1.0-alpha.1) (2026-07-29)


### Bug Fixes

* **go:** gate native engine behind opt-in c12n_native tag ([#33](https://github.com/hop-top/poly-c12n/issues/33)) ([1e47a97](https://github.com/hop-top/poly-c12n/commit/1e47a979cd20b3f13e1dcead691b4027a13872b4))

## 0.1.0-alpha.0 (2026-07-29)


### Miscellaneous

* initial public release ([87ef174](https://github.com/hop-top/poly-c12n/commit/87ef1745bbf979f6cb3f5a77c9e70ee7bfd429f4))
