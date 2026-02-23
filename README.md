# ImageProof

Client-side visual authenticity verification engine scaffold.

## Workspace

- `crates/core`: core verification domain types and engine contracts
- `crates/wasm-bindings`: browser/WASM interface layer
- `crates/cli`: runnable scaffold entrypoint

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

### VS Code Task

Run task `cargo: check` from the Command Palette or Tasks runner.

### Launch (Scaffold)

```powershell
cargo run -p imageproof-cli
```

## Current Behavior

- `imageproof-core` defines verification contracts and returns `NotImplemented` for engine execution.
- `imageproof-wasm-bindings` exposes `verify_image` for browser/WASM integration.
- `imageproof-cli` provides a runnable scaffold entrypoint for launch validation.

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
