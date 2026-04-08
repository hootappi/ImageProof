# Execution Plan — ImageProof Hardening

> Created: 2026-02-24 | Source: Adversarial code review findings
> Status: **COMPLETE** — all 29 findings resolved, 105 tests passing, CI pipeline active

---

## Objectives

**"Fixed" means:**

1. Every Critical finding is resolved and verified by automated test.
2. Every High finding is resolved or has an explicitly accepted risk with documented workaround.
3. CI pipeline enforces `cargo test`, `cargo clippy`, and `npm run check` on every push.
4. The system can emit Indeterminate when evidence is insufficient.
5. Fusion model is mathematically sound (weights sum to 1.0 or use a principled aggregation).
6. Stress test passes the acceptance quality bar on a ≥25-sample-per-class dataset.

---

## Workstreams

| ID | Workstream | Scope |
|----|-----------|-------|
| **WS-S** | Security | Input validation, symlink protection, panic handling, CSP |
| **WS-R** | Reliability | Fusion normalization, Indeterminate path, latency truth, edge cases |
| **WS-A** | Architecture | Layer abstraction, duplicate elimination, config extraction |
| **WS-O** | Observability | Panic hook, real timing, structured logging |
| **WS-P** | Performance | Web Worker, buffer copy elimination, FFT window improvements |
| **WS-DX** | Developer Experience | CI pipeline, test suite, linting, perturbation tag fix |

---

## Sequencing Rationale

1. **CI + test harness first (M0)** — everything else must be verifiable.
2. **Input validation next (M1)** — reduces blast radius for all downstream work. Prevents OOM/crash.
3. **Scoring model fix (M1)** — all accuracy tuning depends on a sound mathematical base.
4. **Indeterminate path (M1)** — trust constraint; must exist before any production use.
5. **Frontend + UX fixes (M2)** — depends on stable backend contract.
6. **Architecture refactor (M3)** — lower urgency, higher effort, benefits future work.

---

## Milestones

### M0 — Guardrails (Est. 2–3 days)

Establish automated quality gates so all subsequent changes are verifiable.

| Item | Deliverable |
|------|------------|
| CI pipeline | GitHub Actions workflow: `cargo test`, `cargo clippy -- -D warnings`, `npm run check` |
| Unit test scaffold | `#[cfg(test)]` modules in `engine.rs` and `model.rs` with ≥10 initial tests |
| WASM panic hook | `console_error_panic_hook` installed in `wasm-bindings` |
| Clippy clean | Zero warnings on `cargo clippy` |

**Exit criteria**: CI passes green on main branch. At least one test per public function in core.

### M1 — Critical Fixes (Est. 4–6 days)

Resolve all Critical findings and the most impactful High findings.

| Item | Deliverable |
|------|------------|
| Input limits | `MAX_IMAGE_DIMENSION` (16384) and `MAX_FILE_SIZE_BYTES` (50 MB) enforced before decode |
| Fusion normalization | Weights normalized to sum ≤1.0; all existing thresholds recalibrated |
| Indeterminate classification | Emitted when `max(synthetic_likelihood, edited_likelihood) < INDETERMINATE_CEILING` and confidence spread is low |
| Latency truth | `std::time::Instant` per-layer measurement (native), `Performance.now()` wrapper (WASM), OR remove `latency_ms` field |
| JPEG-only block scoring | Check decoded image format; skip `block_artifact_score` for non-JPEG |
| Perturbation tag fix | Match only filename stem (not extension or full path) for perturbation keywords |

**Exit criteria**: Stress test passes quality bar. All Critical tests green. Indeterminate emitted on ≤2×2 images.

### M2 — High-Priority Hardening (Est. 3–5 days)

| Item | Deliverable |
|------|------------|
| Frontend confidence fix | Replace parabolic Suspicious formula with linear `(1 - authenticity_score)` inversion |
| Web Worker | Move `verify_image` call to a Web Worker; main thread stays responsive |
| Symlink protection | Skip symlinks in `collect_recursive`, log skipped paths |
| Authentic reason code fix | Emit reason codes based on actual layer contributions, not hard-coded PhyPrnu001 |
| Residual border fix | Exclude border-zero pixels from all downstream consumers, or use mirror-padded residual |
| CSP headers | Add meta tag in `index.html` and `vercel.json` header config |

**Exit criteria**: UI does not freeze on 12MP image. Symlink test passes. Frontend confidence is monotonic.

### M3 — Architecture + Performance (Est. 5–8 days)

