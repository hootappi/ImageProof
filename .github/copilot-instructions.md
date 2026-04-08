# ImageProof Workspace Setup Checklist

- [x] Verify that the copilot-instructions.md file in the .github directory is created.
- [x] Clarify Project Requirements
- [x] Scaffold the Project
- [x] Customize the Project
- [x] Install Required Extensions
- [x] Compile the Project
- [x] Create and Run Task
- [x] Launch the Project
- [x] Ensure Documentation is Complete

## Execution Progress
Starting workspace setup for ImageProof application.

## Clarified Project Requirements (Baseline v0.1 - 2026-02-23)

### Scope and Objective
- Build a client-side ImageProof verification engine for image authenticity assessment.
- Operate locally by default (browser WASM primary), with optional native client path.
- Produce measurable, explainable outputs grounded in physical and statistical signals.

### Core Functional Requirements
- Detect presence/absence of sensor-like signatures (PRNU plausibility proxy only).
- Detect localized hybrid manipulation in otherwise authentic images.
- Detect fully synthetic/generative images.
- Output composite authenticity confidence score in [0,1].
- Output per-layer contributions and structured reason codes.

### Core Non-Functional Requirements
- No external upload of image data or derived forensic signals.
- No reliance on metadata for core decisioning.
- Target latency budget: <=500ms for standard 12MP image.
- Define and enforce per-layer latency allocations and fast/deep execution modes.
- Tolerate JPEG/WebP compression, resizing, cropping, recompression, metadata removal.

### Trust and Threat Constraints
- Must not claim device identity validation without reference fingerprint.
- Must distinguish authenticity verification vs manipulation detection vs synthetic likelihood.
- Must bound claims and prefer indeterminate over false assertion.
- Must characterize performance and degradation across adversary tiers A-D.

### Scoring and Explainability Constraints
- Use explicit fusion logic for per-layer signals to composite score.
- Define thresholds for Authentic, Suspicious, Synthetic, and Indeterminate.
- Include confidence degradation causes and signal insufficiency flags.
- Maintain explicit reason-code taxonomy with stable identifiers.

### Execution and Deployment Constraints
- Primary environment: browser-based WASM with CPU fallback and GPU acceleration path.
- Hardware tiers: CPU-only, CPU+SIMD, WebGPU, Native hardware-access tier.
- Deployment patterns to support: browser extension, corporate R&D integration, desktop tool, enterprise pipeline integration.

### Validation and Calibration Constraints
- Calibrate for <=1% false positives on authentic images.
- Calibrate for <=10% false negatives on synthetic/hybrid images.
- Document dataset strategy, threshold tuning method, and measurement methodology.

## Project Customization (Baseline v0.1 - 2026-02-23)

### Engineering Structure
- Core domain contracts are split into dedicated modules for model and engine concerns.
- Shared enums now define execution mode, hardware tier, and structured reason codes.
- Verification result schema includes per-layer latency and layer reason grouping.

### Project Conventions
- Added repository editor conventions via `.editorconfig`.
- Maintained small, focused workspace with Rust core and WASM bindings crates only.
- Preserved not-implemented verification engine boundary to keep incremental delivery traceable.

### Development Tooling
- Added workspace extension recommendations in `.vscode/extensions.json` for Rust and TOML development.
- Added workspace task in `.vscode/tasks.json` for `cargo check` and executed it successfully.
- Added runnable scaffold CLI target and launched project via `cargo run -p imageproof-cli`.

### Documentation Completion
- README now documents workspace structure, prerequisites, compile/run commands, and current scaffold behavior.
- Setup checklist is fully completed and synchronized with the repository state.

## Post-Setup Incremental Work

### Web App Shell (2026-02-23)
- Added `web` frontend scaffold (Vite) with local image upload flow.
- Added npm script `build:wasm` to regenerate browser bindings from `crates/wasm-bindings`.
- Verified web build and local dev-server launch path for the scaffold UI.

### Fast-Mode Engine Stub (2026-02-23)
- Updated `imageproof-core` verification engine to return a structured `Indeterminate` scaffold result in `Fast` mode.
- Preserved `Deep` mode as `NotImplemented` to keep incremental implementation boundaries explicit.

