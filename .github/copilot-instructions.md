# ImageProof Workspace Setup Checklist

- [x] Verify that the copilot-instructions.md file in the .github directory is created.
- [x] Clarify Project Requirements
- [x] Scaffold the Project
- [x] Customize the Project
- [x] Install Required Extensions
- [x] Compile the Project
- [x] Create and Run Task
- [x] Launch the Project
- [x] Ensure Documentation is Complete

## Execution Progress
Starting workspace setup for ImageProof application.

## Clarified Project Requirements (Baseline v0.1 - 2026-02-23)

### Scope and Objective
- Build a client-side ImageProof verification engine for image authenticity assessment.
- Operate locally by default (browser WASM primary), with optional native client path.
- Produce measurable, explainable outputs grounded in physical and statistical signals.

### Core Functional Requirements
- Detect presence/absence of sensor-like signatures (PRNU plausibility proxy only).
- Detect localized hybrid manipulation in otherwise authentic images.
- Detect fully synthetic/generative images.
- Output composite authenticity confidence score in [0,1].
- Output per-layer contributions and structured reason codes.

### Core Non-Functional Requirements
- No external upload of image data or derived forensic signals.
- No reliance on metadata for core decisioning.
- Target latency budget: <=500ms for standard 12MP image.
- Define and enforce per-layer latency allocations and fast/deep execution modes.
- Tolerate JPEG/WebP compression, resizing, cropping, recompression, metadata removal.

### Trust and Threat Constraints
- Must not claim device identity validation without reference fingerprint.
- Must distinguish authenticity verification vs manipulation detection vs synthetic likelihood.
- Must bound claims and prefer indeterminate over false assertion.
- Must characterize performance and degradation across adversary tiers A-D.

### Scoring and Explainability Constraints
- Use explicit fusion logic for per-layer signals to composite score.
- Define thresholds for Authentic, Suspicious, Synthetic, and Indeterminate.
- Include confidence degradation causes and signal insufficiency flags.
- Maintain explicit reason-code taxonomy with stable identifiers.

### Execution and Deployment Constraints
- Primary environment: browser-based WASM with CPU fallback and GPU acceleration path.
- Hardware tiers: CPU-only, CPU+SIMD, WebGPU, Native hardware-access tier.
- Deployment patterns to support: browser extension, corporate R&D integration, desktop tool, enterprise pipeline integration.

### Validation and Calibration Constraints
- Calibrate for <=1% false positives on authentic images.
- Calibrate for <=10% false negatives on synthetic/hybrid images.
- Document dataset strategy, threshold tuning method, and measurement methodology.

## Project Customization (Baseline v0.1 - 2026-02-23)

### Engineering Structure
- Core domain contracts are split into dedicated modules for model and engine concerns.
- Shared enums now define execution mode, hardware tier, and structured reason codes.
- Verification result schema includes per-layer latency and layer reason grouping.

### Project Conventions
- Added repository editor conventions via `.editorconfig`.
- Maintained small, focused workspace with Rust core and WASM bindings crates only.
- Preserved not-implemented verification engine boundary to keep incremental delivery traceable.

### Development Tooling
- Added workspace extension recommendations in `.vscode/extensions.json` for Rust and TOML development.
- Added workspace task in `.vscode/tasks.json` for `cargo check` and executed it successfully.
- Added runnable scaffold CLI target and launched project via `cargo run -p imageproof-cli`.

### Documentation Completion
- README now documents workspace structure, prerequisites, compile/run commands, and current scaffold behavior.
- Setup checklist is fully completed and synchronized with the repository state.

## Post-Setup Incremental Work

### Web App Shell (2026-02-23)
- Added `web` frontend scaffold (Vite) with local image upload flow.
- Added npm script `build:wasm` to regenerate browser bindings from `crates/wasm-bindings`.
- Verified web build and local dev-server launch path for the scaffold UI.

### Fast-Mode Engine Stub (2026-02-23)
- Updated `imageproof-core` verification engine to return a structured `Indeterminate` scaffold result in `Fast` mode.
- Preserved `Deep` mode as `NotImplemented` to keep incremental implementation boundaries explicit.

### Web Result Formatting (2026-02-23)
- Updated `web/src/main.js` to render friendly verification summaries (classification, score, reason codes, latency).
- Preserved raw JSON output beneath the summary for implementation traceability.

### Web Mode Toggle (2026-02-23)
- Added `Fast`/`Deep` execution mode selector in the web UI.
- Wired mode selection to `verify_image` to validate both scaffold paths from browser flow.

### Web Deep-Mode Hint (2026-02-23)
- Added explicit Deep-mode scaffold message in the web result panel when `NotImplemented` is returned.
- Preserved raw error text for debugging traceability.

### Web Copy Result Control (2026-02-23)
- Added `Copy Result` action in the web result panel.
- Copied text includes summary and raw output, with inline success/failure status feedback.
