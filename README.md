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
- `web` provides an image upload UI and calls `verify_image`; users can choose `Fast` (structured scaffold result) or `Deep` (explicit scaffold-limitation hint for not-implemented mode), and can copy the result panel text for sharing/debugging.

## Repository Operations

- Build validation: `cargo check` or VS Code task `cargo: check`.
- Launch validation: `cargo run -p imageproof-cli`.
- Recommended workflow: keep changes incremental and update `.github/copilot-instructions.md` checklist as steps complete.

## Customization Baseline

- Modular core contracts (`model` and `engine`) for maintainability and testability.
- Structured reason code taxonomy and execution/hardware tier enums.
- Repository editor conventions in `.editorconfig`.

## Status

Workspace setup checklist is complete and aligned to the current implementation baseline.
