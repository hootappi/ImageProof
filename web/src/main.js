import init, { verify_image } from "../pkg/imageproof_wasm_bindings.js";

const fileInput = document.getElementById("fileInput");
const dropZone = document.getElementById("dropZone");
const dropPrompt = document.getElementById("dropPrompt");
const previewImage = document.getElementById("previewImage");
const verifyBtn = document.getElementById("verifyBtn");
const clearBtn = document.getElementById("clearBtn");
const confidenceValue = document.getElementById("confidenceValue");
const justificationValue = document.getElementById("justificationValue");

// F1: progress indicator elements
const progressSection = document.getElementById("progressSection");
const progressLabel = document.getElementById("progressLabel");
const progressTimer = document.getElementById("progressTimer");
const progressBar = document.getElementById("progressBar");

// F2: feedback elements
const feedbackSection = document.getElementById("feedbackSection");
const feedbackCorrect = document.getElementById("feedbackCorrect");
const feedbackIncorrect = document.getElementById("feedbackIncorrect");
const correctionPanel = document.getElementById("correctionPanel");
const correctionSelect = document.getElementById("correctionSelect");
const submitCorrection = document.getElementById("submitCorrection");
const feedbackAck = document.getElementById("feedbackAck");
const diagnosticOptIn = document.getElementById("diagnosticOptIn");

let selectedFile = null;
let previewUrl = null;
let isWasmReady = false;

// F1: analysis state — idle | running | completed | failed
let analysisState = "idle";
let analysisTimerHandle = null;
let analysisStartTime = null;

// F1: last result for feedback binding
let lastResult = null;
let lastElapsedMs = 0;

// H8: Web Worker support — offload WASM verification to a background thread
// so the main thread stays responsive during deep analysis. Falls back to
// synchronous main-thread execution if the Worker fails to initialize.
let worker = null;
let workerReady = false;
let pendingResolve = null;
let pendingReject = null;
let requestId = 0;

function initWorker() {
  try {
    worker = new Worker(new URL("./worker.js", import.meta.url), {
      type: "module",
    });

    worker.addEventListener("message", (event) => {
      const { type, id, result, error } = event.data;

      if (type === "ready") {
        workerReady = true;
        return;
      }

      if (type === "init-error") {
        // Worker WASM init failed — fall back to main thread
        console.warn("Worker WASM init failed, using main-thread fallback:", error);
        worker.terminate();
        worker = null;
        workerReady = false;
        return;
      }

      if (type === "result" && pendingResolve) {
        const resolve = pendingResolve;
        const reject = pendingReject;
        pendingResolve = null;
        pendingReject = null;

        if (error) {
          reject(new Error(error));
        } else {
          resolve(result);
        }
      }
    });

    worker.addEventListener("error", (event) => {
      console.warn("Worker error, using main-thread fallback:", event.message);
      worker.terminate();
      worker = null;
      workerReady = false;

      // Reject pending request so it can retry on main thread
      if (pendingReject) {
        const reject = pendingReject;
        pendingResolve = null;
        pendingReject = null;
        reject(new Error("Worker error"));
      }
    });
  } catch (e) {
    console.warn("Worker creation failed, using main-thread fallback:", e);
    worker = null;
    workerReady = false;
  }
}

function verifyViaWorker(bytes, fastMode) {
  return new Promise((resolve, reject) => {
    pendingResolve = resolve;
    pendingReject = reject;
    const id = ++requestId;
    // Transfer the buffer to avoid copying
    const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    worker.postMessage({ type: "verify", id, bytes: buffer, fastMode }, [buffer]);
  });
}

async function verifyFallback(bytes, fastMode) {
  // Main-thread synchronous fallback — blocks UI but works without Worker
  return verify_image(bytes, fastMode);
}

initWorker();

// ── F1: Progress state management ───────────────────────────────────────