| Item | Deliverable |
|------|------------|
| Layer trait abstraction | `trait AnalysisLayer { fn analyze(&self, gray: &GrayImage) -> LayerOutput; }` with per-layer modules |
| Deduplicate pixel iteration | Single residual-map computation reused by signal metrics; eliminate `compute_signal_metrics` duplicate loop |
| FFT window scaling | Increase FFT cap to `min(dim, 256)` with configurable ceiling |
| Buffer copy elimination | Change `VerifyRequest.image_bytes` to accept `&[u8]` via lifetime or Cow |
| Runtime config | Load thresholds from optional TOML or environment variables |
| f64 accumulation | Use f64 accumulators in correlation functions, cast to f32 at output |
| HardwareTier usage | Either wire `HardwareTier` to conditional SIMD/GPU paths or remove the enum |
| Fast mode | Either implement a lightweight fast path or remove from public API |

**Exit criteria**: Layer modules compile and test independently. Stress test accuracy unchanged (regression gate). WASM binary size delta < 10%.

### M4 — Feature Backlog (Est. 5–10 days)

New features to implement after all planned fixes (M0–M3) are complete.

| Item | Deliverable |
|------|------------|
| F1: Analysis Progress Indicator | Visual state-driven progress indicator (idle/running/completed/failed) in web UI. Driven by real analysis state, not artificial timing. Prevents duplicate analysis. UI remains responsive. |
| F2: Privacy-Preserving Feedback Learning | Post-analysis feedback UI (correct/incorrect + classification correction), local calibration storage, optional opt-in anonymous diagnostic sharing. Strictly no image data leaves device. Modular: feedback UI, capture logic, local storage, diagnostic generator, optional transmitter. |

**Exit criteria (F1)**: Progress indicator visible on analysis start; transitions correctly to completed/failed; no UI freeze; automated UI state tests.

**Exit criteria (F2)**: Feedback capture works with sharing disabled; local calibration stores non-image data; opt-in sharing transmits only derived features + feedback labels; privacy audit passes (no image data in any transmission path); full documentation of privacy model, data schema, local storage model, and transmission model.

---

## Risks and Dependencies

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| Fusion normalization changes all classification thresholds | HIGH | HIGH | Recalibrate using stress-test dataset immediately after normalization. Gate M1 exit on quality bar pass. |
| Indeterminate gate triggers too aggressively | MEDIUM | MEDIUM | Start with conservative (narrow) indeterminate band; widen based on FP data. |
| Web Worker introduces timing/UX regressions | LOW | MEDIUM | Feature-flag the Worker path; fall back to main-thread if Worker init fails. |
| No calibration dataset available yet | HIGH | CRITICAL | M1 cannot exit without ≥25 images per class. Assembling this dataset is a hard dependency. |
| Architecture refactor (M3) breaks WASM build | MEDIUM | MEDIUM | Maintain `npm run check` in CI; run on every PR. |

**ASSUMPTION**: A calibration dataset with ≥25 authentic, ≥25 edited, and ≥25 synthetic images will be assembled before M1 exit.

---

## Definition of Done (DoD)

A finding is "Done" when:

1. Code change is merged to main via reviewed PR.
2. At least one automated test directly validates the fix.
3. CI passes green (cargo test + clippy + npm check).
4. Relevant documentation (ARCHITECTURE.md, SECURITY.md) is updated.
5. CHANGELOG.md entry added.
6. No regression in stress-test accuracy (if applicable).

---

## Backlog