### Web Result Formatting (2026-02-23)
- Updated `web/src/main.js` to render friendly verification summaries (classification, score, reason codes, latency).
- Preserved raw JSON output beneath the summary for implementation traceability.

### Web Mode Toggle (2026-02-23)
- Added `Fast`/`Deep` execution mode selector in the web UI.
- Wired mode selection to `verify_image` to validate both scaffold paths from browser flow.

### Web Deep-Mode Hint (2026-02-23)
- Added explicit Deep-mode scaffold message in the web result panel when `NotImplemented` is returned.
- Preserved raw error text for debugging traceability.

### Web Copy Result Control (2026-02-23)
- Added `Copy Result` action in the web result panel.
- Copied text includes summary and raw output, with inline success/failure status feedback.

### Web Run Trace Line (2026-02-23)
- Added last-run timestamp and execution mode trace line at the top of web result output.
- Preserved identical trace visibility for successful and error result paths.

### Web One-Command Check (2026-02-23)
- Added `npm run check` in `web/package.json` to run `build:wasm` followed by `build`.
- Documented one-command web verification flow in README.

### One-Click Web Launcher (2026-02-23)
- Added root `start-web.ps1` script to automate prerequisite checks/install and launch the web app.
- Script runs `npm run check` before starting dev server to ensure current WASM and web build consistency.

### Double-Click Launcher Wrapper (2026-02-23)
- Added root `start-web.cmd` wrapper so web app startup can be launched without manual PowerShell setup.
- Wrapper runs `start-web.ps1` with execution-policy bypass and keeps error output visible on failure.

### Modern Web UI Refresh (2026-02-23)
- Refreshed web UI with dark-blue modern styling and simplified layout.
- Replaced control set with drag-drop upload, in-page image preview, single verify action, and human-readable result fields (`Confidence`, `Justification`).

### Upload Box Behavior Adjustment (2026-02-23)
- Updated upload flow so selected image renders inside the drag-drop box and upload prompt is hidden while loaded.
- Added `Clear` action to remove current image and reset upload state for another file.

### Deep-Only Tri-State Result Flow (2026-02-23)
- Updated verify action to run Deep analysis path only in web flow.
- Result panel now maps output to three human-readable outcomes: real, edited, or more likely AI generated, each with confidence.

### Deep Heuristic Verifier v0 (2026-02-23)
- Replaced checksum placeholder with first measurable deep-analysis heuristic in `imageproof-core`.
- Deep path now decodes image bytes and evaluates noise, edge, and block-artifact metrics to drive tri-state classification and confidence scaffolding.

### Signal Intelligence Layer v1 (2026-02-23)
- Added residual-noise extraction and FFT-based spectral feature scoring in `imageproof-core` Deep analysis path.
- Updated deep scoring to include spectral peak and high-frequency energy ratios for stronger synthetic/edited separation heuristics.

### Physical Intelligence Layer v1 (2026-02-23)
- Added PRNU plausibility proxy scoring from residual block-to-block correlation statistics.
- Added cross-region consistency scoring to detect spatial instability in sensor-like residual patterns.

### Hybrid Manipulation Layer v1 (2026-02-23)
- Added localized residual inconsistency scoring across neighboring image tiles.
- Added seam anomaly scoring from residual discontinuity excess across candidate splice boundaries.
- Updated Deep-layer fusion weights to incorporate hybrid cues for stronger edited-image separation.

### Semantic Intelligence Layer v1 (2026-02-23)
- Added residual-pattern repetition scoring from shifted residual autocorrelation cues.
- Added gradient-orientation entropy scoring to capture collapsed structural diversity patterns.
- Added semantic synthetic-cue fusion in Deep scoring and conditional semantic reason routing for suspicious outcomes.

### Fusion Calibration Scaffold v1 (2026-02-23)
- Added explicit per-layer contribution outputs (`signal`, `physical`, `hybrid`, `semantic`) to verification results.
- Added threshold profile outputs (`synthetic_min`, `synthetic_margin`, `suspicious_min`) aligned with current decision gates.
- Refactored Deep classification thresholds to named constants for stable calibration tuning entry points.

