# Changelog — ImageProof

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased] — Hardening Sprint (Complete)

### Done

- **M1 (arch)**: Extracted per-layer analysis modules — `signal.rs`, `physical.rs`, `hybrid.rs`, `semantic.rs` — from monolithic `engine.rs`. Engine orchestrates layers via `pub(crate)` calls. Reduced `engine.rs` from ~2300 to ~1750 lines.
- **M3 (config)**: Added `CalibrationConfig` struct (100 fields, `#[serde(default)]`) for runtime threshold overrides via TOML. New `verify_bytes_with_config()` threads config through all engine functions. CLI accepts `--config path.toml`. Created `config.example.toml` reference with all defaults commented out. Added 6 M3 integration tests (default equivalence, threshold shifting, reason codes, input limits, dimension limits, fast mode).
- **M4 (dup iter)**: Added `compute_pixel_stats_and_residual` unified pass that computes noise, edge, block metrics and residual map in a single pixel iteration, eliminating duplicate traversal.
- **M8 (fast)**: Fast mode now runs pixel-level statistics and returns a structured result with per-layer contributions, score mapping, and reason codes using shared `CalibrationConfig` plumbing.
- **L4**: Removed unused `HardwareTier` enum from `model.rs` and `VerifyRequest`.
- **L5**: Added explicit format allowlist (JPEG, PNG, WebP) in `decode_image`; unknown formats rejected with `UnsupportedFormat` error.
- **F1**: Added state-driven analysis progress indicator (idle/running/completed/failed) in web UI. Real elapsed-time counter. Indeterminate pulse animation during analysis, green/red completion states. Duplicate-analysis prevention via running-state guard.
- **F2**: Added post-analysis feedback UI — correct/incorrect buttons with classification correction selector. Feedback persisted in `localStorage` (rolling 500 entries). Optional opt-in anonymous diagnostic sharing (no image data — only scores, classification, reason codes, timing, feedback). Privacy model documented in `docs/FEEDBACK_SYSTEM.md`.
- **L1**: Added consent prompt in `start-web.ps1` — lists required installations and asks for confirmation before invoking `winget` or `cargo install`.
- **L2**: Added `.prettierrc` config + `npm run format` / `npm run format:check` scripts. Formatted all JS/CSS/HTML files.
- **L3**: Added `CONTRIBUTING.md` with semantic versioning strategy, release checklist, branch naming, commit conventions, and PR requirements.
- **Indeterminate recalibration**: Tuned `INDETERMINATE_CEILING` (0.30→0.32) and `INDETERMINATE_MIN_SPREAD` (0.08→0.12) for better separation.
- **C4**: Added automated test suite — 44 unit tests in `imageproof-core`, 17 in `imageproof-cli` (59 total, 2 ignored pending C1). Added GitHub Actions CI workflow (`cargo test`, `clippy`, `npm run check`).
- **H7**: Added `console_error_panic_hook` to WASM bindings. `#[wasm_bindgen(start)]` init installs the hook so Rust panics surface readable stack traces in browser DevTools.
- **C5**: Added input size limit (`MAX_FILE_SIZE_BYTES` = 50 MB) enforced before decode and dimension limit (`MAX_IMAGE_DIMENSION` = 16384) enforced after decode. New error variants `InputTooLarge` and `DimensionTooLarge` with descriptive messages propagated through WASM.
- **C1**: Normalized all fusion weights to sum = 1.0 (synthetic_base was 1.34, edited_base was 1.09, authentic_likelihood coefficients were 1.32). Fixed `0/0` NaN in `block_artifact_score` for flat images. Un-ignored 2 blocked tests. Added 5 regression tests (weight sums, NaN-free property, flat block artifact).
- **C3**: Added Indeterminate classification branch — when both `synthetic_likelihood` and `edited_likelihood` fall below `INDETERMINATE_CEILING` (0.30) with spread below `INDETERMINATE_MIN_SPREAD` (0.08), the engine now emits `Indeterminate` (score 0.50, reason `SysInsuff001`) instead of defaulting to Authentic. Added `make_xorshift_png` test helper and 6 C3 tests. Updated ARCHITECTURE.md to quad-state classification.
- **C2**: Replaced fabricated pixel-count-based latency formula with real `Instant::now()` per-layer wall-clock timing. Extracted `compute_pixel_statistics` and `compute_signal_metrics_timed` functions. Moved `compute_signal_metrics` to `#[cfg(test)]`. Updated latency test to validate real measurement properties.
- **H2**: Added JPEG format detection in decode path. Block artifact scoring (`block_artifact_score`) is now forced to 0.0 for non-JPEG inputs. Added `make_jpeg` test helper and 3 H2 unit tests.
- **H4**: `compute_residual_map` now returns interior-only buffer `(width-2) × (height-2)` excluding zero-padded border rows/cols. All downstream consumers (FFT, PRNU, hybrid, semantic) receive clean residuals. Gradient entropy in semantic layer decoupled to use `gray.width()`/`gray.height()` directly. Updated 4 existing tests, added 3 new H4 tests.
- **H5**: `derive_perturbation_tags` now matches keywords against filename stem only (`Path::file_stem()`), not against the extension or directory path components. Plain `.jpg`/`.jpeg`/`.webp` files no longer receive spurious perturbation tags. Added 5 new H5 tests (extension exclusion, stem keyword, directory ignore).
- **H6**: `collect_recursive` now uses `entry.file_type().is_symlink()` (does not follow symlinks) to detect and skip symlinks with a warning to stderr. Added 2 cross-platform unit tests + 2 Unix-only symlink integration tests + 1 Windows-ignored symlink test.
- **H1**: Replaced parabolic Suspicious confidence formula `(1 - abs(0.5 - s) * 2)` with linear `(1 - bounded)` inversion in `web/src/main.js`. Confidence is now monotonically decreasing with `authenticity_score` for Suspicious classification.
- **M9**: Added Content-Security-Policy meta tag in `web/index.html` and Vercel HTTP header config in `web/vercel.json`. Policy enforces `default-src 'none'`, `connect-src 'none'`, `script-src 'self' 'wasm-unsafe-eval'` — no external network requests permitted. Additional hardening headers: `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`, `Permissions-Policy` (camera/mic/geo/payment denied).
- **M7**: Reason codes are now driven by per-layer contribution scores above `REASON_CODE_CONTRIBUTION_THRESHOLD` (0.15). Authentic and Suspicious branches emit only codes for layers that actually contributed. Added `derive_reason_codes` helper, `REASON_CODE_CONTRIBUTION_THRESHOLD` constant, and 7 new unit tests.
- **H8**: Moved `verify_image` WASM call to a dedicated Web Worker (`web/src/worker.js`). Main thread posts image bytes via `postMessage` with `Transferable` buffer, Worker loads WASM independently and returns result. Falls back to synchronous main-thread execution if Worker initialization fails. Updated CSP: `connect-src 'self'`, `worker-src 'self'` (still blocks all external network requests).
- **M5**: `compute_shifted_residual_corr` and `block_corr` now use `f64` accumulators for all sum/mean/numerator/denominator computations. Final correlation result is cast to `f32` at return. Prevents catastrophic cancellation and precision loss on large images (>10MP). All 98 existing tests pass unchanged.
- **WASM Instant Fix**: Replaced `std::time::Instant` with `web_time::Instant` in `engine.rs` — `std::time::Instant::now()` traps on `wasm32-unknown-unknown`. Added `web-time = "1"` crate dependency.
- **Color Forensic Layer**: Added color channel noise correlation and noise-brightness dependency features for AI-generated image detection. `decode_image` now returns `(GrayImage, RgbImage, is_jpeg)`. `CalibrationConfig` expanded from 93→100 fields.
- **False-Positive Reduction — Color Boost Suppression**: Moved color boost inside physical suppression: `(synthetic_base + color_boost) * suppression`. Raised `COLOR_SYNTH_GATE` 0.25→0.40, lowered `COLOR_SYNTH_BOOST_SCALE` 1.0→0.45. Strengthened suppression weights. Raised `SYNTHETIC_MIN_THRESHOLD` to 0.62 and `SYNTHETIC_MARGIN_THRESHOLD` to 0.12.
- **GitHub & Vercel Deployment**: Public repo at https://github.com/hootappi/ImageProof. Production at https://imageproof.vercel.app. WASM artifacts committed for zero-Rust Vercel builds.
- **Web UI**: Added "alpha" badge in headline and development footer with commit hash and author attribution.

