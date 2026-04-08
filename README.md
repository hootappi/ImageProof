# ImageProof

ImageProof is a client-side image authenticity verification engine. It processes untrusted image files entirely locally — in-browser via Rust/WASM or natively via CLI — extracting statistical, physical, and semantic signal features to classify images as Authentic, Suspicious (edited), or Synthetic (AI-generated), with confidence scores and structured explainability. No image data ever leaves the client.

> **Status**: v0.1.0 — hardening sprint complete. All 29 backlog items (C1–C5, H1–H8, M1–M9, L1–L5, F1–F2) resolved. 105 automated tests, CI pipeline, Web Worker offload, runtime TOML config, and privacy-preserving feedback system are in place. See `docs/EXECUTION_PLAN.md` for the full backlog and `docs/CHANGELOG.md` for details.

## Quickstart

### One-Click Start (Windows, Recommended)

```bat
start-web.cmd
```

Or in PowerShell:

```powershell
Set-ExecutionPolicy -Scope Process Bypass; .\start-web.ps1
```

The script auto-checks/installs Rust toolchain, C++ linker, wasm-pack, and npm dependencies, then builds and launches the dev server at `http://127.0.0.1:4173/`.

### Manual Start

```powershell
# Prerequisites: Rust toolchain (rustup), VS Build Tools (C++ workload), Node.js LTS, wasm-pack
cd web
npm install
npm run check          # builds WASM + Vite production bundle
npm run dev -- --host 127.0.0.1 --port 4173
```

### CLI

```powershell
cargo run -p imageproof-cli                        # launch validation
cargo run -p imageproof-cli -- stress <dataset>    # batch evaluation
```

## Workspace

```
crates/core/           Core domain types (model.rs) and verification engine (engine.rs)
crates/wasm-bindings/  Thin wasm-bindgen bridge for browser
crates/cli/            Native CLI: launch scaffold + stress-test harness
web/                   Vite frontend (vanilla JS) with drag-drop upload flow
docs/                  Architecture, security, operations, execution plan
```

## Configuration

All 93 calibration parameters have sensible compile-time defaults in `crates/core/src/config.rs`. They can be overridden at runtime via an optional TOML file — no recompilation needed.

```powershell
# CLI with custom thresholds
cargo run -p imageproof-cli -- stress <dataset> --config my_config.toml
```

See `config.example.toml` for all available fields with default values.

| Key Parameter | Default | Description |
|---------------|---------|-------------|
| `synthetic_min_threshold` | 0.66 | Minimum synthetic likelihood for Synthetic classification |
| `synthetic_margin_threshold` | 0.12 | Required margin of synthetic over edited likelihood |
| `suspicious_min_threshold` | 0.62 | Minimum edited likelihood for Suspicious classification |
| `indeterminate_ceiling` | 0.32 | Max likelihood for either axis to trigger Indeterminate |
| `max_file_size_bytes` | 52428800 | Input file size limit (50 MB) |
| `max_image_dimension` | 16384 | Max width or height after decode |

Safe defaults: all processing is local, no network calls, no secrets, no API keys.

## Common Workflows

| Task | Command |
|------|---------|
| Compile check (Rust) | `cargo check` |
| Compile check (Rust, VS Code) | Task → `cargo: check` |
| Build WASM bindings | `cd web && npm run build:wasm` |
| Full web build (WASM + Vite) | `cd web && npm run check` |
| Run dev server | `cd web && npm run dev -- --host 127.0.0.1 --port 4173` |
| Lint (Rust) | `cargo clippy -- -D warnings` |
| Run tests (Rust) | `cargo test` *(105 tests: 81 core + 24 CLI)* |
| Stress test | `cargo run -p imageproof-cli -- stress <dataset_root>` |

## Stress Testing

```powershell
cargo run -p imageproof-cli -- stress <dataset_root>
```

Expected dataset structure:

```
<dataset_root>/
    authentic/      # real camera photos
    edited/         # manipulated images
    synthetic/      # AI-generated images
```

Recursively scans `jpg/jpeg/png/webp` files and reports per-class accuracy, perturbation-tag accuracy, decode failures, and acceptance quality-bar pass/fail.

### Acceptance Quality Bar

| Criterion | Threshold |
|-----------|-----------|
| Min samples per class | ≥ 25 |
| Authentic false-positive rate | ≤ 1% |
| Edited miss rate | ≤ 10% |
| Synthetic miss rate | ≤ 10% |

## Troubleshooting

| Problem | Likely Cause | Fix |
|---------|-------------|-----|
| `npm run build:wasm` fails | `wasm-pack` not installed or `wasm32-unknown-unknown` target missing | `cargo install wasm-pack && rustup target add wasm32-unknown-unknown` |
| WASM init fails in browser | Missing `web/pkg/` directory (not built) | Run `cd web && npm run build:wasm` |
| Browser tab crashes on large image | Image exceeds 50 MB or 16384×16384 dimension limit | Engine rejects oversized inputs automatically; check browser console for error |
| `RuntimeError: unreachable` in console | Unexpected Rust panic (edge case) | Panic hook installed — check browser console (F12) for full error message and stack trace |
| Authentic photo classified as edited/synthetic | False positive from heuristic engine | Report image; check fusion thresholds; run stress test to measure rates |

## Documentation

| Document | Contents |
|----------|---------|
| `docs/ARCHITECTURE.md` | Components, data flow, trust boundaries, design decisions, known risks |
| `docs/SECURITY.md` | Threat model, attack surfaces, security controls, test checklist |
| `docs/OPERATIONS.md` | Deployment model, observability gaps, failure modes, runbooks |
| `docs/EXECUTION_PLAN.md` | Hardening backlog, milestones, sequencing, acceptance criteria, patch strategy |
| `docs/CHANGELOG.md` | Release history and change log |
| `docs/FEEDBACK_SYSTEM.md` | Privacy model for the feedback learning system |
| `CONTRIBUTING.md` | Versioning strategy, branch naming, commit conventions, PR requirements |
| `config.example.toml` | Reference TOML with all 93 calibration fields (commented defaults) |

## VS Code Extensions

Recommended in `.vscode/extensions.json`:
`rust-lang.rust-analyzer`, `tamasfe.even-better-toml`, `serayuzgur.crates`, `vadimcn.vscode-lldb`