### False-Positive Reduction Tuning v1 (2026-02-23)
- Raised suspicious-classification gate to require stronger edited evidence before labeling authentic photos as edited.
- Rebalanced edited-likelihood fusion weights to reduce over-reliance on global variance/edge cues.
- Added physical-consistency suppression and reduced hybrid cue aggressiveness for better authentic-image tolerance.

### False-Positive Reduction Tuning v2 (2026-02-23)
- Raised synthetic-classification thresholds and margin to require stronger evidence before labeling AI-generated.
- Reduced semantic synthetic-cue aggressiveness and lowered hybrid influence in synthetic fusion.
- Added explicit synthetic suppression based on strong physical consistency and natural high-frequency texture cues.

### Stress Test Harness v1 (2026-02-24)
- Extended `imageproof-cli` with `stress <dataset_root>` mode for dataset-level robustness evaluation.
- Added recursive class-folder evaluation (`authentic`, `edited`, `synthetic`) with per-class and overall accuracy reporting.
- Added perturbation-tag aggregation (resize/crop/recompress/jpeg/webp/lowlight) and decode-failure tracking.

### Acceptance Quality Bar v1 (2026-02-24)
- Added explicit PASS/FAIL quality-bar evaluation in stress-test output.
- Added thresholds for authentic false positives (`<=1%`) and edited/synthetic miss rates (`<=10%`).
- Added minimum sample-size requirement (`>=25` per class) with explicit failure notes.

### Automated Test Suite — C4 (2026-02-24)
- Added 44 unit tests in `imageproof-core` covering every metric function and public API path.
- Added 17 unit tests in `imageproof-cli` covering `is_supported_image`, `derive_perturbation_tags`, `GroupStats`, and acceptance quality functions.
- 2 tests marked `#[ignore]` pending C1 fix (fusion produces NaN on flat/tiny images).
- Created GitHub Actions CI workflow (`.github/workflows/ci.yml`) with `cargo test`, `clippy -D warnings`, and `npm run check`.

### WASM Panic Hook — H7 (2026-02-24)
- Added `console_error_panic_hook` v0.1 dependency to `crates/wasm-bindings`.
- Added `#[wasm_bindgen(start)] fn init()` that installs the panic hook once on WASM module load.
- Rust panics now surface full error messages in browser DevTools console instead of opaque WASM traps.

### Input Size and Dimension Limits — C5 (2026-02-24)
- Added `MAX_FILE_SIZE_BYTES` (50 MB) pre-decode check and `MAX_IMAGE_DIMENSION` (16384) post-decode check.
- Added `InputTooLarge` and `DimensionTooLarge` error variants to `VerifyError` with descriptive messages.
- Both limits enforced in core `verify()` and inherited by WASM bindings without WASM-specific changes.
- Added 6 new unit tests covering exact boundary, over-limit, and ordering verification.

### Fusion Weight Normalization — C1 (2026-02-24)
- Normalized `synthetic_base` weights from sum 1.34 to 1.00 (preserving relative proportions).
- Normalized `edited_base` weights from sum 1.09 to 1.00.
- Normalized `authentic_likelihood` coefficients from sum 1.32 to 1.00.
- Fixed `0/0` NaN in `block_artifact_score` when both `boundary_avg` and `interior_avg` are zero (flat images).
- Un-ignored 2 blocked tests (`verify_valid_png_returns_result`, `verify_3x3_png_returns_result_no_panic`).
- Added 5 regression tests: weight sum verification (3), NaN-free property (1), flat block artifact (1).