| ID | Severity | Finding | Proposed Fix | Files/Modules | Effort | Owner | Deps | Acceptance Criteria | Verification |
|----|----------|---------|-------------|---------------|--------|-------|------|-------------------|-------------|
| C1 | Critical | Fusion weights sum >1.0 — unstable scoring | **DONE** — Normalized synthetic_base (1.34→1.00), edited_base (1.09→1.00), authentic_likelihood (1.32→1.00). Fixed 0/0 NaN in block_artifact_score. Un-ignored 2 tests. Added 5 regression tests. | `crates/core/src/engine.rs` | M | Backend | C4 | Weights verified to sum ≤1.0 in test ✓; no NaN on flat images ✓ | Unit test asserting weight sums ✓ |
| C2 | Critical | Latency reporting is fabricated | **DONE** — Replaced fabricated formula with real `Instant::now()` per-layer wall-clock timing. Extracted `compute_pixel_statistics` and `compute_signal_metrics_timed`. Updated latency test to validate real measurements. | `crates/core/src/engine.rs` | S | Backend | — | `latency_ms` values correspond to actual wall-clock time ✓; old fabricated formula removed ✓ | Unit test: total < 30s, fusion ≤ signal ✓ |
| C3 | Critical | Indeterminate classification is dead code | **DONE** — Added Indeterminate branch with `INDETERMINATE_CEILING` (0.30) and `INDETERMINATE_MIN_SPREAD` (0.08). Xorshift white-noise image triggers Indeterminate (score 0.50, SysInsuff001). Added `make_xorshift_png` helper and 6 C3 tests. Updated ARCHITECTURE.md to quad-state. | `crates/core/src/engine.rs` | S | Backend | C1 | Xorshift noise image → Indeterminate ✓; constants consistent with higher thresholds ✓ | Unit tests ✓ |
| C4 | Critical | Zero automated tests | **DONE** — 44 core + 17 CLI unit tests (59 total, 2 ignored pending C1). CI workflow added. | `crates/core/src/engine.rs`, `crates/cli/src/main.rs`, `.github/workflows/ci.yml` | L | Backend + Frontend | — | ≥30 tests passing ✅ (59); coverage on every public function | `cargo test` in CI ✅ |
| C5 | Critical | Unbounded memory from large images | **DONE** — Added `MAX_FILE_SIZE_BYTES` (50 MB) pre-decode + `MAX_IMAGE_DIMENSION` (16384) post-decode guards. New error variants `InputTooLarge`, `DimensionTooLarge`. 6 new tests. | `crates/core/src/engine.rs` | S | Backend | — | 50 MB+ file rejected ✓; dimension limit enforced ✓ | Unit tests ✓ |
| H1 | High | Frontend confidence distorts backend scores | **DONE** — Replaced parabolic Suspicious formula `(1 - abs(0.5 - s) * 2)` with linear `(1 - bounded)` inversion. Confidence is now monotonically decreasing with `authenticity_score`. | `web/src/main.js` | S | Frontend | — | Suspicious confidence is monotonically decreasing with authenticity_score ✓ | Web build passes ✓ |
| H2 | High | Block artifact scoring assumes JPEG 8×8 | **DONE** — Detect format via `ImageReader::format()`; `block_artifact_score` forced to 0.0 when `!is_jpeg`. Threaded `is_jpeg` through `compute_pixel_statistics` and `compute_signal_metrics_timed`. Added `make_jpeg` helper and 3 unit tests. | `crates/core/src/engine.rs` | S | Backend | — | `block_artifact_score` is 0.0 for PNG input ✓ | Unit tests ✓ |
| H3 | High | FFT limited to 64×64 samples | **DONE** — FFT window cap raised from 64 to 256 (`FFT_WINDOW_CAP`). Cap is runtime-configurable via `CalibrationConfig.fft_window_cap`. Window is `min(dim, cap)` so images ≥128px use ≥128-sample FFT. | `crates/core/src/signal.rs`, `config.rs` | S | Backend | — | FFT window ≥128 for ≥128px images ✓; configurable ceiling ✓ | Unit tests pass ✓ |
| H4 | High | Residual map border zeros contaminate metrics | **DONE** — `compute_residual_map` now returns `(Vec<f32>, usize, usize)` interior-only buffer excluding border rows/cols. Downstream FFT/PRNU/hybrid/semantic consumers receive clean dimensions. Semantic gradient loop decoupled to use `gray.width()`/`gray.height()`. 4 existing tests updated, 3 new H4 tests. | `crates/core/src/engine.rs` | S | Backend | — | Interior-only residual verified ✓; no border zeros in downstream ✓ | Unit tests ✓ |
| H5 | High | Perturbation tagging matches file extensions | **DONE** — `derive_perturbation_tags` now uses `Path::file_stem()` to match keywords against the filename stem only. Extensions and directory components are excluded. Added 5 new H5 tests (3 extension-exclusion, 1 stem-keyword, 1 directory-ignore). | `crates/cli/src/main.rs` | S | Backend | — | `photo.jpg` produces no jpeg tag ✓; `photo_recompressed_jpeg80.jpg` gets tag ✓ | Unit tests ✓ |
| H6 | High | CLI follows symlinks without boundary check | **DONE** — `collect_recursive` now uses `entry.file_type().is_symlink()` to detect and skip symlinks with a warning. The `file_type()` method does not follow symlinks (unlike `Path::is_dir()`). Added 2 cross-platform tests, 2 Unix-only symlink integration tests, 1 Windows `#[ignore]` symlink test. | `crates/cli/src/main.rs` | S | Backend | — | Symlink skipped with warning ✓ | Unit + integration tests ✓ |
| H7 | High | No WASM panic handler | **DONE** — Added `console_error_panic_hook` dep + `#[wasm_bindgen(start)] fn init()` that calls `set_once()`. | `crates/wasm-bindings/src/lib.rs`, `crates/wasm-bindings/Cargo.toml` | S | Backend | — | WASM panic produces readable message in browser console ✓ | Manual verification; WASM integration test |
| H8 | High | Synchronous main-thread WASM execution | **DONE** — Created `web/src/worker.js` Web Worker that loads WASM and runs `verify_image` off-main-thread. Main thread posts image bytes via `postMessage`/`transferable`, Worker returns result. Falls back to synchronous main-thread execution if Worker init fails. Updated CSP to add `worker-src 'self'` and `connect-src 'self'`. | `web/src/main.js`, new `web/src/worker.js`, `web/index.html`, `web/vercel.json` | M | Frontend | — | UI thread remains responsive during verification ✓; fallback works if Worker unavailable ✓ | Manual test; CSP verified ✓ |
| M1 | Medium | Monolithic engine (858 lines, no layer abstraction) | **DONE** — Extracted per-layer modules: `signal.rs`, `physical.rs`, `hybrid.rs`, `semantic.rs`. Engine orchestrates layers via `pub(crate)` function calls. `engine.rs` reduced from ~2300 to ~1750 lines. All 105 tests pass, clippy clean. | `crates/core/src/signal.rs`, `physical.rs`, `hybrid.rs`, `semantic.rs`, `engine.rs` | L | Backend | C1, C4 | Layers compile and test independently ✓; engine.rs reduced ✓ | All 105 tests pass ✓; clippy clean ✓ |
| M2 | Medium | ~40 undocumented magic numbers | **DONE** — All 93 numeric constants extracted to `config.rs` with doc comments grouped by category (classification, fusion, suppression, score mapping, layer contributions, per-layer params). Engine references named constants. | `crates/core/src/config.rs` | M | Backend | M1 | Every fusion/metric literal replaced with named constant ✓ | Code review ✓ |
| M3 | Medium | No runtime configuration | **DONE** — Added `CalibrationConfig` struct (93 fields, `#[serde(default)]`) in `config.rs`. `verify_bytes_with_config()` threads config through all engine functions. CLI accepts `--config path.toml` via TOML deserialization. Example config at `config.example.toml`. 6 M3 integration tests. | `crates/core/src/config.rs`, `engine.rs`, `lib.rs`, `crates/cli/src/main.rs` | M | Backend | M2 | CLI `--config` works ✓; custom thresholds change classification ✓ | 6 integration tests ✓ |
| M4 | Medium | Duplicate pixel iteration (signal + residual) | **DONE** — Added `compute_pixel_stats_and_residual` that computes noise, edge, block metrics and residual map in a single pixel pass. `compute_signal_metrics_timed` calls this unified function. Eliminated redundant iteration over all pixels. | `crates/core/src/signal.rs` | M | Backend | M1 | Single pixel-iteration pass ✓ | All tests pass ✓ |
| M5 | Medium | f32 precision loss in correlation sums | **DONE** — `compute_shifted_residual_corr` and `block_corr` now use `f64` accumulators for all sum/mean/correlation computations; cast result to `f32` at output. Prevents catastrophic cancellation on large images (>10MP). | `crates/core/src/engine.rs` | S | Backend | — | Correlation computed in f64 ✓; all 98 tests pass ✓ | Unit tests (block_corr, shifted_corr) ✓ |
| M6 | Medium | WASM entry forces full buffer copy | **DONE** — `verify_bytes(&[u8])` accepts borrowed data directly. WASM bindings call `verify_bytes` with `&[u8]` from wasm-bindgen — no `.to_vec()` in hot path. Old `verify(VerifyRequest)` retained as convenience wrapper only. | `crates/core/src/engine.rs`, `crates/wasm-bindings/src/lib.rs` | S | Backend | — | No `.to_vec()` in WASM hot path ✓ | Code review ✓ |
| M7 | Medium | Authentic always emits PhyPrnu001 | **DONE** — Reason codes now driven by `derive_reason_codes()` using per-layer contribution scores above `REASON_CODE_CONTRIBUTION_THRESHOLD` (0.15). Authentic and Suspicious branches emit only codes for layers that actually contributed. Fallback ensures at least one code per result. Added 7 new unit tests. | `crates/core/src/engine.rs` | S | Backend | C1 | Authentic result with low physical contribution omits PhyPrnu001 ✓; all paths emit ≥1 code ✓ | 7 unit tests ✓; clippy clean ✓ |
| M8 | Medium | Fast mode permanently broken | **DONE** — Fast mode now runs pixel-level statistics (noise, edge, block artifact, block variance CV) and returns a structured result with per-layer contributions, score mapping, and reason codes. Uses same `CalibrationConfig` plumbing as Deep mode. | `crates/core/src/engine.rs` | M | Backend | — | Fast mode produces valid result ✓ | Unit test `verify_fast_mode_returns_result` ✓ |
| M9 | Medium | No Content-Security-Policy | **DONE** — Added CSP meta tag in `index.html` and `vercel.json` HTTP header config. Policy: `default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'none'`. Additional hardening: `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`, `Permissions-Policy`. | `web/index.html`, new `web/vercel.json` | S | Frontend | — | CSP header present in production build ✓; `connect-src 'none'` enforced ✓ | Production build inspection ✓ |
| L1 | Low | start-web.ps1 auto-installs without consent | **DONE** — Added consent prompt listing required installations; user must confirm before any `winget` or `cargo install` runs. | `start-web.ps1` | S | DevOps | — | Script prompts before installing ✓ | Manual verification ✓ |
| L2 | Low | No JS/CSS formatting | **DONE** — Added `.prettierrc` config, `npm run format` and `npm run format:check` scripts. Installed `prettier` as devDependency. All files formatted. | `web/package.json`, `web/.prettierrc` | S | Frontend | — | `npm run format:check` passes ✓ | Format check ✓ |
| L3 | Low | No versioning strategy | **DONE** — Added `CONTRIBUTING.md` with SemVer strategy, release checklist, branch naming, conventional commits, and PR requirements. | `CONTRIBUTING.md` | S | DevOps | — | Version bump process documented ✓ | Manual review ✓ |
| L4 | Low | HardwareTier enum unused | **DONE** — Removed `HardwareTier` enum from `model.rs` and `VerifyRequest`. Simplified `verify()` and `verify_bytes()` call sites. | `crates/core/src/model.rs`, `engine.rs`, `wasm-bindings/src/lib.rs` | S | Backend | M8 | No dead enum variants ✓; clippy clean ✓ | Clippy ✓ |
| L5 | Low | image crate format auto-detection | **DONE** — Added explicit format allowlist (JPEG, PNG, WebP) in `decode_image`. Unknown/unsupported formats rejected with `UnsupportedFormat` error. BMP test added. | `crates/core/src/engine.rs` | S | Backend | — | Only JPEG/PNG/WebP accepted ✓; BMP rejected ✓ | Unit test `verify_bmp_rejected_as_unsupported` ✓ |
| F1 | Feature | No analysis progress indicator | **DONE** — Added state-driven progress indicator (idle/running/completed/failed) in web UI. Real elapsed-time counter with indeterminate pulse animation. Duplicate-analysis prevention via running-state guard. Green/red completion states. | `web/src/main.js`, `web/index.html`, `web/src/styles.css` | S | Frontend | H8 | Progress visible on start ✓; transitions correctly ✓; no UI freeze ✓ | Web build passes ✓ |
| F2 | Feature | No feedback learning system | **DONE** — Added post-analysis feedback UI (correct/incorrect + classification correction). Feedback stored in localStorage (500-entry rolling window). Opt-in anonymous diagnostic sharing logs scores/classification/timing only. Privacy model documented in `docs/FEEDBACK_SYSTEM.md`. No image data in any path. | `web/src/main.js`, `web/index.html`, `web/src/styles.css`, `docs/FEEDBACK_SYSTEM.md` | L | Frontend | F1 | Feedback capture works ✓; no image data in storage ✓; opt-in defaults off ✓; privacy doc complete ✓ | Web build ✓; privacy audit checklist ✓ |

