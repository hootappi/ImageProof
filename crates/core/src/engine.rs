use crate::model::{
    ExecutionMode, LayerLatencyMs, ReasonCode, VerificationClass, VerificationResult,
    VerifyRequest,
};

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("image payload is empty")]
    EmptyInput,
    #[error("only deep analysis is available in the current scaffold")]
    NotImplemented,
}

pub fn verify(request: VerifyRequest) -> Result<VerificationResult, VerifyError> {
    if request.image_bytes.is_empty() {
        return Err(VerifyError::EmptyInput);
    }

    match request.execution_mode {
        ExecutionMode::Fast => Err(VerifyError::NotImplemented),
        ExecutionMode::Deep => {
            let checksum: u32 = request
                .image_bytes
                .iter()
                .take(64)
                .map(|byte| u32::from(*byte))
                .sum();

            let outcome = checksum % 3;

            let (classification, authenticity_score, reason_codes, layer_reasons) =
                match outcome {
                    0 => (
                        VerificationClass::Authentic,
                        0.87,
                        vec![ReasonCode::PhyPrnu001],
                        vec![("physical".to_string(), vec![ReasonCode::PhyPrnu001])],
                    ),
                    1 => (
                        VerificationClass::Suspicious,
                        0.46,
                        vec![ReasonCode::HybEla001],
                        vec![("hybrid".to_string(), vec![ReasonCode::HybEla001])],
                    ),
                    _ => (
                        VerificationClass::Synthetic,
                        0.12,
                        vec![ReasonCode::SemClass001],
                        vec![("semantic".to_string(), vec![ReasonCode::SemClass001])],
                    ),
                };

            Ok(VerificationResult {
                authenticity_score,
                classification,
                reason_codes,
                layer_reasons,
                latency_ms: LayerLatencyMs {
                    signal: 96,
                    physical: 101,
                    hybrid: 124,
                    semantic: 138,
                    fusion: 21,
                },
            })
        }
    }
}
