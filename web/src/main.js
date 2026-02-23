import init, { verify_image } from "../pkg/imageproof_wasm_bindings.js";

const fileInput = document.getElementById("fileInput");
const modeSelect = document.getElementById("modeSelect");
const verifyBtn = document.getElementById("verifyBtn");
const resultEl = document.getElementById("result");

let selectedFile = null;

function formatVerificationResult(result, mode) {
  const classification = result?.classification ?? "Unknown";
  const scoreValue = Number(result?.authenticity_score);
  const score = Number.isFinite(scoreValue) ? scoreValue.toFixed(3) : "n/a";

  const reasonCodes = Array.isArray(result?.reason_codes)
    ? result.reason_codes
    : [];

  const layerReasons = Array.isArray(result?.layer_reasons)
    ? result.layer_reasons
    : [];

  const latency = result?.latency_ms ?? {};
  const latencyParts = [
    `signal=${latency.signal ?? 0}`,
    `physical=${latency.physical ?? 0}`,
    `hybrid=${latency.hybrid ?? 0}`,
    `semantic=${latency.semantic ?? 0}`,
    `fusion=${latency.fusion ?? 0}`,
  ];

  const lines = [
    `Execution mode: ${mode.toUpperCase()}`,
    `Classification: ${classification}`,
    `Authenticity score: ${score}`,
    `Reason codes: ${reasonCodes.length > 0 ? reasonCodes.join(", ") : "none"}`,
    `Latency ms: ${latencyParts.join(", ")}`,
  ];

  if (layerReasons.length > 0) {
    lines.push("Layer reasons:");
    for (const entry of layerReasons) {
      const layerName = Array.isArray(entry) ? entry[0] : "unknown";
      const layerCodes = Array.isArray(entry?.[1]) ? entry[1] : [];
      lines.push(`- ${layerName}: ${layerCodes.length > 0 ? layerCodes.join(", ") : "none"}`);
    }
  }

  lines.push("", "Raw result:", JSON.stringify(result, null, 2));
  return lines.join("\n");
}

async function bootstrap() {
  try {
    await init();
    resultEl.textContent = "WASM module loaded. Select an image and click Verify.";
  } catch (error) {
    resultEl.textContent = `WASM init failed: ${String(error)}`;
    verifyBtn.disabled = true;
  }
}

fileInput.addEventListener("change", (event) => {
  const target = event.target;
  selectedFile = target.files?.[0] ?? null;
  verifyBtn.disabled = !selectedFile;
});

verifyBtn.addEventListener("click", async () => {
  if (!selectedFile) {
    return;
  }

  verifyBtn.disabled = true;
  resultEl.textContent = "Running verification...";

  try {
    const bytes = new Uint8Array(await selectedFile.arrayBuffer());
    const mode = modeSelect?.value === "deep" ? "deep" : "fast";
    const fastMode = mode === "fast";
    const response = verify_image(bytes, fastMode);
    resultEl.textContent = formatVerificationResult(response, mode);
  } catch (error) {
    resultEl.textContent = `Verification error: ${String(error)}`;
  } finally {
    verifyBtn.disabled = false;
  }
});

bootstrap();
