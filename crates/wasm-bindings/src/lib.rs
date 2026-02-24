use imageproof_core::{
    verify, ExecutionMode, HardwareTier, VerifyError, VerifyRequest,
};
use wasm_bindgen::prelude::*;

/// One-time initialization: install a panic hook that forwards Rust panics
/// to `console.error` so they are visible in browser DevTools instead of
/// being swallowed as opaque "unreachable" WASM traps.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn verify_image(image_bytes: &[u8], fast_mode: bool) -> Result<JsValue, JsValue> {
    let request = VerifyRequest {
        image_bytes: image_bytes.to_vec(),
        execution_mode: if fast_mode {
            ExecutionMode::Fast
        } else {
            ExecutionMode::Deep
        },
        hardware_tier: HardwareTier::CpuOnly,
    };

    match verify(request) {
        Ok(result) => serde_wasm_bindgen::to_value(&result)
            .map_err(|err| JsValue::from_str(&format!("serialization error: {err}"))),
        Err(err) => Err(to_js_error(err)),
    }
}

fn to_js_error(err: VerifyError) -> JsValue {
    JsValue::from_str(&err.to_string())
}
