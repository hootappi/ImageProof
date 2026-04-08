# Security — ImageProof

> Last updated: 2026-04-08 (all hardening findings resolved)

## Threat Model

### System Description

ImageProof processes untrusted image files locally in-browser (WASM) or via native CLI. It produces a forensic classification (Authentic / Suspicious / Synthetic) with confidence scores. No image data leaves the client.

### Assumptions

1. **Attacker has full control of input images.** Any file dropped into the UI or placed in a CLI dataset folder must be treated as adversarial.
2. **Browser sandbox is the primary isolation boundary.** WASM runs within browser memory limits and origin restrictions.
3. **CLI runs with the invoking user's filesystem permissions.** No privilege separation.
4. **No authentication or authorization exists.** The tool is a local utility, not a multi-user service.
5. **No secrets or credentials are handled.** No API keys, no tokens, no user accounts.

### Attacker Capabilities (Tiers)

| Tier | Capability | Relevant Attacks |
|------|-----------|-----------------|
| A | Casual user submitting unusual images | Oversized images, corrupted files, unsupported formats |
| B | Deliberate adversary crafting images to evade detection | Adversarial perturbations, anti-forensic processing |
| C | Adversary crafting images to crash/DoS the engine | Decompression bombs, malformed headers, extreme dimensions |
| D | Supply-chain attacker modifying dependencies | Compromised `image` crate, malicious npm packages |

## Attack Surfaces

### 1. Image Payload (Primary)

**Entry points**: `verify_image()` in WASM bindings, `verify()` in core, `fs::read()` in CLI.

| Attack | Current Status | Finding ID |
|--------|---------------|------------|
| Decompression bomb (huge dimensions in small file) | **RESOLVED** — 50 MB file size limit + 16384 max dimension enforced before/after decode | C5 |
| Malformed image header causing panic | **MITIGATED** — decode errors caught; `console_error_panic_hook` installed for WASM panics | H7 |
| Format confusion (polyglot files) | **MITIGATED** — explicit format allowlist (JPEG/PNG/WebP only); all others rejected with `UnsupportedFormat` | L5 |

**Required control**: Enforce max file size (e.g., 50 MB) and max decoded dimensions (e.g., 16384×16384) before decode.

### 2. Filesystem Traversal (CLI only)

**Entry point**: `collect_recursive()` in `crates/cli/src/main.rs`.

| Attack | Current Status | Finding ID |
|--------|---------------|------------|
| Symlink to sensitive files | **RESOLVED** — `entry.file_type().is_symlink()` detects and skips symlinks with warning | H6 |
| Symlink escape from dataset directory | **RESOLVED** — symlinks skipped before any read occurs | H6 |

**Required control**: Check `entry.file_type().is_symlink()` and skip, or canonicalize paths and verify they remain within dataset root.

### 3. WASM Runtime

| Attack | Current Status | Finding ID |
|--------|---------------|------------|
| Panic → opaque crash | **RESOLVED** — `console_error_panic_hook` installed; panics surface readable messages | H7 |
| OOM via large allocation | **MITIGATED** — 50 MB file + 16384 dimension limits reject before decode | C5 |
| Main-thread blocking (DoS of UI) | **RESOLVED** — Web Worker offload with main-thread fallback | H8 |

### 4. Web Frontend

| Attack | Current Status | Finding ID |
|--------|---------------|------------|
| XSS via injected content | **LOW RISK** — uses `textContent` (not `innerHTML`); CSP enforced | M9 |
| Content injection post-deployment | **RESOLVED** — CSP meta tag + Vercel HTTP headers enforce `default-src 'none'`; no external scripts | M9 |

### 5. Developer Tooling

| Attack | Current Status | Finding ID |
|--------|---------------|------------|
| Auto-install of software via `start-web.ps1` | **LOW RISK** — only affects dev machines, uses official sources | L1 |
| npm supply chain | **LOW RISK** — single dependency (vite), lockfile present | — |

## Security Controls Implemented

| Control | Status | Notes |
|---------|--------|-------|
| No network calls from core/WASM | ✅ Implemented | No fetch, XHR, or WebSocket in any Rust or JS code |
| No image data in output | ✅ Implemented | Output contains only scores, classifications, reason codes |
| Empty-input rejection | ✅ Implemented | `VerifyError::EmptyInput` on zero-length bytes |
| Decode error handling | ✅ Implemented | `VerifyError::DecodeFailed` catches `image` crate errors |
| DOM output via textContent | ✅ Implemented | No innerHTML usage in `main.js` |
| Input dimension limits | ✅ Implemented | Max 16384×16384 after decode (C5) |
| Input file size limits | ✅ Implemented | Max 50 MB before decode (C5) |
| Format allowlist | ✅ Implemented | JPEG/PNG/WebP only; others rejected (L5) |
| WASM panic hook | ✅ Implemented | `console_error_panic_hook` installed at WASM init (H7) |
| CSP headers | ✅ Implemented | Meta tag + Vercel HTTP headers (M9) |
| Symlink protection (CLI) | ✅ Implemented | Symlinks detected and skipped with warning (H6) |
| Web Worker offload | ✅ Implemented | Verification runs off main thread with fallback (H8) |
| Automated security tests | ✅ Implemented | 105 tests including boundary/edge-case/input-limit tests (C4) |

## Required Operational Practices

### For Development

1. **Pin dependency versions** — `Cargo.lock` is committed. Do not delete it. Run `cargo audit` before updating dependencies.
2. **Review `image` crate updates** — this is the primary parser for untrusted data. Check CVE databases before bumping.
3. **Do not add `innerHTML` usage** — all DOM text output must use `textContent` or equivalent safe APIs.
4. **Do not add network calls** — the privacy guarantee depends on no image data leaving the client.

### For Deployment (Future)

1. **Set CSP headers** — `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' blob:; connect-src 'none'`
2. **Serve over HTTPS only** — required for WASM execution in most browsers.
3. **Set `X-Content-Type-Options: nosniff`** and `X-Frame-Options: DENY`.
4. **No secrets to rotate** — no credentials exist in this system.

### For CLI Usage

1. **Symlink protection is active** — `collect_recursive` skips symlinks with a warning to stderr.
2. **Input limits enforced** — files >50 MB and images >16384 in either dimension are rejected automatically.

## Security Test Checklist

Tests implemented as part of hardening (see EXECUTION_PLAN.md):

- [x] **Input limits**: Submit image with declared dimensions 65535×65535. Rejected with `DimensionTooLarge`.
- [x] **File size limit**: Submit >50 MB file. Rejected with `InputTooLarge`.
- [x] **Empty input**: Submit zero-byte payload. Returns `EmptyInput` error.
- [x] **Corrupted header**: Submit random bytes. Returns `DecodeFailed` error.
- [x] **Minimum viable image**: Submit 1×1 PNG. Graceful result (no panic).
- [x] **3×3 image**: Submit tiny image. Returns valid classification.
- [x] **Symlink in dataset** (CLI): Symlinks detected and skipped with warning.
- [x] **WASM panic recovery**: Panic hook installed; errors surface in browser console.
- [x] **Web Worker offload**: Verification runs off main thread; fallback to sync if Worker fails.
- [x] **Format restriction**: BMP/GIF/TIFF rejected with `UnsupportedFormat`.
- [x] **Fusion weight sums**: All weight groups verified to sum ≤1.0 in automated tests.
- [x] **NaN-free scoring**: Property test ensures no NaN in output on flat/tiny images.
