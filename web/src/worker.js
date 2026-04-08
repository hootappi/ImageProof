// H8: Web Worker — run WASM verification off the main thread so the UI
// remains responsive during deep analysis of large images.

import init, { verify_image } from "../pkg/imageproof_wasm_bindings.js";

let wasmReady = false;

async function bootstrap() {
  try {
    await init();
    wasmReady = true;
    self.postMessage({ type: "ready" });
  } catch (error) {
    self.postMessage({ type: "init-error", error: String(error) });
  }
}

self.addEventListener("message", (event) => {
  const { type, id, bytes, fastMode } = event.data;

  if (type !== "verify") {
    return;
  }

  if (!wasmReady) {
    self.postMessage({ type: "result", id, error: "WASM not initialized" });
    return;
  }

  try {
    const result = verify_image(new Uint8Array(bytes), fastMode ?? false);
    self.postMessage({ type: "result", id, result });
  } catch (error) {
    self.postMessage({ type: "result", id, error: String(error) });
  }
});

bootstrap();
