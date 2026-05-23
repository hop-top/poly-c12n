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

## [0.0.0-alpha.1](https://github.com/hop-top/poly-c12n/compare/c12n-poly/v0.0.0-alpha.0...c12n-poly/v0.0.0-alpha.1) (2026-05-23)


### Features

* **c12n-core:** add wasm feature + parallel wasm.rs module for #[wasm_bindgen] types (T-0120) ([e4f8472](https://github.com/hop-top/poly-c12n/commit/e4f847204c0200538d83fae1746542ce65b217ce))
* **c12n-php:** scaffold PHP FFI bindings + kit-php logging (T-0136, T-0137, T-0138, T-0139, T-0152) ([7765c00](https://github.com/hop-top/poly-c12n/commit/7765c00fdd6568dc9c55fcfa448f251976221b84))
* **c12n-ts:** scaffold WASM-based TypeScript bindings + kit-ts logging (T-0117, T-0119, T-0121, T-0151) ([741c3ab](https://github.com/hop-top/poly-c12n/commit/741c3ab6b50da4cf13c420958ad90f7d4a30d24b))
* initial c12n publish — polyglot Rust core + Go + Python bindings ([458f34d](https://github.com/hop-top/poly-c12n/commit/458f34dc3a9b5d000a21c8acb1813e1cd3af1d48))
* **release:** add WinGet distribution for c12n CLI (prerelease-gated) ([3b88037](https://github.com/hop-top/poly-c12n/commit/3b8803758b5ddc257e9135b434b80a6c37fd1499))
* **release:** initial polyglot c12n publish — Rust core + Go/Python/TS/PHP bindings + CLI ([167a0af](https://github.com/hop-top/poly-c12n/commit/167a0af556b1e521521f9bc80e2d4a323431fc8e))


### Bug Fixes

* **core:** wasm32 Pipeline::new panic — use instant crate for time-on-wasm (T-0186) ([c02e72d](https://github.com/hop-top/poly-c12n/commit/c02e72d51299b6cb251ccf5aa87f4ec527dde6ba))


### Refactoring

* **release:** adopt poly-c12n config + package renames ([4b41049](https://github.com/hop-top/poly-c12n/commit/4b41049cd57a827cf27617d90bd44fd1a34d5657))
* **release:** centralize binary distribution via org-wide tap/bucket ([80babf7](https://github.com/hop-top/poly-c12n/commit/80babf79cec309051a9c61af59f7feb67fd54b4d))

## [Unreleased]

Stub entry — first published tags will populate this section
automatically via release-please.
