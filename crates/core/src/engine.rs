use crate::model::{
    ExecutionMode, LayerLatencyMs, ReasonCode, VerificationClass, VerificationResult,
    VerifyRequest,
};

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("image payload is empty")]
    EmptyInput,
    #[error("verification engine is not implemented yet")]
    NotImplemented,
}

pub fn verify(request: VerifyRequest) -> Result<VerificationResult, VerifyError> {
    if request.image_bytes.is_empty() {
        return Err(VerifyError::EmptyInput);
    }

    match request.execution_mode {
        ExecutionMode::Fast => Ok(VerificationResult {
            authenticity_score: 0.5,
            classification: VerificationClass::Indeterminate,
            reason_codes: vec![ReasonCode::SysInsuff001],
            layer_reasons: vec![(
                "system".to_string(),
                vec![ReasonCode::SysInsuff001],
            )],
            latency_ms: LayerLatencyMs {
                signal: 0,
                physical: 0,
                hybrid: 0,
                semantic: 0,
                fusion: 1,
            },
        }),
        ExecutionMode::Deep => Err(VerifyError::NotImplemented),
    }
}