---

## Patch Strategy

### Branch Strategy

- **Main branch** (`main`): protected, requires PR + CI green + 1 approval.
- **Feature branches**: `harden/<finding-id>` (e.g., `harden/c1-fusion-normalization`).
- **Milestone branches** (optional): `harden/m0-guardrails`, `harden/m1-critical` for batching if team prefers.

### PR Slicing

Each PR should contain **one logical change** that is independently verifiable:

| PR | Contents | Milestone |
|----|---------|-----------|
| PR-1 | CI pipeline (GitHub Actions) + clippy fixes | M0 |
| PR-2 | WASM panic hook (H7) | M0 |
| PR-3 | Unit test scaffold (C4 partial — metric function tests) | M0 |
| PR-4 | Input limits (C5) + tests | M1 |
| PR-5 | Fusion weight normalization (C1) + threshold recalibration + tests | M1 |
| PR-6 | Indeterminate classification (C3) + tests | M1 |
| PR-7 | Latency truth-or-remove (C2) + tests | M1 |
| PR-8 | JPEG-only block scoring (H2) + residual border fix (H4) + tests | M1 |
| PR-9 | Perturbation tag fix (H5) + symlink protection (H6) + tests | M1 |
| PR-10 | Frontend confidence fix (H1) + CSP (M9) | M2 |
| PR-11 | Web Worker (H8) | M2 |
| PR-12 | Authentic reason code fix (M7) + f32 precision (M5) | M2 |
| PR-13 | Layer trait + module extraction (M1 arch) | M3 |
| PR-14 | Config extraction (M2, M3) + runtime config | M3 |
| PR-15 | Duplicate iteration elimination (M4) + FFT window (H3) | M3 |
| PR-16 | Buffer copy elimination (M6) + Fast mode resolution (M8) + dead code cleanup (L4) | M3 |

