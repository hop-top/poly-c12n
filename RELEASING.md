# Releasing c12n

c12n ships **seven components from one repo**, each independently versioned via [`release-please`](https://github.com/googleapis/release-please).

## Use this when

- You're landing a `release-please` PR and want to know what happens next.
- A `publish.yml` job failed and you need to understand the chain.
- You need to force a specific version (e.g. to skip a buggy alpha).

## The seven components

| Component | Path | Registry | Tag prefix |
|---|---|---|---|
| `c12n-poly` | `.` (root) | — (umbrella) | `c12n-poly/v*` |
| `c12n-core` | `core/` | crates.io `hop-top-c12n-core` | `c12n-core/v*` |
| `c12n-rs` | `rs/` | crates.io `hop-top-c12n` | `c12n-rs/v*` |
| `c12n` (Go) | `go/` | `hop.top/c12n` via proxy.golang.org | `c12n/v*` |
| `c12n-py` | `py/` | PyPI `hop-top-c12n` | `c12n-py/v*` |
| `c12n-ts` | `ts/` | npm `@hop-top/c12n` | `c12n-ts/v*` |
| `c12n-php` | `php/` | Packagist `hop-top/c12n` | `c12n-php/v*` |

Each component is configured independently in `.github/release-please-config.json`. Versions are not synchronized.

## Result

After cutting a release, you'll have:

- A `<component>/v<version>` tag on `main`.
- A GitHub Release with auto-generated notes.
- Source mirrored to the component's standalone repo (e.g. `hop-top/c12n-php`).
- A registry artifact (crate / module / wheel / npm package / composer package) at the new version.
- For the Go CLI tag (`c12n/v*`): a `goreleaser` build with multi-arch binaries on the release page + Homebrew tap + Scoop bucket + WinGet manifest update.

## Quick version

```bash
# 1. Open a release-please PR for the component you want to ship.
# (release-please creates these automatically on every push to main.)
gh pr list --label "status:release-pending"

# 2. Verify CI is green on the PR.
gh pr checks <pr-number>

# 3. Verify the manifest delta + CHANGELOG diff match expectation.
gh pr diff <pr-number>

# 4. Merge (squash).
gh pr merge <pr-number> --squash --delete-branch

# Tag pushes automatically; publish.yml fans out to the registry.
```

## Steps

### 1. Verify the release-please PR

When you push a conventional commit touching e.g. `core/`, `release-please` opens (or updates) a PR titled `chore(release): c12n-core <version>`. The PR contains:

- `core/CHANGELOG.md` — new section with the commits this release ships.
- `core/Cargo.toml` — `version` bumped.
- `.github/.release-please-manifest.json` — `core` entry updated.

If the PR is missing a commit you expect, check:

- The commit message uses a valid Conventional Commits type (`feat`, `fix`, etc.).
- The commit touched files inside the component's path (e.g. a commit only touching `go/` won't appear in a `c12n-core` release).
- The commit isn't a release PR itself (release-please skips its own commits).

### 2. Verify CI

The polyglot CI (`.github/workflows/ci.yml`) runs on every PR including release-please PRs. It must be green before merging. The matrix covers:

- Rust core × 3 OS
- Go (stub + cgo) × 3 OS each
- Python bindings × 3 OS
- TypeScript bindings × 3 OS
- PHP bindings × 2 OS (PHP 8.4 on ubuntu + macOS)

### 3. Merge the release-please PR

```bash
gh pr merge <pr-number> --squash --delete-branch
```

This commits to `main` and pushes the `<component>/v<version>` tag.

### 4. Watch publish.yml

The tag push triggers `.github/workflows/publish.yml`, which calls the reusable workflow `hop-top/.github/.github/workflows/publish-on-tag.yml@v0`.

For each ecosystem:

| Ecosystem | What happens |
|---|---|
| `rs` (cargo) | `cargo publish` to crates.io with `CARGO_REGISTRY_TOKEN` |
| `py` | `maturin publish` to PyPI with `PYPI_REGISTRY_TOKEN` (token auth) |
| `ts` (npm) | `pnpm publish` with `NPM_REGISTRY_TOKEN` |
| `php` | Notify Packagist via `update-package` API with `PACKAGIST_USERNAME` + `PACKAGIST_TOKEN` |
| `go` | nothing — proxy.golang.org indexes tags from `hop-top/c12n` (mirror) automatically |

