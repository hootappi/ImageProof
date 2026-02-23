use crate::model::{VerificationResult, VerifyRequest};

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

    Err(VerifyError::NotImplemented)
}
