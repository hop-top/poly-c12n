# Changelog

All notable changes to c12n (Go, Rust core, Python bindings) are
documented here. Components are released as linked versions — bumps
apply across `c12n`, `c12n-core`, and `c12n-py` together.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
via [release-please](https://github.com/googleapis/release-please).

Tag prefixes (linked):

- `c12n/v*` — Go module (`hop.top/c12n`)
- `c12n-core/v*` — Rust crate (`c12n-core`)
- `c12n-py/v*` — Python package (`c12n` on PyPI)

See `.github/release-please-config.json` for full release plumbing.

## [0.1.0-alpha.1](https://github.com/hop-top/poly-c12n/compare/c12n-poly/v0.1.0-alpha.0...c12n-poly/v0.1.0-alpha.1) (2026-08-06)


### ⚠ BREAKING CHANGES

* **go:** `PipelineError` is now a string type, not a struct. Consumers reading `.SignalFailed` / `.Timeout` must read the message string instead.

### Features

* **core:** tiered detector chains with per-signal strategy ([083cfc9](https://github.com/hop-top/poly-c12n/commit/083cfc91a98aca1a02b898091f8a876aed8ed0de))
* **core:** tiered detector chains, tier-1 detectors, and loud empty-pipeline errors ([f1ed2b5](https://github.com/hop-top/poly-c12n/commit/f1ed2b5500ec1a75eb2758fa9225ef80401cad31))


### Bug Fixes

* **build:** sync Cargo.lock to 0.1.0-alpha.0 crate versions ([8e3a04b](https://github.com/hop-top/poly-c12n/commit/8e3a04b335dd6c53d99ba68afb85810a705e4a9c))
* **build:** sync Cargo.lock to 0.1.0-alpha.0 crate versions ([d8180f9](https://github.com/hop-top/poly-c12n/commit/d8180f9849d2e7fdaafe96705f077f071e50f121))
* **ci,ts:** install pyyaml for py publish tests, fix nodejs wasm module cast ([ead58cc](https://github.com/hop-top/poly-c12n/commit/ead58cc3c7407de78e7e97c5bc9f4ae5d5b83df3))
* **ci:** build py wheel inside official maturin manylinux2014 container ([c299f90](https://github.com/hop-top/poly-c12n/commit/c299f90558e5ec6a53a10fe1b47f1040e57b9a57))
* **ci:** build py wheel with manylinux2014 compatibility tag ([6f3d15d](https://github.com/hop-top/poly-c12n/commit/6f3d15dfd1daad0ebef6e9640b99187047dc6133))
* **ci:** exclude c12n-poly tags from publish trigger, install pytest-asyncio for py tests ([3dd0449](https://github.com/hop-top/poly-c12n/commit/3dd0449efd41ae60bfd917985e16e1b8b1b554f0))
* **ci:** install wasm-pack before ts build, pin core dep version for rs publish ([15b0695](https://github.com/hop-top/poly-c12n/commit/15b0695b3f3663523add1d3a460bd1a487747330))
* **core:** drop tokio time feature on wasm32, add per-target rt shims ([0efd9fb](https://github.com/hop-top/poly-c12n/commit/0efd9fb6b50f8e161e4ca3647ce9b409f154d2b3))
* **core:** drop tokio time feature on wasm32, add per-target rt shims ([5a72c67](https://github.com/hop-top/poly-c12n/commit/5a72c6781d2e8747e7c6d5675afb87a50821865f))
* **go:** decode `errors` as strings to match core wire format ([419e932](https://github.com/hop-top/poly-c12n/commit/419e93210abf59e2013e177a7523ec375d22d5d3))
* **go:** gate native engine behind opt-in c12n_native tag ([#33](https://github.com/hop-top/poly-c12n/issues/33)) ([1e47a97](https://github.com/hop-top/poly-c12n/commit/1e47a979cd20b3f13e1dcead691b4027a13872b4))

## 0.1.0-alpha.0 (2026-07-29)


### Miscellaneous

* initial public release ([87ef174](https://github.com/hop-top/poly-c12n/commit/87ef1745bbf979f6cb3f5a77c9e70ee7bfd429f4))

## [Unreleased]

Stub entry — first published tags will populate this section
automatically via release-please.