In parallel, the subtree-mirror job pushes the component's directory to its standalone mirror repo (`hop-top/c12n-core`, `hop-top/c12n-php`, etc.) using `GH_MIRROR_PAT`.

### 5. (Go CLI only) goreleaser

When the tag is `c12n/v*` (the bare Go CLI), `publish.yml`'s `goreleaser` job fires the reusable workflow `goreleaser-on-tag.yml@v0`. It:

- Builds multi-arch Linux/macOS/Windows binaries.
- Uploads them to the GitHub release.
- Updates the `hop-top/homebrew-tap` formula.
- Updates the `hop-top/scoop-bucket` manifest.
- Opens a PR against `microsoft/winget-pkgs` (gated on stable releases; prerelease tags skip WinGet).

## Override: Release-As

Sometimes you need a specific version (e.g. to skip a buggy alpha or to align with a downstream pinning). Drop a `Release-As: x.y.z-alpha.N` footer on a `chore:` commit touching the component:

```bash
git commit --allow-empty -m "chore(core): force version (T-NNNN)

Release-As: 0.1.0-alpha.0"
```

release-please's next run will propose exactly that version.

## Common issues

| Symptom | Likely cause | Fix |
|---|---|---|
| Publish job reports "success" but registry has no new version | Reusable workflow doesn't handle the ecosystem | Check `hop-top/.github/.github/workflows/publish-on-tag.yml` has a `publish-<ecosystem>` job |
| Packagist still shows `dev-main` after merging a PHP release-please PR | `PACKAGIST_USERNAME` / `PACKAGIST_TOKEN` not passed in `publish.yml` `secrets:` block, OR kit-php's `composer.json` requires alpha-stability deps that consumer hasn't allowed | Verify `secrets:` passthrough; consumer must declare `"minimum-stability": "alpha"` |
| `cargo publish` fails with "crate name already taken" | First-publish issue — crates.io requires reservation | Reserve the name first via `cargo owner --add` (one-time per crate) |
| Mirror repo push fails | `GH_MIRROR_PAT` lacks `Administration + Contents: write` on the mirror | Rotate the PAT with correct scopes |
| Release-please PR version is `0.0.0-alpha.N+1` when you want `0.x.y-alpha.0` | Prerelease versioning bumps only the prerelease counter unless overridden | Use a `Release-As: ` footer to jump |
| goreleaser job didn't fire for the Go CLI tag | Tag didn't match the `if: startsWith(github.ref_name, 'c12n/v')` filter | The bare `c12n/` prefix excludes `c12n-*/`; verify the tag is `c12n/v...` not `c12n-rs/v...` |

## How it works

The full chain on a `c12n-php/v0.0.0-alpha.1` merge:

1. release-please PR squash-merged to `main`.
2. release-please-action tags the merge commit as `c12n-php/v0.0.0-alpha.1` and creates a GitHub Release.
3. The tag push fires `publish.yml`.
4. `publish.yml` calls the reusable workflow with `ecosystem: php`, `mirror: hop-top/c12n-php`, etc. for that component.
5. The reusable workflow's `mirror` job pushes `php/` subtree to `hop-top/c12n-php@v0.0.0-alpha.1`.
6. The `publish-php` job calls `https://packagist.org/api/update-package` with the mirror's repo URL.
7. Packagist polls `hop-top/c12n-php`, sees the new tag, indexes the new `composer.json`.
8. `composer require hop-top/c12n:^0.0.0-alpha` from a downstream project resolves the new version.

## Options

| Action | When to use |
|---|---|
| Merge release-please PR | Standard release — all CI green, content reviewed |
| `Release-As:` footer | You need an explicit version (skip, jump, or fix a wrong prior release) |
| Manually create tag | Almost never — bypasses release-please's manifest/changelog tracking; only for emergency rollback |
| Re-run failed publish job | Transient registry / network failure — `gh run rerun <run-id>` |

## Reference

- `.github/release-please-config.json` — per-component config
- `.github/.release-please-manifest.json` — current-version source of truth
- `.github/workflows/publish.yml` — this repo's tag-push handler
- [`hop-top/.github`](https://github.com/hop-top/.github) — reusable workflows (`publish-on-tag.yml`, `goreleaser-on-tag.yml`, `mirror-subtree.yml`)
