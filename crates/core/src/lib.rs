use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerificationClass {
    Authentic,
    Suspicious,
    Synthetic,
    Indeterminate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub authenticity_score: f32,
    pub classification: VerificationClass,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub image_bytes: Vec<u8>,
    pub fast_mode: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("image payload is empty")]
    EmptyInput,
    #[error("verification engine is not implemented yet")]
    NotImplemented,
}

pub fn verify(_request: VerifyRequest) -> Result<VerificationResult, VerifyError> {
    Err(VerifyError::NotImplemented)
}