### Test Strategy Upgrades

| Layer | Current | Required |
|-------|---------|----------|
| Unit (Rust) | 0 tests | ≥30 tests: metric functions, fusion logic, classification gates, edge cases (tiny images, empty input, max dimensions) |
| Integration (Rust) | 0 tests | ≥5 tests: known-classification images (1 authentic JPEG, 1 PNG, 1 synthetic, 1 edited, 1 corrupt) |
| Property (Rust) | 0 tests | ≥3 tests: all scores in [0,1], fusion weights sum to 1.0, no panic on random bytes |
| Security (Rust) | 0 tests | ≥4 tests: oversized image, symlink, 1×1 image, corrupted header (see SECURITY.md checklist) |
| Frontend (JS) | 0 tests | ≥5 tests: `formatConfidence` for each classification, `formatJustification` for each classification |
| CI | None | GitHub Actions: `cargo test`, `cargo clippy -- -D warnings`, `npm run check` on push/PR |

### Rollout Strategy

1. **M0 (Guardrails)** — merge directly to main. No behavioral change. Adds safety net.
2. **M1 (Critical)** — merge PRs one at a time. After C1 (fusion normalization), **immediately re-run stress test** and recalibrate thresholds if needed before merging further M1 PRs.
3. **M2 (High)** — merge after M1 is stable. Web Worker (H8) should be feature-flagged: fall back to main-thread if Worker fails to initialize.
4. **M3 (Architecture)** — merge after M2. Run full stress test before and after refactor to confirm no accuracy regression.

