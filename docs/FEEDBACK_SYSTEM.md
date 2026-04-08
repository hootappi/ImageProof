# Feedback System — Privacy & Data Model

> Last updated: 2026-04-08

## Overview

ImageProof includes a post-analysis feedback mechanism that lets users
indicate whether a verification result was correct or incorrect. All feedback
is stored locally; no data leaves the device unless the user explicitly
opts in to anonymous diagnostic sharing.

## Privacy Principles

1. **No image data in any storage or transmission path.** The feedback
   system never captures, stores, or transmits pixel data, file names,
   file paths, or any information that could reconstruct the original image.

2. **Local-first by default.** Feedback is persisted in the browser's
   `localStorage` under the key `imageproof_feedback_log`. No server
   endpoint exists in the current release.

3. **Opt-in diagnostics.** The user must manually check the "Share
   anonymous diagnostic data" checkbox before any diagnostic payload
   is generated. The preference is persisted in `localStorage` under
   `imageproof_diagnostic_optin` and defaults to **off**.

4. **Minimal payload.** When diagnostics are enabled, only derived
   features (scores, classification, reason codes, layer contributions,
   timing) and the user's feedback label are included. See the schema
   below.

## Data Schema

### Feedback Entry (localStorage)

Each entry in the `imageproof_feedback_log` array:

| Field                  | Type       | Description                                      |
|------------------------|------------|--------------------------------------------------|
| `timestamp`            | ISO 8601   | When the feedback was recorded                   |
| `classification`       | string     | Engine classification (Authentic / Suspicious / Synthetic / Indeterminate) |
| `authenticity_score`   | float      | Engine authenticity score [0, 1]                  |
| `reason_codes`         | string[]   | Reason codes emitted by the engine               |
| `layer_contributions`  | object     | Per-layer contribution scores (signal, physical, hybrid, semantic) |
| `elapsed_ms`           | integer    | Wall-clock analysis duration in milliseconds     |
| `feedback`             | string     | `"correct"` or `"incorrect"`                     |
| `correction`           | string?    | User-provided correct classification (only when feedback is `"incorrect"`) |

### Diagnostic Payload (opt-in)

Identical fields as above, wrapped with a version number:

```json
{
  "version": 1,
  "timestamp": "2026-04-08T12:00:00.000Z",
  "classification": "Suspicious",
  "authenticity_score": 0.42,
  "reason_codes": ["HybEla001", "SigFreq001"],
  "layer_contributions": { "signal": 0.31, "physical": 0.12, "hybrid": 0.45, "semantic": 0.08 },
  "elapsed_ms": 237,
  "feedback": "incorrect",
  "correction": "Authentic"
}
```

**Fields explicitly excluded**: file name, file path, file size, image
dimensions, pixel data, EXIF/metadata, user identifiers, device fingerprints.

## Local Storage Model

- **Key**: `imageproof_feedback_log`
- **Format**: JSON array of feedback entries
- **Retention**: rolling window of the most recent 500 entries (older entries are pruned on write)
- **Scope**: per-origin (`localStorage` is bound to the web app's origin)

## Diagnostic Opt-In Preference

- **Key**: `imageproof_diagnostic_optin`
- **Values**: `"true"` (opted in) or absent/`"false"` (opted out)
- **Default**: opted out

## Transmission Model (Future)

The current release does **not** transmit diagnostic payloads. When an
endpoint is configured:

1. Payloads will be sent via `POST` to a configurable URL.
2. Transport must use HTTPS with certificate pinning.
3. No cookies, session tokens, or user identifiers will be attached.
4. The CSP `connect-src` directive must be updated to allow the
   diagnostic endpoint.
5. Transmission failures are silently dropped (no retry, no queue).

## Audit Checklist

- [x] Feedback UI captures only classification + feedback label
- [x] localStorage contains no image data
- [x] Diagnostic payload contains no image data, file names, or PII
- [x] Opt-in defaults to off
- [x] No `fetch` or `XMLHttpRequest` calls exist in the feedback code path
- [x] CSP blocks all external network by default (`connect-src 'self'`)