### Indeterminate Classification — C3 (2026-02-24)
- Added `INDETERMINATE_CEILING` (0.30) and `INDETERMINATE_MIN_SPREAD` (0.08) classification constants.
- Added Indeterminate branch in Deep classification: fires when both `synthetic_likelihood` and `edited_likelihood` are below the ceiling and their spread is below the minimum, emitting score 0.50 and reason code `SysInsuff001`.
- Classification is now quad-state: Synthetic → Suspicious → Indeterminate → Authentic.
- Added `make_xorshift_png` test helper (xorshift32 PRNG for flat-spectrum white noise).
- Added 6 C3 unit tests: 3 constant-consistency checks, 1 integration (xorshift noise → Indeterminate), 1 score (0.50), 1 reason code (SysInsuff001).
- Updated ARCHITECTURE.md: tri-state → quad-state, resolved markers for C1/C3/C4/C5/H7.

### Real Per-Layer Latency — C2 (2026-02-24)
- Replaced fabricated pixel-count-based latency formula with real `Instant::now()` per-layer wall-clock timing.
- Extracted `compute_pixel_statistics` (pixel-level noise/edge/block-artifact/CV) and `compute_signal_metrics_timed` (per-layer timing wrapper) from monolithic `compute_signal_metrics`.
- Moved `compute_signal_metrics` to `#[cfg(test)]` (only tests use the untimed variant now).
- Updated latency test: validates total < 30s and fusion ≤ signal (real measurement properties).

### JPEG Format Gating — H2 (2026-02-24)
- Added JPEG format detection via `ImageReader::format()` in decode path.
- `block_artifact_score` is now forced to 0.0 for non-JPEG inputs (PNG, WebP, BMP, etc.).
- Threaded `is_jpeg: bool` through `compute_pixel_statistics` and `compute_signal_metrics_timed`.
- Added `make_jpeg` test helper and 3 H2 unit tests (PNG zero, JPEG non-negative, flag unit test).

### Residual Border Exclusion — H4 (2026-02-24)
- `compute_residual_map` now returns `(Vec<f32>, usize, usize)` — interior-only buffer excluding zero-padded border rows/cols.
- All downstream consumers (FFT, PRNU, hybrid, semantic) receive interior dimensions, eliminating border zero contamination.
- Semantic gradient entropy decoupled to use `gray.width()`/`gray.height()` directly instead of residual dimensions.
- Updated 4 existing residual map tests; added 3 new H4 tests (manual value verification, FFT downstream clean, 3×3 edge case).

### Perturbation Tag Fix — H5 (2026-02-24)
- `derive_perturbation_tags` now matches keywords against filename stem only (`Path::file_stem()`), not against the full path string.
- Extensions (`.jpg`, `.jpeg`, `.webp`) and directory components no longer produce spurious perturbation tags.
- Added 5 new H5 tests: 3 extension-exclusion, 1 stem-keyword, 1 directory-ignore.

### Symlink Protection — H6 (2026-02-24)
- `collect_recursive` now uses `entry.file_type().is_symlink()` to detect and skip symlinks with a warning.
- `DirEntry::file_type()` does not follow symlinks (unlike `Path::is_dir()`).
- Added 2 cross-platform unit tests (normal files, nested dirs).
- Added 2 Unix-only symlink integration tests (file symlink, dir symlink).
- Added 1 Windows `#[ignore]` symlink test (requires Developer Mode).

### Feature Backlog Added (2026-02-24)
- **F1: Analysis Progress Indicator** — state-driven progress UI (idle/running/completed/failed), scheduled for M4.
- **F2: Privacy-Preserving Feedback Learning** — post-analysis feedback, local calibration, optional anonymous diagnostic sharing, scheduled for M4.

### Frontend Confidence Fix — H1 (2026-02-25)
- Replaced parabolic Suspicious confidence formula `(1 - abs(0.5 - s) * 2)` with linear `(1 - bounded)` inversion.
- Suspicious confidence is now monotonically decreasing with `authenticity_score`.
- Previous formula peaked at 0.5 and dropped to 0% at both extremes, distorting backend scores.

### Content-Security-Policy — M9 (2026-02-25)
- Added CSP meta tag in `web/index.html` enforcing `default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'none'`.
- Created `web/vercel.json` with matching CSP HTTP header plus `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy` hardening headers.
- No external network requests permitted — aligns with core privacy requirement.

