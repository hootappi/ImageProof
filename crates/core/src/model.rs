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
pub enum HardwareTier {
    CpuOnly,
    CpuSimd,
    WebGpu,
    Native,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub authenticity_score: f32,
    pub classification: VerificationClass,
    pub reason_codes: Vec<ReasonCode>,
    pub layer_reasons: Vec<(String, Vec<ReasonCode>)>,
    pub latency_ms: LayerLatencyMs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub image_bytes: Vec<u8>,
    pub execution_mode: ExecutionMode,
    pub hardware_tier: HardwareTier,
}
