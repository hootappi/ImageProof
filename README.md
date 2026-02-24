# ImageProof

ImageProof is a client-side image authenticity verification engine. It processes untrusted image files entirely locally — in-browser via Rust/WASM or natively via CLI — extracting statistical, physical, and semantic signal features to classify images as Authentic, Suspicious (edited), or Synthetic (AI-generated), with confidence scores and structured explainability. No image data ever leaves the client.

> **Status**: Early prototype (v0.1.0). A hardening sprint is planned — see `docs/EXECUTION_PLAN.md` for the full backlog. Known critical issues are documented in `docs/ARCHITECTURE.md` and `docs/CHANGELOG.md`.

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

There are **no runtime configuration options** at present. All thresholds and weights are compile-time constants in `crates/core/src/engine.rs`. There are no secrets, API keys, or environment variables.

| Parameter | File | Default |
|-----------|------|---------|
| `SYNTHETIC_MIN_THRESHOLD` | `engine.rs` | 0.66 |
| `SYNTHETIC_MARGIN_THRESHOLD` | `engine.rs` | 0.12 |
| `SUSPICIOUS_MIN_THRESHOLD` | `engine.rs` | 0.62 |

Safe defaults: all processing is local, no network calls, no persistent state.

## Common Workflows

| Task | Command |
|------|---------|
| Compile check (Rust) | `cargo check` |
| Compile check (Rust, VS Code) | Task → `cargo: check` |
| Build WASM bindings | `cd web && npm run build:wasm` |
| Full web build (WASM + Vite) | `cd web && npm run check` |
| Run dev server | `cd web && npm run dev -- --host 127.0.0.1 --port 4173` |
| Lint (Rust) | `cargo clippy -- -D warnings` |
| Run tests (Rust) | `cargo test` *(no tests yet — see hardening plan)* |
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
| Browser tab crashes on large image | No input dimension limits (known issue C5) | Use images ≤ 16 MP for now; fix tracked in hardening plan |
| `RuntimeError: unreachable` in console | WASM panic with no panic hook (known issue H7) | Reproduce via CLI for stack trace; fix tracked in hardening plan |
| Authentic photo classified as edited/synthetic | False positive from heuristic engine | Report image; check fusion thresholds; run stress test to measure rates |

## Documentation

| Document | Contents |
|----------|---------|
| `docs/ARCHITECTURE.md` | Components, data flow, trust boundaries, design decisions, known risks |
| `docs/SECURITY.md` | Threat model, attack surfaces, security controls, test checklist |
| `docs/OPERATIONS.md` | Deployment model, observability gaps, failure modes, runbooks |
| `docs/EXECUTION_PLAN.md` | Hardening backlog, milestones, sequencing, acceptance criteria, patch strategy |
| `docs/CHANGELOG.md` | Release history and planned changes |

## VS Code Extensions

Recommended in `.vscode/extensions.json`:
`rust-lang.rust-analyzer`, `tamasfe.even-better-toml`, `serayuzgur.crates`, `vadimcn.vscode-lldb`
