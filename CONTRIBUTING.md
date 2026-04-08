# Contributing to ImageProof

## Versioning Strategy

ImageProof follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** — incompatible API changes (e.g., `VerificationResult` field removal, classification enum changes)
- **MINOR** — backward-compatible feature additions (e.g., new reason codes, new layer metrics)
- **PATCH** — backward-compatible bug fixes and calibration adjustments

### Current Version

The canonical version lives in the workspace root `Cargo.toml` and is reflected
in `web/package.json`. Both must be updated together.

### Releasing a New Version

1. Update `version` in root `Cargo.toml` and all crate `Cargo.toml` files.
2. Update `version` in `web/package.json`.
3. Add a dated section to `docs/CHANGELOG.md`.
4. Commit with message `release: vX.Y.Z`.
5. Tag the commit: `git tag vX.Y.Z`.
6. Push the tag: `git push origin vX.Y.Z`.

### Pre-release Versions

Use `-alpha.N` or `-beta.N` suffixes for pre-release builds (e.g., `0.2.0-alpha.1`).

## Development Workflow

### Prerequisites

- Rust toolchain (stable) via [rustup](https://rustup.rs/)
- Node.js LTS (>= 18)
- wasm-pack (`cargo install wasm-pack`)

### Build & Test

```bash
# Rust tests (core + CLI)
cargo test

# Clippy lint check
cargo clippy -- -D warnings

# Full web build (WASM + Vite)
cd web && npm run check

# Format JS/CSS
cd web && npm run format
```

### Branch Naming

- `main` — protected, CI-gated
- `harden/<id>` — hardening sprint fixes (e.g., `harden/c1-fusion-normalization`)
- `feature/<name>` — new features (e.g., `feature/f1-progress-indicator`)
- `fix/<description>` — bug fixes

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` new feature
- `fix:` bug fix
- `refactor:` code restructuring (no behavior change)
- `docs:` documentation only
- `test:` test additions or corrections
- `chore:` build, CI, dependency updates
- `release:` version bump

### Pull Request Requirements

1. CI must pass (cargo test + clippy + npm check).
2. At least one approval from a reviewer.
3. CHANGELOG.md updated if user-facing.
4. No regression in stress-test accuracy (if applicable).