function setAnalysisState(state) {
  analysisState = state;
  progressBar.classList.remove("indeterminate", "completed", "failed");

  switch (state) {
    case "running":
      progressSection.classList.remove("hidden");
      feedbackSection.classList.add("hidden");
      progressLabel.textContent = "Analyzing\u2026";
      progressBar.style.width = "";
      progressBar.classList.add("indeterminate");
      analysisStartTime = performance.now();
      progressTimer.textContent = "0.0 s";
      analysisTimerHandle = setInterval(() => {
        const elapsed = ((performance.now() - analysisStartTime) / 1000).toFixed(1);
        progressTimer.textContent = `${elapsed} s`;
      }, 100);
      break;

    case "completed":
      clearInterval(analysisTimerHandle);
      analysisTimerHandle = null;
      lastElapsedMs = performance.now() - analysisStartTime;
      progressLabel.textContent = "Complete";
      progressTimer.textContent = `${(lastElapsedMs / 1000).toFixed(1)} s`;
      progressBar.classList.add("completed");
      feedbackSection.classList.remove("hidden");
      resetFeedbackUI();
      break;

    case "failed":
      clearInterval(analysisTimerHandle);
      analysisTimerHandle = null;
      lastElapsedMs = performance.now() - (analysisStartTime ?? performance.now());
      progressLabel.textContent = "Failed";
      progressTimer.textContent = `${(lastElapsedMs / 1000).toFixed(1)} s`;
      progressBar.classList.add("failed");
      feedbackSection.classList.add("hidden");
      break;

    case "idle":
    default:
      clearInterval(analysisTimerHandle);
      analysisTimerHandle = null;
      progressSection.classList.add("hidden");
      feedbackSection.classList.add("hidden");
      lastResult = null;
      break;
  }
}

// ── F2: Feedback & local calibration ────────────────────────────────────

const FEEDBACK_STORAGE_KEY = "imageproof_feedback_log";

function resetFeedbackUI() {
  feedbackCorrect.classList.remove("selected");
  feedbackIncorrect.classList.remove("selected");
  correctionPanel.classList.add("hidden");
  feedbackAck.classList.add("hidden");
  feedbackAck.textContent = "";
}

