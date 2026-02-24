# Architecture — ImageProof

> Last updated: 2026-02-24 (post-review hardening baseline)

## Overview

ImageProof is a client-side image authenticity verification engine.
It processes untrusted image bytes locally (no server upload), extracts statistical and physical signal features, fuses them through a weighted scoring model, and emits a quad-state classification (Authentic / Suspicious / Synthetic / Indeterminate) with confidence and explainability metadata.

Primary runtime target is browser via Rust→WASM. A native CLI path exists for batch evaluation and stress testing.

## Workspace Layout

```
ImageProof/
├── crates/
│   ├── core/           # Domain types (model.rs) + verification engine (engine.rs)
│   ├── wasm-bindings/  # Thin wasm-bindgen bridge → browser
│   └── cli/            # Native CLI: launch scaffold + stress-test harness
├── web/                # Vite frontend (vanilla JS, CSS, HTML)
│   ├── src/main.js     # UI logic, drag-drop, verify action, result formatting
│   ├── src/styles.css
│   ├── index.html
│   └── pkg/            # wasm-pack output (gitignored, generated)
├── start-web.ps1       # One-click dev launcher (Windows)
├── start-web.cmd       # Double-click wrapper for start-web.ps1
└── Cargo.toml          # Workspace root
```

## Core Components

### `imageproof-core` — Verification Engine

**Files**: `crates/core/src/engine.rs` (~858 lines), `crates/core/src/model.rs`

Responsibilities:
1. Decode image bytes to grayscale (`image` crate, auto-format detection).
2. Extract signal metrics across four analysis layers.
3. Fuse per-layer scores into synthetic/edited/authentic likelihoods.
4. Classify and emit structured `VerificationResult`.

#### Detection Layers

| Layer | Metrics | Purpose |
|-------|---------|---------|
| **Signal** | noise_score, edge_score, block_artifact_score (JPEG-only, H2), block_variance_cv, spectral_peak_score (FFT), high_freq_ratio_score (FFT) | Detect statistical anomalies in pixel distributions |
| **Physical** | prnu_plausibility_score, cross_region_consistency | Proxy for sensor-originated noise patterns |
| **Hybrid** | hybrid_local_inconsistency, hybrid_seam_anomaly | Detect localized manipulation (splices, composites) |
| **Semantic** | semantic_pattern_repetition, semantic_gradient_entropy, semantic_synthetic_cue | Detect generative-model artifacts |

#### Fusion Model

Scores are combined via weighted linear sums (clamped to [0,1]) with multiplicative suppression gates:
- `synthetic_likelihood = clamp(synthetic_base × synthetic_suppression)`
- `edited_likelihood = clamp(edited_base × edited_suppression)`
- Classification thresholds: `SYNTHETIC_MIN_THRESHOLD`, `SYNTHETIC_MARGIN_THRESHOLD`, `SUSPICIOUS_MIN_THRESHOLD`, `INDETERMINATE_CEILING`, `INDETERMINATE_MIN_SPREAD`

**RESOLVED (C1)**: Fusion weights normalized to sum = 1.00 per group (synthetic_base, edited_base, authentic_likelihood). Block-artifact NaN fixed for flat images.

### `imageproof-wasm-bindings` — Browser Bridge

**File**: `crates/wasm-bindings/src/lib.rs` (32 lines)

Thin adapter: receives `&[u8]` from JS, copies into `VerifyRequest`, calls `verify()`, serializes result to `JsValue` via `serde-wasm-bindgen`.

**KNOWN ISSUE (M6)**: Forces a full buffer copy (`to_vec()`) at the entry point.
**RESOLVED (H7)**: `#[wasm_bindgen(start)]` installs `console_error_panic_hook` — Rust panics now surface readable messages in browser DevTools.

### `imageproof-cli` — Native CLI

**File**: `crates/cli/src/main.rs` (381 lines)

Two modes:
1. **Default**: prints launch confirmation.
2. **`stress <dataset_root>`**: batch evaluation across `authentic/`, `edited/`, `synthetic/` folders with per-class accuracy, perturbation tagging, and acceptance quality-bar gate.

**KNOWN ISSUE (H6)**: ~~Follows symlinks without boundary checks.~~ **RESOLVED** — `collect_recursive` uses `entry.file_type().is_symlink()` to skip symlinks with a warning.

