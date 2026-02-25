import init, { verify_image } from "../pkg/imageproof_wasm_bindings.js";

const fileInput = document.getElementById("fileInput");
const dropZone = document.getElementById("dropZone");
const dropPrompt = document.getElementById("dropPrompt");
const previewImage = document.getElementById("previewImage");
const verifyBtn = document.getElementById("verifyBtn");
const clearBtn = document.getElementById("clearBtn");
const confidenceValue = document.getElementById("confidenceValue");
const justificationValue = document.getElementById("justificationValue");

let selectedFile = null;
let previewUrl = null;
let isWasmReady = false;

function setResult(confidence, justification) {
  confidenceValue.textContent = confidence;
  justificationValue.textContent = justification;
}

function enableVerifyIfReady() {
  verifyBtn.disabled = !(isWasmReady && selectedFile);
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
  setResult("—", "Load an image and verify to see a result.");
});

verifyBtn.addEventListener("click", async () => {
  if (!selectedFile) {
    return;
  }

  verifyBtn.disabled = true;
  setResult("Running...", "Deep analysis in progress.");

  try {
    const bytes = new Uint8Array(await selectedFile.arrayBuffer());
    const response = verify_image(bytes, false);
    setResult(formatConfidence(response), formatJustification(response));
  } catch (error) {
    setResult("Unavailable", `Verification failed: ${String(error)}`);
  } finally {
    enableVerifyIfReady();
  }
});

bootstrap();