### Known Issues

- Calibration thresholds are educated defaults — not yet validated against a real multi-class dataset.
- Hand-tuned linear fusion weights plateau on accuracy; logistic regression planned (M5 Phase 3).
- No JPEG-specific forensic features yet (quantization tables, DCT distributions).
- Structured logging (`tracing` crate) not yet integrated.
- No `Cargo.lock` audit policy enforced.

### Development Path (M5)

- **Phase 1**: Assemble calibration dataset (≥125 images), measure baseline accuracy, tune thresholds against data.
- **Phase 2**: Add JPEG quantization table analysis, DCT coefficient distribution, richer color features, GAN/diffusion spectral fingerprint.
- **Phase 3**: Train logistic regression on all features, cross-validation framework, ROC-optimized thresholds.

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

All critical and high-priority issues identified in the v0.1.0 code review have been resolved in the Hardening Sprint:

- Fusion weights exceed 1.0 (C1) — **RESOLVED**
- Latency values are fabricated, not measured (C2) — **RESOLVED**
- Indeterminate classification is never emitted (C3) — **RESOLVED**
- Zero automated tests (C4) — **RESOLVED** (105 tests)
- No input size/dimension limits (C5) — **RESOLVED**

See `docs/EXECUTION_PLAN.md` for the complete findings list and resolution details.