**Rollback**: Any PR that breaks CI or regresses stress-test accuracy is reverted immediately.

---

## Appendix: Finding ID Cross-Reference

| Review ID | Backlog ID | Milestone |
|-----------|-----------|-----------|
| C1 | C1 | M1 |
| C2 | C2 | M1 |
| C3 | C3 | M1 |
| C4 | C4 | M0 |
| C5 | C5 | M1 |
| H1 | H1 | M2 | **DONE** |
| H2 | H2 | M1 |
| H3 | H3 | M3 | **DONE** |
| H4 | H4 | M1 | **DONE** |
| H5 | H5 | M1 | **DONE** |
| H6 | H6 | M1 | **DONE** |
| H7 | H7 | M0 |
| H8 | H8 | M2 | **DONE** |
| M1 (arch) | M1 | M3 | **DONE** |
| M2 (magic) | M2 | M3 | **DONE** |
| M3 (config) | M3 | M3 | **DONE** |
| M4 (dup iter) | M4 | M3 | **DONE** |
| M5 (f32) | M5 | M2 | **DONE** |
| M6 (copy) | M6 | M3 | **DONE** |
| M7 (reason) | M7 | M2 | **DONE** |
| M8 (fast) | M8 | M3 | **DONE** |
| M9 (CSP) | M9 | M2 | **DONE** |
| L1 | L1 | M3 | **DONE** |
| L2 | L2 | M3 | **DONE** |
| L3 | L3 | M3 | **DONE** |
| L4 | L4 | M3 | **DONE** |
| L5 | L5 | M3 | **DONE** |
| F1 | F1 | M4 | **DONE** |
| F2 | F2 | M4 | **DONE** |