function loadFeedbackLog() {
  try {
    const raw = localStorage.getItem(FEEDBACK_STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveFeedbackEntry(entry) {
  const log = loadFeedbackLog();
  log.push(entry);
  // Keep last 500 entries to bound storage
  if (log.length > 500) log.splice(0, log.length - 500);
  try {
    localStorage.setItem(FEEDBACK_STORAGE_KEY, JSON.stringify(log));
  } catch {
    // Storage full — silently drop oldest
  }
}

function buildDiagnosticPayload(entry) {
  // F2: privacy-preserving diagnostic — contains ONLY scores, classification,
  // reason codes, feedback, and timing. Never contains image data or file names.
  return {
    version: 1,
    timestamp: entry.timestamp,
    classification: entry.classification,
    authenticity_score: entry.authenticity_score,
    reason_codes: entry.reason_codes,
    layer_contributions: entry.layer_contributions,
    elapsed_ms: entry.elapsed_ms,
    feedback: entry.feedback,
    correction: entry.correction ?? null,
  };
}

function recordFeedback(isCorrect, correction) {
  if (!lastResult) return;

  const entry = {
    timestamp: new Date().toISOString(),
    classification: lastResult.classification ?? "Unknown",
    authenticity_score: lastResult.authenticity_score ?? null,
    reason_codes: lastResult.reason_codes ?? [],
    layer_contributions: lastResult.layer_contributions ?? {},
    elapsed_ms: Math.round(lastElapsedMs),
    feedback: isCorrect ? "correct" : "incorrect",
    correction: correction ?? null,
  };

  saveFeedbackEntry(entry);

  // If user opted in to diagnostics, build (but don't transmit — no endpoint yet)
  if (diagnosticOptIn.checked) {
    const payload = buildDiagnosticPayload(entry);
    // Future: POST payload to optional telemetry endpoint.
    // For now, log to console so developers can inspect it.
    console.info("[ImageProof Diagnostic]", payload);
  }

  feedbackAck.textContent = isCorrect
    ? "Thanks \u2014 feedback recorded."
    : `Thanks \u2014 correction to "${correction}" recorded.`;
  feedbackAck.classList.remove("hidden");
}

// Restore opt-in preference from localStorage
try {
  diagnosticOptIn.checked = localStorage.getItem("imageproof_diagnostic_optin") === "true";
} catch {
  // ignore
}
diagnosticOptIn?.addEventListener("change", () => {
  try {
    localStorage.setItem("imageproof_diagnostic_optin", diagnosticOptIn.checked ? "true" : "false");
  } catch {
    // ignore
  }
});

feedbackCorrect?.addEventListener("click", () => {
  feedbackCorrect.classList.add("selected");
  feedbackIncorrect.classList.remove("selected");
  correctionPanel.classList.add("hidden");
  recordFeedback(true, null);
});

feedbackIncorrect?.addEventListener("click", () => {
  feedbackIncorrect.classList.add("selected");
  feedbackCorrect.classList.remove("selected");
  correctionPanel.classList.remove("hidden");
  feedbackAck.classList.add("hidden");
});

submitCorrection?.addEventListener("click", () => {
  const correction = correctionSelect.value;
  recordFeedback(false, correction);
  correctionPanel.classList.add("hidden");
});

// ── UI helpers ──────────────────────────────────────────────────────────

function setResult(confidence, justification) {
  confidenceValue.textContent = confidence;
  justificationValue.textContent = justification;
}

function enableVerifyIfReady() {
  // F1: also block when analysis is already running (prevent duplicates)
  verifyBtn.disabled = !(isWasmReady && selectedFile && analysisState !== "running");
}

function setSelectedFile(file) {
  selectedFile = file;

  if (previewUrl) {
    URL.revokeObjectURL(previewUrl);
    previewUrl = null;
  }

  if (!selectedFile) {
    dropZone.classList.remove("has-image");
    dropPrompt.classList.remove("hidden");
    clearBtn.classList.add("hidden");
    previewImage.removeAttribute("src");
    previewImage.classList.add("hidden");
    fileInput.value = "";
    enableVerifyIfReady();
    return;
  }

  previewUrl = URL.createObjectURL(selectedFile);
  previewImage.src = previewUrl;
  dropZone.classList.add("has-image");
  dropPrompt.classList.add("hidden");
  previewImage.classList.remove("hidden");
  clearBtn.classList.remove("hidden");
  enableVerifyIfReady();
}

function formatConfidence(result) {
  const scoreValue = Number(result?.authenticity_score);
  if (!Number.isFinite(scoreValue)) {
    return "Unavailable";
  }

  const bounded = Math.max(0, Math.min(1, scoreValue));
  const classification = result?.classification ?? "Unknown";

  if (classification === "Authentic") {
    return `${Math.round(bounded * 100)}%`;
  }

  if (classification === "Suspicious") {
    // H1: linear inversion — confidence that the image is edited increases
    // as authenticity_score decreases. Previous parabolic formula distorted
    // scores (peaked at 0.5, dropped at extremes).
    return `${Math.round((1 - bounded) * 100)}%`;
  }

  if (classification === "Synthetic") {
    return `${Math.round((1 - bounded) * 100)}%`;
  }

  return `${Math.round(bounded * 100)}%`;
}

function formatJustification(result) {
  const classification = result?.classification ?? "Unknown";

  if (classification === "Authentic") {
    return "This image is real.";
  }

  if (classification === "Suspicious") {
    return "This image is edited.";
  }

  if (classification === "Synthetic") {
    return "This image is more likely AI generated.";
  }

  return "Deep analysis could not classify this image.";
}

async function bootstrap() {
  try {
    await init();
    isWasmReady = true;
    setResult("Ready", "Drop an image and click Verify (deep analysis). ");
    enableVerifyIfReady();
  } catch (error) {
    setResult("Unavailable", `WASM init failed: ${String(error)}`);
    verifyBtn.disabled = true;
  }
}

fileInput.addEventListener("change", (event) => {
  const target = event.target;
  setSelectedFile(target.files?.[0] ?? null);
});

dropZone?.addEventListener("click", () => fileInput.click());

dropZone?.addEventListener("keydown", (event) => {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    fileInput.click();
  }
});

dropZone?.addEventListener("dragover", (event) => {
  event.preventDefault();
  dropZone.classList.add("drag-over");
});

dropZone?.addEventListener("dragleave", () => {
  dropZone.classList.remove("drag-over");
});

dropZone?.addEventListener("drop", (event) => {
  event.preventDefault();
  dropZone.classList.remove("drag-over");

  const file = event.dataTransfer?.files?.[0] ?? null;
  if (file && file.type.startsWith("image/")) {
    setSelectedFile(file);
  }
});

clearBtn?.addEventListener("click", () => {
  setSelectedFile(null);
  setAnalysisState("idle");
  setResult("—", "Load an image and verify to see a result.");
});

verifyBtn.addEventListener("click", async () => {
  // F1: prevent duplicate analysis
  if (!selectedFile || analysisState === "running") {
    return;
  }

  verifyBtn.disabled = true;
  setAnalysisState("running");
  setResult("Running…", "Deep analysis in progress.");

  try {
    const bytes = new Uint8Array(await selectedFile.arrayBuffer());
    let response;

    if (worker && workerReady) {
      // H8: offload to Web Worker — main thread stays responsive
      response = await verifyViaWorker(bytes, false);
    } else {
      // Fallback: run on main thread (blocks UI but still works)
      response = await verifyFallback(bytes, false);
    }

    lastResult = response;
    setAnalysisState("completed");
    setResult(formatConfidence(response), formatJustification(response));
  } catch (error) {
    lastResult = null;
    setAnalysisState("failed");
    setResult("Unavailable", `Verification failed: ${String(error)}`);
  } finally {
    enableVerifyIfReady();
  }
});

bootstrap();
