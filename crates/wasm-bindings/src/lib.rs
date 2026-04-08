use imageproof_core::{
    verify_bytes, ExecutionMode, VerifyError,
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
    // M6: call verify_bytes directly — no Vec<u8> copy needed.
    let mode = if fast_mode {
        ExecutionMode::Fast
    } else {
        ExecutionMode::Deep
    };

    match verify_bytes(image_bytes, mode) {
        Ok(result) => serde_wasm_bindgen::to_value(&result)
            .map_err(|err| JsValue::from_str(&format!("serialization error: {err}"))),
        Err(err) => Err(to_js_error(err)),
    }
}

fn to_js_error(err: VerifyError) -> JsValue {
    JsValue::from_str(&err.to_string())
}
