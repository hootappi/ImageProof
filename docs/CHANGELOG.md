# Changelog — ImageProof

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased] — Hardening Sprint

### Done

- **C4**: Added automated test suite — 44 unit tests in `imageproof-core`, 17 in `imageproof-cli` (59 total, 2 ignored pending C1). Added GitHub Actions CI workflow (`cargo test`, `clippy`, `npm run check`).
- **H7**: Added `console_error_panic_hook` to WASM bindings. `#[wasm_bindgen(start)]` init installs the hook so Rust panics surface readable stack traces in browser DevTools.
- **C5**: Added input size limit (`MAX_FILE_SIZE_BYTES` = 50 MB) enforced before decode and dimension limit (`MAX_IMAGE_DIMENSION` = 16384) enforced after decode. New error variants `InputTooLarge` and `DimensionTooLarge` with descriptive messages propagated through WASM.
- **C1**: Normalized all fusion weights to sum = 1.0 (synthetic_base was 1.34, edited_base was 1.09, authentic_likelihood coefficients were 1.32). Fixed `0/0` NaN in `block_artifact_score` for flat images. Un-ignored 2 blocked tests. Added 5 regression tests (weight sums, NaN-free property, flat block artifact).
- **C3**: Added Indeterminate classification branch — when both `synthetic_likelihood` and `edited_likelihood` fall below `INDETERMINATE_CEILING` (0.30) with spread below `INDETERMINATE_MIN_SPREAD` (0.08), the engine now emits `Indeterminate` (score 0.50, reason `SysInsuff001`) instead of defaulting to Authentic. Added `make_xorshift_png` test helper and 6 C3 tests. Updated ARCHITECTURE.md to quad-state classification.
- **C2**: Replaced fabricated pixel-count-based latency formula with real `Instant::now()` per-layer wall-clock timing. Extracted `compute_pixel_statistics` and `compute_signal_metrics_timed` functions. Moved `compute_signal_metrics` to `#[cfg(test)]`. Updated latency test to validate real measurement properties.
- **H2**: Added JPEG format detection in decode path. Block artifact scoring (`block_artifact_score`) is now forced to 0.0 for non-JPEG inputs. Added `make_jpeg` test helper and 3 H2 unit tests.
- **H4**: `compute_residual_map` now returns interior-only buffer `(width-2) × (height-2)` excluding zero-padded border rows/cols. All downstream consumers (FFT, PRNU, hybrid, semantic) receive clean residuals. Gradient entropy in semantic layer decoupled to use `gray.width()`/`gray.height()` directly. Updated 4 existing tests, added 3 new H4 tests.
- **H5**: `derive_perturbation_tags` now matches keywords against filename stem only (`Path::file_stem()`), not against the extension or directory path components. Plain `.jpg`/`.jpeg`/`.webp` files no longer receive spurious perturbation tags. Added 5 new H5 tests (extension exclusion, stem keyword, directory ignore).

### Planned

Full hardening pass driven by adversarial code review (2026-02-24). Scope:

- **Critical**: Normalize fusion weights to sound mathematical model (C1)
- **Critical**: Replace fabricated latency with real measurement or remove (C2)
- **Critical**: Implement Indeterminate classification for low-confidence inputs (C3)
- **Critical**: Add automated test suite — unit, integration, property (C4)
- **Critical**: Enforce input size and dimension limits (C5)
- **High**: Fix frontend confidence formula distortion (H1)
- **High**: Add WASM panic hook for debuggable errors (H7)
- **High**: Move WASM verification to Web Worker (H8)
- **High**: Skip JPEG-specific block artifact scoring for non-JPEG inputs (H2)
- **High**: Fix perturbation tag derivation in stress test (H5)
- **Medium**: Add symlink protection in CLI file traversal (H6)
- **Medium**: Add CI pipeline with cargo test + clippy + npm check (M12)
- **Medium**: Refactor monolithic engine into per-layer modules (M1)
- **Low**: Add Content-Security-Policy for deployment (M9)

See `docs/EXECUTION_PLAN.md` for full backlog, sequencing, and acceptance criteria.

## [0.1.0] — 2026-02-24

### Added

- Signal Intelligence v1: noise residual, edge, block artifact, FFT spectral features.
- Physical Intelligence v1: PRNU plausibility proxy, cross-region consistency.
- Hybrid Manipulation v1: localized residual inconsistency, seam anomaly.
- Semantic Intelligence v1: residual pattern repetition, gradient-entropy scoring.
- Fusion Calibration Scaffold v1: per-layer contribution scores, threshold profiles.
- False-Positive Reduction Tuning v1 + v2 for authentic camera photo tolerance.
- Stress-test harness with recursive dataset evaluation and perturbation tagging.
- Acceptance quality bar (≤1% authentic FP, ≤10% edited/synthetic miss, ≥25 samples).
- Web UI: drag-drop upload, image preview, verify/clear actions, confidence + justification display.
- WASM bindings via wasm-pack for browser integration.
- CLI scaffold with launch validation and stress-test mode.
- One-click launcher scripts (`start-web.ps1`, `start-web.cmd`).

### Known Issues at Release

- Fusion weights exceed 1.0 (C1) — **RESOLVED**: All fusion weight sets normalized to sum = 1.0; NaN from 0/0 fixed.
- Latency values are fabricated, not measured (C2).
- Indeterminate classification is never emitted (C3).
- Zero automated tests (C4) — **RESOLVED**: 59 tests + CI pipeline added.
- No input size/dimension limits (C5) — **RESOLVED**: 50 MB file size + 16384 dimension limits enforced.
- See `docs/EXECUTION_PLAN.md` for complete findings list.
