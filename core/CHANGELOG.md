# Changelog

## [0.1.0-alpha.1](https://github.com/hop-top/poly-c12n/compare/c12n-core/v0.1.0-alpha.0...c12n-core/v0.1.0-alpha.1) (2026-08-06)


### ⚠ BREAKING CHANGES

* **core:** `PiiSignal` with empty deny list previously reported no entities, now reports all detected types.

### Features

* **core:** approximate tokenizer ([c94535c](https://github.com/hop-top/poly-c12n/commit/c94535cbc4492e2029bdf87fc248b2c3274e08dc))
* **core:** derive Default on ClassificationContext, Debug on PipelineResult ([c0a3455](https://github.com/hop-top/poly-c12n/commit/c0a345549afa9b94292add84e3fc1e7d643fef64))
* **core:** detector registry for config-driven chain construction ([afc1ed5](https://github.com/hop-top/poly-c12n/commit/afc1ed545cfb076ac3ce2f7401a15346c5795433))
* **core:** heuristic jailbreak detector ([aee5cca](https://github.com/hop-top/poly-c12n/commit/aee5cca1ae7c9d5669b08b5485153eeaac30169c))
* **core:** loud error on evaluate with zero registered signals ([07cf35f](https://github.com/hop-top/poly-c12n/commit/07cf35f8ceef90d8faaf5dd98589fa1caa076e9f))
* **core:** regex PII detector with Luhn validation ([05f7644](https://github.com/hop-top/poly-c12n/commit/05f7644cea6367791fe0ec598d8e53519579cb54))
* **core:** stopword language detector ([485b8f2](https://github.com/hop-top/poly-c12n/commit/485b8f2256a524ae56d6bf4896490f6e44ea4999))
* **core:** tiered detector chains with per-signal strategy ([083cfc9](https://github.com/hop-top/poly-c12n/commit/083cfc91a98aca1a02b898091f8a876aed8ed0de))
* **core:** tiered detector chains, tier-1 detectors, and loud empty-pipeline errors ([f1ed2b5](https://github.com/hop-top/poly-c12n/commit/f1ed2b5500ec1a75eb2758fa9225ef80401cad31))


### Bug Fixes

* **core:** drop tokio time feature on wasm32, add per-target rt shims ([0efd9fb](https://github.com/hop-top/poly-c12n/commit/0efd9fb6b50f8e161e4ca3647ce9b409f154d2b3))
* **core:** drop tokio time feature on wasm32, add per-target rt shims ([5a72c67](https://github.com/hop-top/poly-c12n/commit/5a72c6781d2e8747e7c6d5675afb87a50821865f))
* **core:** empty PII deny list reports all entities, not none ([068ef78](https://github.com/hop-top/poly-c12n/commit/068ef7895f2536c8fc48103ff1758ee54462f2e0))
* **rs:** re-export signals + async_trait so documented quickstart compiles ([e0616f0](https://github.com/hop-top/poly-c12n/commit/e0616f0193ac78cd1b06ca8640adcf0b9007576d))

## 0.1.0-alpha.0 (2026-07-29)


### Miscellaneous

* initial public release ([87ef174](https://github.com/hop-top/poly-c12n/commit/87ef1745bbf979f6cb3f5a77c9e70ee7bfd429f4))
