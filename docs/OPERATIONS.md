# Operations — ImageProof

> Last updated: 2026-04-08 (color forensics, Vercel deployment live, dev roadmap added)

## Deployment Model

### Current State

ImageProof is a **local-only development tool** with two runtime paths:

| Path | Environment | Entry Point | Status |
|------|-------------|-------------|--------|
| **Web (WASM)** | Browser | `web/` via Vite (dev) or Vercel (prod) | ✅ Live at https://imageproof.vercel.app |
| **CLI (native)** | Windows/Linux terminal | `cargo run -p imageproof-cli` | Operational with stress-test harness |

**Production deployment**: https://imageproof.vercel.app (Vercel, static site hosting, zero-Rust build — WASM artifacts committed to git).

### Dev Environment Setup

Prerequisites (Windows):
- Rust toolchain (`rustup`) with `wasm32-unknown-unknown` target
- Visual Studio 2022 Build Tools (C++ workload for `link.exe`)
- Node.js LTS (for Vite)
- `wasm-pack` (auto-installed by `start-web.ps1`)

Quick start:
```powershell
# Option A: one-click
.\start-web.cmd

# Option B: manual
Set-Location web
npm install
npm run check        # builds WASM + Vite
npm run dev -- --host 127.0.0.1 --port 4173
```

### Future Production Path (Vercel)

Deployed and operational:
- Static site hosting (Vite `dist/` output + `pkg/` WASM artifacts)
- No server-side component — all processing client-side
- CSP headers configured in `vercel.json` with hardening headers (`X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy`)
- HTTPS enforced by platform
- GitHub repo: https://github.com/hootappi/ImageProof

## Observability

### Current Gaps

| Capability | Status | Notes |
|------------|--------|-------|
| Structured logging | ❌ Missing | No `tracing` or `log` crate in Rust; no console.log strategy in JS |
| Per-layer timing | ✅ Real measurement | `Instant::now()` per-layer wall-clock timing (C2 resolved) |
| Error reporting | Improved | Browser: panic hook surfaces readable messages (H7). CLI: stderr. No aggregation. |
| Progress indicator | ✅ Implemented | State-driven UI progress (idle/running/completed/failed) with real elapsed time (F1) |
| User feedback | ✅ Implemented | Post-analysis feedback UI with localStorage persistence (F2) |
| Metrics/counters | ❌ Missing | No classification distribution tracking |
| Health checks | N/A | No backend to monitor |

### Where to Look When Something Goes Wrong

| Symptom | Where to Check | Likely Cause |
|---------|---------------|--------------|
| WASM init fails | Browser console (F12) | Missing `pkg/` files, CORS issue, or wasm-pack build failure |
| "Verification failed" in UI | Browser console | Image decode error or unsupported format (JPEG/PNG/WebP only). Panic hook shows readable message. |
| Browser tab crashes | Task manager | Image may exceed 50 MB file or 16384 dimension limit (guards in place; check console for rejection) |
| Stress test panics | Terminal stderr | Edge-case image triggering unwrap or index OOB |
| `npm run check` fails | Terminal | `wasm-pack` not installed, Rust compile error, or missing wasm32 target |
| Incorrect classification | Stress test report | Algorithm tuning issue — check per-class accuracy and fusion weights |

### Recommended Observability Additions

1. Add `tracing` crate to core for structured per-layer debug logging (gated behind feature flag).
2. Add classification-outcome counters in web UI for local session diagnostics.
3. Add aggregate diagnostic reporting from F2 feedback data (opt-in anonymous sharing).

## Failure Modes and Runbooks

### FM1: OOM on Large Image

**Trigger**: User drops image with extreme decoded dimensions (e.g., 65535×65535).
**Impact**: Rejected before decode with `DimensionTooLarge` or `InputTooLarge` error.
**Current mitigation**: 50 MB file size limit (pre-decode) + 16384 max dimension (post-decode). Both enforced in core engine.
**Runbook**: Error message displayed in result panel. No crash expected.

### FM2: WASM Panic (Unexpected Error)

**Trigger**: Unexpected Rust panic inside WASM (e.g., index OOB, division by zero in edge case).
**Impact**: JS receives error with readable message and stack trace via `console_error_panic_hook`.
**Current mitigation**: Panic hook installed (H7). Caught by `try/catch` in `main.js`, displayed as error in result panel.
**Runbook**: Check browser console (F12) for the full error message. Reproduce with the same image in CLI for detailed diagnostics.

### FM3: Incorrect Classification

**Trigger**: Algorithm produces false positive (authentic labeled as edited/synthetic) or false negative.
**Impact**: User receives wrong forensic conclusion.
**Current mitigation**: Fusion weights normalized to sum = 1.0 (C1). Indeterminate classification gate emits uncertain result when evidence is insufficient (C3). Stress test harness available. User feedback system (F2) captures corrections.
**Runbook**: Record the image (or its hash). Provide feedback via the web UI (F2). Run stress test. Check per-class accuracy. Adjust thresholds via `--config` if systematic.
**Fix**: Expand calibration dataset and tune thresholds iteratively.

### FM4: Stress Test Crashes Mid-Run

**Trigger**: Corrupted or adversarial image in dataset.
**Impact**: Test aborts — partial results lost.
**Current mitigation**: `DecodeFailed` errors are counted and skipped.
**Runbook**: Check the error message for the offending file path. Remove or quarantine the file. Re-run.
**Fix**: Add per-file timeout, add more robust error recovery.

### FM5: WASM Build Fails

**Trigger**: Rust compile error, missing `wasm32-unknown-unknown` target, or `wasm-pack` not installed.
**Impact**: `npm run check` or `npm run build:wasm` returns nonzero.
**Runbook**:
```powershell
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
Set-Location web; npm run build:wasm
```
Check Rust compiler output for the specific error.

## Backup and Restore

Not applicable. ImageProof:
- Stores no persistent data.
- Has no database.
- Has no user state.
- All inputs are transient (dropped image files).
- Source code is version-controlled in Git.

## Configuration

### Current State

All 93 calibration parameters are defined as compile-time constants in `crates/core/src/config.rs` and can be overridden at runtime via an optional TOML file using `CalibrationConfig`.

```powershell
cargo run -p imageproof-cli -- stress <dataset> --config my_config.toml
```

See `config.example.toml` for the complete reference with all fields and defaults.

| Key Parameter | Location | Default |
|---------------|----------|---------|
| `synthetic_min_threshold` | `config.rs` | 0.62 |
| `synthetic_margin_threshold` | `config.rs` | 0.12 |
| `suspicious_min_threshold` | `config.rs` | 0.62 |
| `indeterminate_ceiling` | `config.rs` | 0.32 |
| `max_file_size_bytes` | `config.rs` | 52428800 (50 MB) |
| `max_image_dimension` | `config.rs` | 16384 |
| `fft_window_cap` | `config.rs` | 256 |
| `min_samples_per_class` | `cli/main.rs` | 25 |
| `max_authentic_false_positive_rate` | `cli/main.rs` | 0.01 |
| `max_suspicious_miss_rate` | `cli/main.rs` | 0.10 |
| `max_synthetic_miss_rate` | `cli/main.rs` | 0.10 |
