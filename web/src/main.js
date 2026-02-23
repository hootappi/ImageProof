import init, { verify_image } from "../pkg/imageproof_wasm_bindings.js";

const fileInput = document.getElementById("fileInput");
const verifyBtn = document.getElementById("verifyBtn");
const resultEl = document.getElementById("result");

let selectedFile = null;

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
    const response = verify_image(bytes, true);
    resultEl.textContent = JSON.stringify(response, null, 2);
  } catch (error) {
    resultEl.textContent = `Verification error: ${String(error)}`;
  } finally {
    verifyBtn.disabled = false;
  }
});

bootstrap();
