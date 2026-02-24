# Operations — ImageProof

> Last updated: 2026-02-24 (post-review hardening baseline)

## Deployment Model

### Current State

ImageProof is a **local-only development tool** with two runtime paths:

| Path | Environment | Entry Point | Status |
|------|-------------|-------------|--------|
| **Web (WASM)** | Browser (localhost) | `web/` via Vite dev server | Dev-only, no production deployment |
| **CLI (native)** | Windows terminal | `cargo run -p imageproof-cli` | Dev/evaluation tool |

**No production deployment exists.** Vercel deployment is planned but deferred.

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

When deployed:
- Static site hosting (Vite `dist/` output + `pkg/` WASM artifacts)
- No server-side component — all processing client-side
- CSP headers must be configured in `vercel.json` (not yet created)
- HTTPS enforced by platform

## Observability

### Current Gaps

| Capability | Status | Notes |
|------------|--------|-------|
| Structured logging | ❌ Missing | No `tracing` or `log` crate in Rust; no console.log strategy in JS |
| Per-layer timing | ❌ Fabricated | `latency_ms` in output is a formula, not measurement (finding C2) |
| Error reporting | Minimal | Browser: errors shown in result panel. CLI: stderr. No aggregation. |
| Metrics/counters | ❌ Missing | No classification distribution tracking |
| Health checks | N/A | No backend to monitor |

### Where to Look When Something Goes Wrong

| Symptom | Where to Check | Likely Cause |
|---------|---------------|--------------|
| WASM init fails | Browser console (F12) | Missing `pkg/` files, CORS issue, or wasm-pack build failure |
| "Verification failed" in UI | Browser console | Image decode error, WASM panic (opaque `RuntimeError: unreachable`) |
| Browser tab crashes | Task manager | OOM from oversized image (no dimension limits — finding C5) |
| Stress test panics | Terminal stderr | Edge-case image triggering unwrap or index OOB |
| `npm run check` fails | Terminal | `wasm-pack` not installed, Rust compile error, or missing wasm32 target |
| Incorrect classification | Stress test report | Algorithm tuning issue — check per-class accuracy and fusion weights |

### Recommended Observability Additions (Hardening)

1. Add `console_error_panic_hook` in WASM bindings (finding H7) — makes panics debuggable.
2. Replace fabricated `latency_ms` with `std::time::Instant` / `web_sys::Performance` measurements (finding C2).
3. Add `tracing` crate to core for structured per-layer debug logging (gated behind feature flag).
4. Add classification-outcome counters in web UI for local session diagnostics.

## Failure Modes and Runbooks

### FM1: OOM on Large Image

**Trigger**: User drops image with extreme decoded dimensions (e.g., 65535×65535).
**Impact**: Browser tab crash (WASM), or process killed (CLI).
**Current mitigation**: None.
**Runbook**: Refresh browser tab. In CLI, skip the offending image.
**Fix**: Implement input dimension and file-size limits (finding C5). Reject before decode.

### FM2: WASM Panic (Opaque Crash)

**Trigger**: Unexpected Rust panic inside WASM (e.g., index OOB, division by zero in edge case).
**Impact**: JS receives `RuntimeError: unreachable` with no stack trace.
**Current mitigation**: Caught by `try/catch` in `main.js`, displayed as "Verification failed".
**Runbook**: Check browser console for the opaque error. Reproduce with the same image in CLI for better diagnostics.
**Fix**: Add `console_error_panic_hook` (finding H7).

### FM3: Incorrect Classification

**Trigger**: Algorithm produces false positive (authentic labeled as edited/synthetic) or false negative.
**Impact**: User receives wrong forensic conclusion.
**Current mitigation**: Conservative tuning (v1+v2 FP reduction). Stress test harness available.
**Runbook**: Record the image (or its hash). Run stress test. Check per-class accuracy. Adjust fusion thresholds if systematic.
**Fix**: Normalize fusion weights (finding C1), implement Indeterminate gate (finding C3), expand calibration dataset.

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

There are **no runtime configuration options**. All parameters are compile-time constants:

| Parameter | Location | Value |
|-----------|----------|-------|
| `SYNTHETIC_MIN_THRESHOLD` | `crates/core/src/engine.rs` | 0.66 |
| `SYNTHETIC_MARGIN_THRESHOLD` | `crates/core/src/engine.rs` | 0.12 |
| `SUSPICIOUS_MIN_THRESHOLD` | `crates/core/src/engine.rs` | 0.62 |
| `MIN_SAMPLES_PER_CLASS` | `crates/cli/src/main.rs` | 25 |
| `MAX_AUTHENTIC_FALSE_POSITIVE_RATE` | `crates/cli/src/main.rs` | 0.01 |
| `MAX_SUSPICIOUS_MISS_RATE` | `crates/cli/src/main.rs` | 0.10 |
| `MAX_SYNTHETIC_MISS_RATE` | `crates/cli/src/main.rs` | 0.10 |

### Recommended Additions (Hardening)

- `MAX_IMAGE_DIMENSION` and `MAX_FILE_SIZE_BYTES` constants (finding C5).
- Runtime config struct loaded from env or TOML for threshold tuning without recompilation (finding M3).
