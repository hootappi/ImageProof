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

## Open Items (Pending)
- Stress test algorithm robustness across authentic/edited/synthetic samples and perturbation variants.
- Prepare Vercel deployment path for browser/WASM app delivery.
- Plan user feedback collection and triage loop for calibration iterations.
