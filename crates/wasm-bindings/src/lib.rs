use imageproof_core::{
    verify, ExecutionMode, HardwareTier, VerifyError, VerifyRequest,
};
use wasm_bindgen::prelude::*;

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
