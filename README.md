# ImageProof

Client-side visual authenticity verification engine scaffold.

## Workspace

- `crates/core`: core verification domain types and engine contracts
- `crates/wasm-bindings`: browser/WASM interface layer
- `crates/cli`: runnable scaffold entrypoint
- `web`: minimal browser app shell that loads WASM bindings

## VS Code Extensions

Recommended extensions are defined in `.vscode/extensions.json`:

- `rust-lang.rust-analyzer`
- `tamasfe.even-better-toml`
- `serayuzgur.crates`
- `vadimcn.vscode-lldb`

## Build

### One-Click Start (Recommended)

From project root:

Double-click:

```bat
start-web.cmd
```

or run in PowerShell:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\start-web.ps1
```

The script auto-checks/installs required tooling (Rust toolchain, C++ linker workload, wasm-pack), runs web verification build, and starts the dev server.

### Prerequisites (Windows)

- Rust toolchain via `rustup`
- Visual Studio 2022 Build Tools with C++ workload (for `link.exe`)

### Compile

```powershell
cargo check
```

### Build WASM for Browser

```powershell
Set-Location web
npm run build:wasm
```

### Verify Web Build in One Command

```powershell
Set-Location web
npm run check
```

### VS Code Task

Run task `cargo: check` from the Command Palette or Tasks runner.

### Launch (Scaffold)

```powershell
cargo run -p imageproof-cli
```

### Launch Web App (Local)

```powershell
Set-Location web
npm install
npm run build:wasm
npm run dev -- --host 127.0.0.1 --port 4173
```

Open: `http://127.0.0.1:4173/`

## Current Behavior

- `imageproof-core` currently runs Deep heuristic verification with Signal Intelligence v1 (noise residual extraction + FFT spectral features + block/edge metrics), Physical Intelligence v1 (PRNU plausibility proxy + cross-region consistency), Hybrid Manipulation v1 (localized residual inconsistency + seam anomaly cues), and Semantic Intelligence v1 (residual repetition + gradient-entropy cues), returning one of three classifications: `Authentic`, `Suspicious` (edited), or `Synthetic`.
- Deep verification output now includes explicit per-layer contribution scores (`signal`, `physical`, `hybrid`, `semantic`) and an embedded threshold profile (`synthetic_min`, `synthetic_margin`, `suspicious_min`) used by the current fusion gates.
- Edited-path fusion is currently tuned conservatively (higher suspicious threshold + stronger consistency suppression) to reduce false-positive edited outcomes on authentic photos.
- Synthetic-path fusion is tuned with stronger real-signal suppression (physical consistency and natural high-frequency texture) to reduce false-positive AI-generated outcomes on authentic camera photos.
- `imageproof-wasm-bindings` exposes `verify_image` for browser/WASM integration.
- `imageproof-cli` provides a runnable scaffold entrypoint for launch validation.
- `web` provides a modern drag-drop upload flow with in-box image preview, `Verify` and `Clear` actions, and simple human-readable result output with three options: real, edited, or more likely AI generated (each with confidence).

## Repository Operations

- Build validation: `cargo check` or VS Code task `cargo: check`.
- Web validation: `Set-Location web; npm run check`.
- Launch validation: `cargo run -p imageproof-cli`.
- Recommended workflow: keep changes incremental and update `.github/copilot-instructions.md` checklist as steps complete.

## Stress Testing

- Run the robustness harness:

```powershell
cargo run -p imageproof-cli -- stress <dataset_root>
```

- Expected dataset structure:

```text
<dataset_root>/
	authentic/
	edited/
	synthetic/
```

- The harness recursively scans `jpg/jpeg/png/webp` files, runs Deep verification, and reports:
	- overall accuracy
	- per-class accuracy (`Authentic`, `Suspicious`, `Synthetic`)
	- perturbation-tag accuracy based on path hints (`resized`, `cropped`, `recompressed`, `jpeg`, `webp`, `lowlight`)
	- decode-failure count

## Customization Baseline

- Modular core contracts (`model` and `engine`) for maintainability and testability.
- Structured reason code taxonomy and execution/hardware tier enums.
- Repository editor conventions in `.editorconfig`.

## Status

Workspace setup checklist is complete and aligned to the current implementation baseline.

## Open Items

- Stress-test the Deep verification pipeline across authentic, edited, and synthetic image sets (including compression/resizing variants).
- Prepare Vercel deployment path for the `web` app to collect real-user feedback.
- Define lightweight feedback collection loop (structured form/issues + triage cadence for threshold tuning).

## Acceptance Quality Bar

Current stress-test gate (printed by `imageproof-cli stress`) uses the following release-readiness criteria:

- Minimum sample size: at least `25` images per class (`authentic`, `edited`, `synthetic`).
- Authentic false-positive rate: `<=1%` (`authentic` classified as `Suspicious` or `Synthetic`).
- Edited miss rate: `<=10%` (`edited` classified as `Authentic`).
- Synthetic miss rate: `<=10%` (`synthetic` classified as `Authentic`).

A run is marked `PASS` only if all sample-size and rate criteria are met; otherwise it is marked `FAIL` with explicit notes.
