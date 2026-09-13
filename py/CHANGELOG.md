# Changelog

## [0.1.0-alpha.1](https://github.com/hop-top/poly-c12n/compare/c12n-py/v0.1.0-alpha.0...c12n-py/v0.1.0-alpha.1) (2026-09-13)


### ⚠ BREAKING CHANGES

* **go:** exported field `PipelineResult.DurationNs` renamed to `DurationMs`; unit is milliseconds.

### Bug Fixes

* **go:** parse `duration_ms` from core, not `duration_ns` ([e1f79e1](https://github.com/hop-top/poly-c12n/commit/e1f79e1565e14025d7c18f0984a83ef4548c1338))
* **py:** accept flat config schema, reject unknown keys ([#55](https://github.com/hop-top/poly-c12n/issues/55)) ([e79246d](https://github.com/hop-top/poly-c12n/commit/e79246d6d71e6414b3dfc9372f717e425795afd8))

## 0.1.0-alpha.0 (2026-07-29)


### Miscellaneous

* initial public release ([87ef174](https://github.com/hop-top/poly-c12n/commit/87ef1745bbf979f6cb3f5a77c9e70ee7bfd429f4))