### Authentic Reason Code Fix — M7 (2026-02-25)
- Reason codes are now driven by per-layer contribution scores via `derive_reason_codes()`.
- Added `REASON_CODE_CONTRIBUTION_THRESHOLD` (0.15) constant — only layers above this emit reason codes.
- Authentic and Suspicious branches no longer hardcode `PhyPrnu001` regardless of physical contribution.
- Fallback logic ensures every result has at least one reason code.
- Added 7 new unit tests covering all derive_reason_codes paths and integration verification.

### Web Worker Offload — H8 (2026-02-26)
- Created `web/src/worker.js` Web Worker that loads WASM independently and runs `verify_image` off the main thread.
- Main thread posts image bytes to Worker via `postMessage` with `Transferable` buffer; Worker returns result.
- Falls back to synchronous main-thread execution if Worker initialization fails.
- Updated CSP in `index.html` and `vercel.json`: `connect-src 'self'`, `worker-src 'self'` (still blocks all external requests).

### f64 Accumulation Precision — M5 (2026-02-26)
- Upgraded `compute_shifted_residual_corr` and `block_corr` to use `f64` accumulators for all sum/mean/correlation computations.
- Final correlation result cast to `f32` at output.
- Prevents catastrophic cancellation and precision loss on large images (>10MP).
- All 98 existing tests pass unchanged.

### Indeterminate Recalibration (2026-02-26)
- Tuned `INDETERMINATE_CEILING` from 0.30 to 0.32 and `INDETERMINATE_MIN_SPREAD` from 0.08 to 0.12 for better separation.

### Deduplicate Pixel Iteration — M4 (2026-02-26)
- Added `compute_pixel_stats_and_residual` unified pass in `signal.rs`, computing noise, edge, block metrics, and residual map in a single pixel iteration.
- Eliminated redundant pixel traversal from separate signal-metrics and residual-map functions.

### Format Restriction — L5 (2026-02-26)
- Added explicit format allowlist (JPEG, PNG, WebP) in `decode_image`.
- Unknown/unsupported formats (BMP, GIF, TIFF, etc.) rejected with `UnsupportedFormat` error.
- Added `verify_bmp_rejected_as_unsupported` unit test.

### Remove HardwareTier — L4 (2026-02-26)
- Removed unused `HardwareTier` enum from `model.rs` and `VerifyRequest`.
- Simplified `verify()` and `verify_bytes()` call sites and WASM bindings.

### Per-Layer Module Extraction — M1 (2026-02-26)
- Extracted `signal.rs`, `physical.rs`, `hybrid.rs`, `semantic.rs` from monolithic `engine.rs`.
- Engine orchestrates layers via `pub(crate)` function calls.
- Reduced `engine.rs` from ~2300 to ~1750 lines.
- Made `sample_rect`, `fft2d_magnitude` (signal), `block_corr` (physical) `pub(crate)` for cross-module test access.

### Runtime TOML Config — M3 (2026-02-26)
- Added `CalibrationConfig` struct (93 fields, `#[derive(Deserialize)]`, `#[serde(default)]`) to `config.rs`.
- Implemented `Default for CalibrationConfig` mapping every field to its compile-time constant.
- Added `verify_bytes_with_config(bytes, mode, &CalibrationConfig)` as the primary configurable entry point.
- Threaded `&CalibrationConfig` through all engine functions: `decode_image`, `verify_fast`, `verify_deep_heuristic`, `compute_layer_contributions`, `derive_reason_codes`.
- CLI: added `--config <path.toml>` flag with `parse_config_arg()` and `filter_positional_args()`.
- Added `toml = "0.8"` workspace dependency.
- Created `config.example.toml` reference file with all 93 fields commented out showing defaults.
- Added 6 M3 integration tests (default equivalence, threshold shifting, reason codes, input limits, dimension limits, fast mode).
- Total: 105 tests (81 core + 24 CLI), clippy clean.

### Analysis Progress Indicator — F1 (2026-04-08)
- Added state-driven progress indicator (idle/running/completed/failed) in web UI.
- Real elapsed-time counter updated at 100ms intervals via `performance.now()`.
- Indeterminate pulse animation during analysis, green/red completion states.
- Duplicate-analysis prevention via running-state guard on verify button.

