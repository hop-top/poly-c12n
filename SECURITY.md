# Security policy

## Reporting a vulnerability

**Do not open a public issue.**

Report via [GitHub Private Security Advisory](https://github.com/hop-top/poly-c12n/security/advisories/new) (preferred) or email **security@hop.top**.

Include:

- A description of the vulnerability and the component(s) affected (`core/`, `go/`, `py/`, `ts/`, `php/`, or `rs/`).
- Steps to reproduce.
- Affected version(s) — c12n ships per-component versions; specify which (e.g. `hop-top-c12n-core 0.0.0-alpha.1`, `@hop-top/c12n 0.0.0-alpha.1`).
- Suggested fix or workaround if you have one.

## Response timeline

| Stage | Target |
|---|---|
| Acknowledgement | 48 hours |
| Initial assessment | 1 week |
| Fix or mitigation | best effort within 30 days |

## Supported versions

c12n is **alpha** (v0.x). Only the latest published version of each component is supported:

| Component | Registry | Supported |
|---|---|---|
| `hop-top-c12n-core` | crates.io | latest |
| `hop-top-c12n` (Rust SDK) | crates.io | latest |
| `hop.top/c12n` (Go) | proxy.golang.org | latest |
| `hop-top-c12n` (Python) | PyPI | latest |
| `@hop-top/c12n` (TypeScript) | npm | latest |
| `hop-top/c12n` (PHP) | Packagist | latest |

Stable v1.x will publish a longer support matrix.

## Threat model

c12n is a **classifier**, not an executor. It reads LLM request inputs and emits labels + confidence scores. It does not:

- Make network requests on behalf of callers.
- Invoke tools or run code from the classified content.
- Store request content beyond the lifetime of a single `Pipeline.evaluate()` call.

**Trust boundaries** to keep in mind when reporting:

- **FFI surface** (`core/src/ffi.rs`): accepts untrusted JSON from C ABI callers. Each `extern "C"` function is marked `unsafe` and validates pointer arguments. Memory-safety bugs in the JSON parser, signal evaluator, or wasm-bindgen layer are in scope.
- **PHP `Installer`** (`php/src/Installer.php`): downloads `libc12n_core` at `composer install` time. The download URL is hardcoded to `github.com/hop-top/poly-c12n/releases/`. Issues with the manifest verification (`manifest.json` SHA256) or download URL substitution are in scope.
- **TypeScript wasm loader** (`ts/src/wasm-loader.ts`): loads the wasm binary from the package's own `pkg/` dir. Path-traversal or import-shadowing issues are in scope.

**Out of scope**:

- LLM model output quality, prompt injection, or jailbreak resistance — c12n classifies requests *before* they reach a model; it doesn't see model output.
- Denial-of-service via crafted inputs that take a long time to classify. Mitigated by `Pipeline`'s per-evaluation timeout, configurable by the caller.
- Security of the LLM provider you're routing to.

## Disclosure policy

Coordinated disclosure. Once a fix is available we publish a [GitHub Security Advisory](https://github.com/hop-top/poly-c12n/security/advisories) and credit the reporter (unless anonymity is requested). For severe issues we pre-notify downstream binding mirror repos (`hop-top/c12n-{core,rs,go,py,ts,php}`).
