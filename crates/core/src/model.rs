use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerificationClass {
    Authentic,
    Suspicious,
    Synthetic,
    Indeterminate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionMode {
    Fast,
    Deep,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReasonCode {
    SysInsuff001,
    SysDegrad001,
    SigFreq001,
    PhyPrnu001,
    HybEla001,
    SemClass001,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayerLatencyMs {
    pub signal: u32,
    pub physical: u32,
    pub hybrid: u32,
    pub semantic: u32,
    pub fusion: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayerContributionScores {
    pub signal: f32,
    pub physical: f32,
    pub hybrid: f32,
    pub semantic: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThresholdProfile {
    pub synthetic_min: f32,
    pub synthetic_margin: f32,
    pub suspicious_min: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub authenticity_score: f32,
    pub classification: VerificationClass,
    pub reason_codes: Vec<ReasonCode>,
    pub layer_reasons: Vec<(String, Vec<ReasonCode>)>,
    pub layer_contributions: LayerContributionScores,
    pub threshold_profile: ThresholdProfile,
    pub latency_ms: LayerLatencyMs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub image_bytes: Vec<u8>,
    pub execution_mode: ExecutionMode,
}