### `web/` — Frontend

**Files**: `index.html`, `src/main.js` (170 lines), `src/styles.css`

Vanilla JS Vite app. Drag-drop image upload → WASM call → formatted result display.

**KNOWN ISSUE (H1)**: Suspicious confidence uses a parabolic formula that distorts backend scores.
**KNOWN ISSUE (H8)**: `verify_image` runs synchronously on main thread, blocking UI.

## Data Flow

```
[User drops image file]
    │
    ▼
[JS: File → ArrayBuffer → Uint8Array]
    │
    ▼
[WASM: verify_image(bytes, false)]
    │
    ▼
[Rust: VerifyRequest { image_bytes, Deep, CpuOnly }]
    │
    ▼
[image crate: decode → DynamicImage → GrayImage]
    │
    ├──► compute_signal_metrics(gray)
    │       ├── noise/edge/block (pixel iteration)
    │       ├── compute_residual_map → interior-only (H4: no border zeros)
    │       ├── compute_fft_signal_features (64×64 cap)     ← KNOWN ISSUE H3
    │       ├── compute_prnu_proxy_metrics (block correlation)
    │       ├── compute_hybrid_metrics (tile energy + seam scan)
    │       └── compute_semantic_metrics (shifted correlation + gradient entropy)
    │
    ▼
[Fusion: weighted sum → suppression → classification gates]
    │
    ▼
[VerificationResult → serde → JsValue → JS display]
```

## Trust Boundaries

```
┌─────────────────────────────────────────────┐
│  UNTRUSTED: Image bytes from user           │
│  (drag-drop, file input, CLI filesystem)    │
└──────────────────────┬──────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────┐
│  BOUNDARY: Input validation                 │
│  - Empty check: ✓                           │
│  - Max dimensions: ✓ (16384, C5 resolved)  │
│  - Max file size: ✓ (50 MB, C5 resolved)    │
│  - Format restriction: partial (guessed)    │
└──────────────────────┬──────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────┐
│  TRUSTED: image crate decode + engine       │
│  (runs in-process, no network, no IPC)      │
└──────────────────────┬──────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────┐
│  OUTPUT: VerificationResult (JSON)          │
│  - No PII, no image data in output          │
│  - Score, classification, reason codes      │
└─────────────────────────────────────────────┘
```

**No network calls exist in core or WASM paths.** The "no external upload" requirement is met.

## Key Design Decisions and Tradeoffs

| Decision | Rationale | Tradeoff |
|----------|-----------|----------|
| Heuristic-only (no ML model) | Zero model download, predictable latency, no GPU dependency | Lower accuracy ceiling than trained classifiers |
| All-in-one engine.rs | Rapid iteration during prototyping phase | Monolithic, hard to test layers independently (M1) |
| Browser-first WASM target | Meets privacy requirement (no upload) | Main-thread blocking (H8), limited compute budget |
| Grayscale-only analysis | Halves memory, simplifies math | Loses color-channel forensic signals |
| 64×64 FFT window cap | Bounded compute cost | Misses fine spectral artifacts (H3) |
| Fabricated latency values | Placeholder for future instrumentation | ~~Produces misleading diagnostics (C2)~~ RESOLVED: real `Instant::now()` timing |

## Known Risks — Summary

| ID | Risk | Mitigation Status |
|----|------|-------------------|
| C1 | Fusion weights >1.0 — unstable scoring | **RESOLVED** — weights normalized to sum = 1.00 |
| C2 | Fabricated latency data | **RESOLVED** — real `Instant::now()` per-layer timing |
| C3 | Indeterminate classification dead code | **RESOLVED** — quad-state classification with Indeterminate branch |
| C4 | Zero automated tests | **RESOLVED** — 78 tests + CI pipeline |
| C5 | Unbounded memory from large images | **RESOLVED** — 50 MB file + 16384 dimension limits |
| H1–H8 | Various high-priority issues | H2 **RESOLVED** (JPEG format gating), H4 **RESOLVED** (residual border exclusion), H5 **RESOLVED** (stem-only perturbation tagging), H6 **RESOLVED** (symlink protection), H7 **RESOLVED** (panic hook); remainder unmitigated — see EXECUTION_PLAN.md |

All risk IDs reference the code review findings. See `docs/EXECUTION_PLAN.md` for remediation plan and sequencing.