### Privacy-Preserving Feedback Learning — F2 (2026-04-08)
- Added post-analysis feedback UI (correct/incorrect buttons + classification correction selector).
- Feedback persisted in `localStorage` under `imageproof_feedback_log` (rolling 500-entry window).
- Optional opt-in anonymous diagnostic sharing (checkbox, defaults off, persisted).
- Diagnostic payload contains scores, classification, reason codes, timing, feedback only — no image data.
- Privacy model documented in `docs/FEEDBACK_SYSTEM.md` with data schema, storage model, transmission model, and audit checklist.

### Consent Prompt — L1 (2026-04-08)
- `start-web.ps1` now lists required installations and prompts for confirmation before invoking `winget` or `cargo install`.

### Prettier Config — L2 (2026-04-08)
- Added `.prettierrc` config in `web/`.
- Added `npm run format` and `npm run format:check` scripts.
- Installed `prettier` as devDependency; formatted all JS/CSS/HTML files.

### Versioning Strategy — L3 (2026-04-08)
- Added `CONTRIBUTING.md` with SemVer strategy, release checklist, branch naming, conventional commits, and PR requirements.

### WASM Instant Fix (2026-04-08)
- Replaced `std::time::Instant` with `web_time::Instant` in `engine.rs` — `std::time::Instant::now()` traps on `wasm32-unknown-unknown` (no system clock).
- Added `web-time = "1"` crate dependency (uses `performance.now()` in browser, `std::time::Instant` on native).
- All 105 tests pass unchanged on native; WASM runtime panic resolved.

### Color Forensic Layer (2026-04-08)
- Added two new discriminative features for AI-generated image detection:
  - **Color channel noise correlation**: exploits independent per-channel sensor noise in real cameras vs correlated noise in AI generators (shared latent space).
  - **Noise-brightness dependency**: real cameras follow shot-noise physics (variance ∝ brightness); AI noise has no brightness dependency.
- Features feed an additive "color boost" pathway modulated by physical suppression.
- Grayscale images detected and gated (channel_noise_corr=0.0, noise_brightness_corr=0.5 neutral).
- Synthetic classification branch now uses `derive_reason_codes()` for consistency with other branches.
- `CalibrationConfig` expanded from 93→100 fields (7 new color forensic parameters).
- `decode_image` now returns `(GrayImage, RgbImage, is_jpeg)` to support color analysis.
- 105 tests pass, clippy clean.

### False-Positive Reduction — Color Boost Suppression (2026-04-08)
- Moved color boost inside physical suppression: `(synthetic_base + color_boost) * suppression` instead of `synthetic_base * suppression + color_boost`.
- Previous architecture let color cues bypass PRNU/consistency evidence, causing real camera photos to classify as AI-generated.
- Raised `COLOR_SYNTH_GATE` 0.25→0.40, lowered `COLOR_SYNTH_BOOST_SCALE` 1.0→0.45.
- Strengthened suppression weights: `SYN_SUPP_PRNU` 0.16→0.25, `SYN_SUPP_CONSISTENCY` 0.10→0.18, `SYN_SUPP_HF_RATIO` 0.06→0.10, `SYN_SUPP_FLOOR` 0.55→0.40.
- Raised `SYNTHETIC_MIN_THRESHOLD` 0.58→0.62 and `SYNTHETIC_MARGIN_THRESHOLD` 0.10→0.12.
- 105 tests pass, clippy clean.

### GitHub & Vercel Deployment (2026-04-08)
- Created GitHub repo: https://github.com/hootappi/ImageProof
- Deployed to Vercel: https://imageproof.vercel.app
- WASM artifacts committed to git for zero-Rust Vercel builds.

## Open Items (Pending)
- Calibrate color forensic thresholds with real AI-generated image dataset (DALL-E, Midjourney, Stable Diffusion, Flux).
- Stress test algorithm robustness across authentic/edited/synthetic samples and perturbation variants.
- Plan user feedback collection and triage loop for calibration iterations.
