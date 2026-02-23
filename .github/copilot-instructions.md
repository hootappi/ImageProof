# ImageProof Workspace Setup Checklist

- [x] Verify that the copilot-instructions.md file in the .github directory is created.
- [x] Clarify Project Requirements
- [ ] Scaffold the Project
- [ ] Customize the Project
- [ ] Install Required Extensions
- [ ] Compile the Project
- [ ] Create and Run Task
- [ ] Launch the Project
- [ ] Ensure Documentation is Complete

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
