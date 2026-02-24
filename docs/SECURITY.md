# Security — ImageProof

> Last updated: 2026-02-24 (post-review hardening baseline)

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
| Decompression bomb (huge dimensions in small file) | **VULNERABLE** — no dimension/size limits | C5 |
| Malformed image header causing panic | **PARTIALLY MITIGATED** — decode errors caught, but panics in `image` crate are unbounded | H7 |
| Format confusion (polyglot files) | **LOW RISK** — `with_guessed_format()` delegates to `image` crate heuristics | L5 |

**Required control**: Enforce max file size (e.g., 50 MB) and max decoded dimensions (e.g., 16384×16384) before decode.

### 2. Filesystem Traversal (CLI only)

**Entry point**: `collect_recursive()` in `crates/cli/src/main.rs`.

| Attack | Current Status | Finding ID |
|--------|---------------|------------|
| Symlink to sensitive files | **VULNERABLE** — no symlink detection | H6 |
| Symlink escape from dataset directory | **VULNERABLE** — no path canonicalization | H6 |

**Required control**: Check `entry.file_type().is_symlink()` and skip, or canonicalize paths and verify they remain within dataset root.

### 3. WASM Runtime

| Attack | Current Status | Finding ID |
|--------|---------------|------------|
| Panic → opaque crash | **VULNERABLE** — no `console_error_panic_hook` | H7 |
| OOM via large allocation | **VULNERABLE** — browser may kill tab | C5 |
| Main-thread blocking (DoS of UI) | **VULNERABLE** — synchronous execution | H8 |

### 4. Web Frontend

| Attack | Current Status | Finding ID |
|--------|---------------|------------|
| XSS via injected content | **LOW RISK** — uses `textContent` (not `innerHTML`), but no CSP | M9 |
| Content injection post-deployment | **PARTIALLY VULNERABLE** — no Content-Security-Policy header | M9 |

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
| Input dimension limits | ❌ Missing | See C5 |
| Input file size limits | ❌ Missing | See C5 |
| WASM panic hook | ❌ Missing | See H7 |
| CSP headers | ❌ Missing | See M9 |
| Symlink protection (CLI) | ❌ Missing | See H6 |
| Automated security tests | ❌ Missing | See C4 |

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

1. **Do not run stress tests on untrusted dataset directories** until symlink protection (H6) is implemented.
2. **Monitor memory usage** when processing unknown images — no dimension limits exist yet (C5).

## Security Test Checklist

Tests to implement as part of hardening (see EXECUTION_PLAN.md):

- [ ] **Input limits**: Submit image with declared dimensions 65535×65535. Expect rejection, not OOM.
- [ ] **Empty input**: Submit zero-byte payload. Expect `EmptyInput` error.
- [ ] **Corrupted header**: Submit 1 KB of random bytes. Expect `DecodeFailed` error.
- [ ] **Truncated image**: Submit first 50% of a valid JPEG. Expect `DecodeFailed` error.
- [ ] **Minimum viable image**: Submit 1×1 PNG. Expect graceful result (not panic).
- [ ] **3×3 image**: Submit tiny image. Expect all-zero metrics and Indeterminate or safe classification.
- [ ] **Symlink in dataset** (CLI): Create symlink to file outside dataset root. Expect skip or rejection.
- [ ] **WASM panic recovery**: Trigger edge case that would panic. Expect error message (not opaque crash).
- [ ] **Concurrent verification** (future): If Web Worker is added, verify no shared-state corruption.
