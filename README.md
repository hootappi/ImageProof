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

- `imageproof-core` defines verification contracts and returns a minimal `Indeterminate` scaffold result in `Fast` mode; `Deep` mode remains `NotImplemented`.
- `imageproof-wasm-bindings` exposes `verify_image` for browser/WASM integration.
- `imageproof-cli` provides a runnable scaffold entrypoint for launch validation.
- `web` provides a modern drag-drop upload flow with in-box image preview, `Verify` and `Clear` actions, and simple human-readable result output (`Confidence` and `Justification`).

## Repository Operations

- Build validation: `cargo check` or VS Code task `cargo: check`.
- Web validation: `Set-Location web; npm run check`.
- Launch validation: `cargo run -p imageproof-cli`.
- Recommended workflow: keep changes incremental and update `.github/copilot-instructions.md` checklist as steps complete.

## Customization Baseline

- Modular core contracts (`model` and `engine`) for maintainability and testability.
- Structured reason code taxonomy and execution/hardware tier enums.
- Repository editor conventions in `.editorconfig`.

## Status

Workspace setup checklist is complete and aligned to the current implementation baseline.
