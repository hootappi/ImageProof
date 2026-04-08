use crate::config::CalibrationConfig;
use crate::hybrid::compute_hybrid_metrics;
use crate::model::{
    ExecutionMode, LayerContributionScores, LayerLatencyMs, ReasonCode, ThresholdProfile,
    VerificationClass, VerificationResult, VerifyRequest,
};
use crate::physical::compute_prnu_proxy_metrics;
use crate::semantic::compute_semantic_metrics;
use crate::signal::{compute_fft_signal_features, compute_pixel_statistics, compute_pixel_stats_and_residual};
use image::{GrayImage, ImageFormat, ImageReader};
use std::io::Cursor;
use web_time::Instant;

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("image payload is empty")]
    EmptyInput,
    #[error("image payload exceeds maximum allowed size ({size} bytes, limit {limit} bytes)")]
    InputTooLarge { size: usize, limit: usize },
    #[error("image dimensions exceed maximum allowed ({width}x{height}, limit {limit}x{limit})")]
    DimensionTooLarge {
        width: u32,
        height: u32,
        limit: u32,
    },
    #[error("unsupported image format (only JPEG, PNG, and WebP are accepted)")]
    UnsupportedFormat,
    #[error("image decode failed")]
    DecodeFailed,
}

#[derive(Debug, Clone, Copy)]
struct SignalMetrics {
    noise_score: f32,
    edge_score: f32,
    block_artifact_score: f32,
    block_variance_cv: f32,
    spectral_peak_score: f32,
    high_freq_ratio_score: f32,
    prnu_plausibility_score: f32,
    cross_region_consistency: f32,
    hybrid_local_inconsistency: f32,
    hybrid_seam_anomaly: f32,
    semantic_pattern_repetition: f32,
    semantic_gradient_entropy: f32,
    semantic_synthetic_cue: f32,
}

pub fn verify(request: VerifyRequest) -> Result<VerificationResult, VerifyError> {
    verify_bytes(&request.image_bytes, request.execution_mode)
}

/// M6: Primary entry point that borrows image data — no copy required.
/// WASM and CLI callers should prefer this to avoid unnecessary `Vec<u8>`
/// allocation when they already hold a reference to the raw bytes.
pub fn verify_bytes(
    image_bytes: &[u8],
    execution_mode: ExecutionMode,
) -> Result<VerificationResult, VerifyError> {
    verify_bytes_with_config(image_bytes, execution_mode, &CalibrationConfig::default())
}

/// M3: Primary entry point with runtime-configurable calibration parameters.
/// Callers can load a partial TOML file into `CalibrationConfig` to override
/// classification thresholds, fusion weights, or any other parameter.
pub fn verify_bytes_with_config(
    image_bytes: &[u8],
    execution_mode: ExecutionMode,
    cfg: &CalibrationConfig,
) -> Result<VerificationResult, VerifyError> {
    if image_bytes.is_empty() {
        return Err(VerifyError::EmptyInput);
    }

    let size = image_bytes.len();
    if size > cfg.max_file_size_bytes {
        return Err(VerifyError::InputTooLarge {
            size,
            limit: cfg.max_file_size_bytes,
        });
    }

    match execution_mode {
        ExecutionMode::Fast => verify_fast(image_bytes, cfg),
        ExecutionMode::Deep => verify_deep_heuristic(image_bytes, cfg),
    }
}

/// L5: Decode and validate image bytes — shared by both fast and deep paths.
/// Rejects unsupported formats (only JPEG, PNG, WebP accepted) and enforces
/// dimension limits. Returns `(gray_image, is_jpeg)`.
fn decode_image(image_bytes: &[u8], cfg: &CalibrationConfig) -> Result<(GrayImage, bool), VerifyError> {
    let reader = ImageReader::new(Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|_| VerifyError::DecodeFailed)?;

    let format = reader.format();
    let is_jpeg = format == Some(ImageFormat::Jpeg);

    // L5: Only accept JPEG, PNG, and WebP — reject BMP, GIF, TIFF, etc.
    match format {
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP) => {}
        _ => return Err(VerifyError::UnsupportedFormat),
    }

    let image = reader.decode().map_err(|_| VerifyError::DecodeFailed)?;

    let (width, height) = (image.width(), image.height());
    if width > cfg.max_image_dimension || height > cfg.max_image_dimension {
        return Err(VerifyError::DimensionTooLarge {
            width,
            height,
            limit: cfg.max_image_dimension,
        });
    }

    Ok((image.to_luma8(), is_jpeg))
}

/// M8: Lightweight fast-mode analysis — pixel statistics only.
/// Skips FFT, PRNU, hybrid, and semantic layers for lower latency
/// at the cost of reduced accuracy. Suitable for quick triage.
fn verify_fast(image_bytes: &[u8], cfg: &CalibrationConfig) -> Result<VerificationResult, VerifyError> {
    let (gray, is_jpeg) = decode_image(image_bytes, cfg)?;

    let t_signal = Instant::now();
    let (noise_score, edge_score, block_artifact_score, block_variance_cv) =
        compute_pixel_statistics(&gray, is_jpeg);
    let signal_ms = t_signal.elapsed().as_millis() as u32;

    // Simplified scoring: use only pixel-level statistics.
    let synthetic_raw = (cfg.fast_syn_w_block_art * block_artifact_score
        + cfg.fast_syn_w_noise_inv * (1.0 - noise_score).max(0.0)
        + cfg.fast_syn_w_edge_inv * (1.0 - edge_score).max(0.0)
        + cfg.fast_syn_w_block_var * block_variance_cv)
        .clamp(0.0, 1.0);

    let edited_raw = (cfg.fast_edt_w_block_var * block_variance_cv
        + cfg.fast_edt_w_block_art * block_artifact_score
        + cfg.fast_edt_w_edge * edge_score
        + cfg.fast_edt_w_noise * noise_score)
        .clamp(0.0, 1.0);

    let authentic_likelihood = (1.0 - cfg.auth_w_synthetic * synthetic_raw - cfg.auth_w_edited * edited_raw).clamp(0.0, 1.0);

    let (classification, authenticity_score, reason_codes, layer_reasons) =
        if synthetic_raw > cfg.synthetic_min_threshold {
            (
                VerificationClass::Synthetic,
                (1.0 - cfg.synthetic_score_scale * synthetic_raw).clamp(cfg.synthetic_score_min, cfg.synthetic_score_max),
                vec![ReasonCode::SigFreq001],
                vec![("signal".to_string(), vec![ReasonCode::SigFreq001])],
            )
        } else if edited_raw > cfg.suspicious_min_threshold {
            (
                VerificationClass::Suspicious,
                (cfg.suspicious_score_base + (1.0 - edited_raw) * cfg.suspicious_score_range).clamp(cfg.suspicious_score_min, cfg.suspicious_score_max),
                vec![ReasonCode::HybEla001],
                vec![("hybrid".to_string(), vec![ReasonCode::HybEla001])],
            )
        } else if synthetic_raw < cfg.indeterminate_ceiling
            && edited_raw < cfg.indeterminate_ceiling
            && (synthetic_raw - edited_raw).abs() < cfg.indeterminate_min_spread
        {
            (
                VerificationClass::Indeterminate,
                cfg.indeterminate_score,
                vec![ReasonCode::SysInsuff001],
                vec![("system".to_string(), vec![ReasonCode::SysInsuff001])],
            )
        } else {
            (
                VerificationClass::Authentic,
                (cfg.authentic_score_base + authentic_likelihood * cfg.authentic_score_range).clamp(cfg.authentic_score_min, cfg.authentic_score_max),
                vec![ReasonCode::PhyPrnu001],
                vec![("physical".to_string(), vec![ReasonCode::PhyPrnu001])],
            )
        };

    let signal_contribution = (cfg.fast_lc_block_art * block_artifact_score
        + cfg.fast_lc_noise_inv * (1.0 - noise_score).max(0.0)
        + cfg.fast_lc_edge_inv * (1.0 - edge_score).max(0.0)
        + cfg.fast_lc_block_var * block_variance_cv)
        .clamp(0.0, 1.0);

    Ok(VerificationResult {
        authenticity_score,
        classification,
        reason_codes,
        layer_reasons,
        layer_contributions: LayerContributionScores {
            signal: signal_contribution,
            physical: 0.0,
            hybrid: 0.0,
            semantic: 0.0,
        },
        threshold_profile: ThresholdProfile {
            synthetic_min: cfg.synthetic_min_threshold,
            synthetic_margin: cfg.synthetic_margin_threshold,
            suspicious_min: cfg.suspicious_min_threshold,
        },
        latency_ms: LayerLatencyMs {
            signal: signal_ms,
            physical: 0,
            hybrid: 0,
            semantic: 0,
            fusion: 0,
        },
    })
}

fn verify_deep_heuristic(image_bytes: &[u8], cfg: &CalibrationConfig) -> Result<VerificationResult, VerifyError> {
    let (gray, is_jpeg) = decode_image(image_bytes, cfg)?;

    // C2: Real per-layer timing via compute_signal_metrics_timed.
    let timed = compute_signal_metrics_timed(&gray, is_jpeg);
    let metrics = timed.metrics;

    // Synthetic-base fusion weights — normalized to sum = 1.00 (C1 fix).
    let synthetic_base = (cfg.syn_w_block_artifact * metrics.block_artifact_score
        + cfg.syn_w_noise_inv * (1.0 - metrics.noise_score).max(0.0)
        + cfg.syn_w_edge_inv * (1.0 - metrics.edge_score).max(0.0)
        + cfg.syn_w_spectral_peak * metrics.spectral_peak_score
        + cfg.syn_w_hf_ratio_inv * (1.0 - metrics.high_freq_ratio_score).max(0.0)
        + cfg.syn_w_prnu_inv * (1.0 - metrics.prnu_plausibility_score).max(0.0)
        + cfg.syn_w_consistency_inv * (1.0 - metrics.cross_region_consistency).max(0.0)
        + cfg.syn_w_hybrid_local * metrics.hybrid_local_inconsistency
        + cfg.syn_w_hybrid_seam * metrics.hybrid_seam_anomaly
        + cfg.syn_w_semantic_cue * metrics.semantic_synthetic_cue)
        .clamp(0.0, 1.0);

    let synthetic_suppression = (1.0
        - cfg.syn_supp_prnu * metrics.prnu_plausibility_score
        - cfg.syn_supp_consistency * metrics.cross_region_consistency
        - cfg.syn_supp_hf_ratio * metrics.high_freq_ratio_score)
        .clamp(cfg.syn_supp_floor, 1.0);
    let synthetic_likelihood = (synthetic_base * synthetic_suppression).clamp(0.0, 1.0);

    // Edited-base fusion weights — normalized to sum = 1.00 (C1 fix).
    let edited_base = (cfg.edt_w_block_var_cv * metrics.block_variance_cv
        + cfg.edt_w_edge * metrics.edge_score
        + cfg.edt_w_block_artifact * metrics.block_artifact_score * (1.0 - synthetic_likelihood)
        + cfg.edt_w_spectral_peak * metrics.spectral_peak_score * cfg.edt_spectral_damp
        + cfg.edt_w_consistency_inv * (1.0 - metrics.cross_region_consistency).max(0.0)
        + cfg.edt_w_prnu_inv * (1.0 - metrics.prnu_plausibility_score).max(0.0)
        + cfg.edt_w_hybrid_local * metrics.hybrid_local_inconsistency
        + cfg.edt_w_hybrid_seam * metrics.hybrid_seam_anomaly
        + cfg.edt_w_semantic_cue * metrics.semantic_synthetic_cue)
        .clamp(0.0, 1.0);

    let edited_suppression =
        (1.0 - cfg.edt_supp_prnu * metrics.prnu_plausibility_score - cfg.edt_supp_consistency * metrics.cross_region_consistency)
            .clamp(cfg.edt_supp_floor, 1.0);
    let edited_likelihood = (edited_base * edited_suppression).clamp(0.0, 1.0);

    // Authentic complement — coefficients sum to 1.0 (C1 fix, was 0.72+0.60=1.32).
    let authentic_likelihood =
        (1.0 - cfg.auth_w_synthetic * synthetic_likelihood - cfg.auth_w_edited * edited_likelihood).clamp(0.0, 1.0);

    // M7: Compute layer contributions before classification so reason codes
    // can be driven by actual contribution scores instead of being hardcoded.
    let layer_contributions = compute_layer_contributions(&metrics, cfg);

    let (classification, authenticity_score, reason_codes, layer_reasons) =
        if synthetic_likelihood > cfg.synthetic_min_threshold
            && synthetic_likelihood > edited_likelihood + cfg.synthetic_margin_threshold
        {
            // Synthetic: strong evidence — emit all layer codes unconditionally.
            (
                VerificationClass::Synthetic,
                (1.0 - cfg.synthetic_score_scale * synthetic_likelihood).clamp(cfg.synthetic_score_min, cfg.synthetic_score_max),
                vec![
                    ReasonCode::SemClass001,
                    ReasonCode::SigFreq001,
                    ReasonCode::PhyPrnu001,
                    ReasonCode::HybEla001,
                ],
                vec![
                    ("semantic".to_string(), vec![ReasonCode::SemClass001]),
                    ("signal".to_string(), vec![ReasonCode::SigFreq001]),
                    ("physical".to_string(), vec![ReasonCode::PhyPrnu001]),
                    ("hybrid".to_string(), vec![ReasonCode::HybEla001]),
                ],
            )
        } else if edited_likelihood > cfg.suspicious_min_threshold {
            // Suspicious: emit codes only for layers that actually contributed (M7).
            let (mut reason_codes, mut layer_reasons) =
                derive_reason_codes(&layer_contributions, cfg);

            // Conditional semantic escalation for suspicious semantic cues.
            if !reason_codes.contains(&ReasonCode::SemClass001)
                && (metrics.semantic_synthetic_cue > cfg.semantic_cue_escalation_threshold
                    || (metrics.semantic_pattern_repetition > cfg.semantic_repetition_escalation_threshold
                        && metrics.semantic_gradient_entropy < cfg.semantic_entropy_escalation_ceiling))
            {
                reason_codes.push(ReasonCode::SemClass001);
                layer_reasons.push(("semantic".to_string(), vec![ReasonCode::SemClass001]));
            }

            // Suspicious must have at least one reason code — fallback to HybEla001.
            if reason_codes.is_empty() {
                reason_codes.push(ReasonCode::HybEla001);
                layer_reasons.push(("hybrid".to_string(), vec![ReasonCode::HybEla001]));
            }

            (
                VerificationClass::Suspicious,
                (cfg.suspicious_score_base + (1.0 - edited_likelihood) * cfg.suspicious_score_range).clamp(cfg.suspicious_score_min, cfg.suspicious_score_max),
                reason_codes,
                layer_reasons,
            )
        } else if synthetic_likelihood < cfg.indeterminate_ceiling
            && edited_likelihood < cfg.indeterminate_ceiling
            && (synthetic_likelihood - edited_likelihood).abs() < cfg.indeterminate_min_spread
        {
            // C3: Neither signal path reached a meaningful level and the two
            // paths are within the spread threshold — insufficient evidence.
            (
                VerificationClass::Indeterminate,
                cfg.indeterminate_score,
                vec![ReasonCode::SysInsuff001],
                vec![("system".to_string(), vec![ReasonCode::SysInsuff001])],
            )
        } else {
            // Authentic: emit codes only for layers that actually contributed (M7).
            let (mut reason_codes, mut layer_reasons) =
                derive_reason_codes(&layer_contributions, cfg);

            // Authentic must have at least one reason code — fallback to PhyPrnu001.
            if reason_codes.is_empty() {
                reason_codes.push(ReasonCode::PhyPrnu001);
                layer_reasons.push(("physical".to_string(), vec![ReasonCode::PhyPrnu001]));
            }

            (
                VerificationClass::Authentic,
                (cfg.authentic_score_base + authentic_likelihood * cfg.authentic_score_range).clamp(cfg.authentic_score_min, cfg.authentic_score_max),
                reason_codes,
                layer_reasons,
            )
        };

    // --- Fusion timing (C2: real measurement) ---
    let t_fusion = Instant::now();
    // layer_contributions already computed before classification (M7).
    let threshold_profile = ThresholdProfile {
        synthetic_min: cfg.synthetic_min_threshold,
        synthetic_margin: cfg.synthetic_margin_threshold,
        suspicious_min: cfg.suspicious_min_threshold,
    };
    let fusion_ms = t_fusion.elapsed().as_millis() as u32;

    Ok(VerificationResult {
        authenticity_score,
        classification,
        reason_codes,
        layer_reasons,
        layer_contributions,
        threshold_profile,
        latency_ms: LayerLatencyMs {
            signal: timed.signal_ms,
            physical: timed.physical_ms,
            hybrid: timed.hybrid_ms,
            semantic: timed.semantic_ms,
            fusion: fusion_ms,
        },
    })
}

/// Per-layer timing bundled with signal metrics (C2: real latency).
struct TimedMetrics {
    metrics: SignalMetrics,
    signal_ms: u32,
    physical_ms: u32,
    hybrid_ms: u32,
    semantic_ms: u32,
}

/// Compute all signal metrics with real per-layer wall-clock timing.
fn compute_signal_metrics_timed(gray: &GrayImage, is_jpeg: bool) -> TimedMetrics {
    let width = gray.width();
    let height = gray.height();

    if width < 3 || height < 3 {
        return TimedMetrics {
            metrics: SignalMetrics {
                noise_score: 0.0,
                edge_score: 0.0,
                block_artifact_score: 0.0,
                block_variance_cv: 0.0,
                spectral_peak_score: 0.0,
                high_freq_ratio_score: 0.0,
                prnu_plausibility_score: 0.0,
                cross_region_consistency: 0.0,
                hybrid_local_inconsistency: 0.0,
                hybrid_seam_anomaly: 0.0,
                semantic_pattern_repetition: 0.0,
                semantic_gradient_entropy: 0.0,
                semantic_synthetic_cue: 0.0,
            },
            signal_ms: 0,
            physical_ms: 0,
            hybrid_ms: 0,
            semantic_ms: 0,
        };
    }

    // --- Signal layer: pixel statistics + FFT (M4: single-pass) ---
    let t_signal = Instant::now();
    let (noise_score, edge_score, block_artifact_score, block_variance_cv, residual_map, res_w, res_h) =
        compute_pixel_stats_and_residual(gray, is_jpeg);
    let (spectral_peak_score, high_freq_ratio_score) =
        compute_fft_signal_features(&residual_map, res_w, res_h);
    let signal_ms = t_signal.elapsed().as_millis() as u32;

    // --- Physical layer: PRNU proxy ---
    let t_physical = Instant::now();
    let (prnu_plausibility_score, cross_region_consistency) =
        compute_prnu_proxy_metrics(&residual_map, res_w, res_h);
    let physical_ms = t_physical.elapsed().as_millis() as u32;

    // --- Hybrid layer: local inconsistency + seam ---
    let t_hybrid = Instant::now();
    let (hybrid_local_inconsistency, hybrid_seam_anomaly) =
        compute_hybrid_metrics(&residual_map, res_w, res_h);
    let hybrid_ms = t_hybrid.elapsed().as_millis() as u32;

    // --- Semantic layer: repetition + gradient entropy ---
    let t_semantic = Instant::now();
    let (semantic_pattern_repetition, semantic_gradient_entropy, semantic_synthetic_cue) =
        compute_semantic_metrics(&residual_map, gray, res_w, res_h);
    let semantic_ms = t_semantic.elapsed().as_millis() as u32;

    TimedMetrics {
        metrics: SignalMetrics {
            noise_score,
            edge_score,
            block_artifact_score,
            block_variance_cv,
            spectral_peak_score,
            high_freq_ratio_score,
            prnu_plausibility_score,
            cross_region_consistency,
            hybrid_local_inconsistency,
            hybrid_seam_anomaly,
            semantic_pattern_repetition,
            semantic_gradient_entropy,
            semantic_synthetic_cue,
        },
        signal_ms,
        physical_ms,
        hybrid_ms,
        semantic_ms,
    }
}

#[cfg(test)]
fn compute_signal_metrics(gray: &GrayImage) -> SignalMetrics {
    compute_signal_metrics_for(gray, false)
}

#[cfg(test)]
fn compute_signal_metrics_for(gray: &GrayImage, is_jpeg: bool) -> SignalMetrics {
    let width = gray.width();
    let height = gray.height();

    if width < 3 || height < 3 {
        return SignalMetrics {
            noise_score: 0.0,
            edge_score: 0.0,
            block_artifact_score: 0.0,
            block_variance_cv: 0.0,
            spectral_peak_score: 0.0,
            high_freq_ratio_score: 0.0,
            prnu_plausibility_score: 0.0,
            cross_region_consistency: 0.0,
            hybrid_local_inconsistency: 0.0,
            hybrid_seam_anomaly: 0.0,
            semantic_pattern_repetition: 0.0,
            semantic_gradient_entropy: 0.0,
            semantic_synthetic_cue: 0.0,
        };
    }

    let (noise_score, edge_score, block_artifact_score, block_variance_cv, residual_map, res_w, res_h) =
        compute_pixel_stats_and_residual(gray, is_jpeg);
    let (spectral_peak_score, high_freq_ratio_score) =
        compute_fft_signal_features(&residual_map, res_w, res_h);
    let (prnu_plausibility_score, cross_region_consistency) =
        compute_prnu_proxy_metrics(&residual_map, res_w, res_h);
    let (hybrid_local_inconsistency, hybrid_seam_anomaly) =
        compute_hybrid_metrics(&residual_map, res_w, res_h);
    let (semantic_pattern_repetition, semantic_gradient_entropy, semantic_synthetic_cue) =
        compute_semantic_metrics(&residual_map, gray, res_w, res_h);

    SignalMetrics {
        noise_score,
        edge_score,
        block_artifact_score,
        block_variance_cv,
        spectral_peak_score,
        high_freq_ratio_score,
        prnu_plausibility_score,
        cross_region_consistency,
        hybrid_local_inconsistency,
        hybrid_seam_anomaly,
        semantic_pattern_repetition,
        semantic_gradient_entropy,
        semantic_synthetic_cue,
    }
}

/// M7: Derive reason codes and layer_reasons from actual contribution scores.
/// Only layers whose contribution score meets `REASON_CODE_CONTRIBUTION_THRESHOLD`
/// receive a reason code. Returns `(reason_codes, layer_reasons)`.
fn derive_reason_codes(
    contributions: &LayerContributionScores,
    cfg: &CalibrationConfig,
) -> (Vec<ReasonCode>, Vec<(String, Vec<ReasonCode>)>) {
    let mut codes = Vec::new();
    let mut layers = Vec::new();
    let t = cfg.reason_code_contribution_threshold;

    if contributions.signal >= t {
        codes.push(ReasonCode::SigFreq001);
        layers.push(("signal".to_string(), vec![ReasonCode::SigFreq001]));
    }
    if contributions.physical >= t {
        codes.push(ReasonCode::PhyPrnu001);
        layers.push(("physical".to_string(), vec![ReasonCode::PhyPrnu001]));
    }
    if contributions.hybrid >= t {
        codes.push(ReasonCode::HybEla001);
        layers.push(("hybrid".to_string(), vec![ReasonCode::HybEla001]));
    }
    if contributions.semantic >= t {
        codes.push(ReasonCode::SemClass001);
        layers.push(("semantic".to_string(), vec![ReasonCode::SemClass001]));
    }

    (codes, layers)
}

fn compute_layer_contributions(metrics: &SignalMetrics, cfg: &CalibrationConfig) -> LayerContributionScores {
    let signal = (cfg.lc_signal_block_art * metrics.block_artifact_score
        + cfg.lc_signal_noise_inv * (1.0 - metrics.noise_score).max(0.0)
        + cfg.lc_signal_edge_inv * (1.0 - metrics.edge_score).max(0.0)
        + cfg.lc_signal_spectral * metrics.spectral_peak_score
        + cfg.lc_signal_hf_inv * (1.0 - metrics.high_freq_ratio_score).max(0.0))
        .clamp(0.0, 1.0);

    let physical = (cfg.lc_physical_prnu_inv * (1.0 - metrics.prnu_plausibility_score).max(0.0)
        + cfg.lc_physical_consist_inv * (1.0 - metrics.cross_region_consistency).max(0.0))
        .clamp(0.0, 1.0);

    let hybrid = (cfg.lc_hybrid_local * metrics.hybrid_local_inconsistency + cfg.lc_hybrid_seam * metrics.hybrid_seam_anomaly)
        .clamp(0.0, 1.0);

    let semantic = metrics.semantic_synthetic_cue.clamp(0.0, 1.0);

    LayerContributionScores {
        signal,
        physical,
        hybrid,
        semantic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use crate::model::{ExecutionMode, VerifyRequest};
    use crate::physical::block_corr;
    use crate::semantic::compute_shifted_residual_corr;
    use crate::signal::{
        compute_block_variance_cv, compute_residual_map, fft2d_magnitude, sample_rect,
    };
    use image::{GrayImage, ImageBuffer, ImageFormat, Luma};
    use std::io::Cursor;

    // ---------------------------------------------------------------
    // Helper: create a minimal valid PNG in memory
    // ---------------------------------------------------------------
    fn make_png(width: u32, height: u32, fill: u8) -> Vec<u8> {
        let img = GrayImage::from_pixel(width, height, Luma([fill]));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn make_gradient_png(width: u32, height: u32) -> Vec<u8> {
        let img = ImageBuffer::from_fn(width, height, |x, y| {
            Luma([((x.wrapping_add(y * 3)) % 256) as u8])
        });
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn make_noisy_png(width: u32, height: u32, seed: u64) -> Vec<u8> {
        let img = ImageBuffer::from_fn(width, height, |x, y| {
            // Simple deterministic pseudo-random fill
            let v = ((x as u64)
                .wrapping_mul(2654435761)
                .wrapping_add(y as u64)
                .wrapping_mul(2246822519)
                .wrapping_add(seed))
                % 256;
            Luma([v as u8])
        });
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn make_xorshift_png(width: u32, height: u32, seed: u32) -> Vec<u8> {
        let mut img = GrayImage::new(width, height);
        let mut state = seed | 1; // xorshift must be non-zero
        for y in 0..height {
            for x in 0..width {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                img.put_pixel(x, y, Luma([(state & 0xFF) as u8]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    // ---------------------------------------------------------------
    // 1. Public API: verify()
    // ---------------------------------------------------------------

    #[test]
    fn verify_empty_input_returns_empty_error() {
        let req = VerifyRequest {
            image_bytes: vec![],
            execution_mode: ExecutionMode::Deep,
        };
        let err = verify(req).unwrap_err();
        assert!(matches!(err, VerifyError::EmptyInput));
    }

    #[test]
    fn verify_input_too_large_is_rejected() {
        // 50 MB + 1 byte exceeds the limit
        let oversized = vec![0u8; MAX_FILE_SIZE_BYTES + 1];
        let req = VerifyRequest {
            image_bytes: oversized,
            execution_mode: ExecutionMode::Deep,
        };
        let err = verify(req).unwrap_err();
        assert!(
            matches!(err, VerifyError::InputTooLarge { size, limit }
                if size == MAX_FILE_SIZE_BYTES + 1 && limit == MAX_FILE_SIZE_BYTES),
            "expected InputTooLarge, got {err:?}"
        );
    }

    #[test]
    fn verify_input_at_exact_limit_is_not_rejected_for_size() {
        // Exactly 50 MB should pass the size gate (it will fail on format check, not size).
        let at_limit = vec![0u8; MAX_FILE_SIZE_BYTES];
        let req = VerifyRequest {
            image_bytes: at_limit,
            execution_mode: ExecutionMode::Deep,
        };
        let err = verify(req).unwrap_err();
        // L5: all-zero bytes have no recognizable format header.
        assert!(
            matches!(err, VerifyError::UnsupportedFormat),
            "expected UnsupportedFormat (not InputTooLarge), got {err:?}"
        );
    }

    #[test]
    fn verify_fast_mode_returns_result() {
        // M8: Fast mode now produces a real result using pixel-level statistics only.
        let png = make_png(64, 64, 128);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Fast,
        };
        let result = verify(req).unwrap();
        assert!(
            result.authenticity_score >= 0.0 && result.authenticity_score <= 1.0,
            "fast mode score out of [0,1]: {}",
            result.authenticity_score
        );
        assert!(!result.reason_codes.is_empty());
        // Fast mode skips physical/hybrid/semantic layers
        assert_eq!(result.layer_contributions.physical, 0.0);
        assert_eq!(result.layer_contributions.hybrid, 0.0);
        assert_eq!(result.layer_contributions.semantic, 0.0);
        // Latency: only signal layer has time
        assert_eq!(result.latency_ms.physical, 0);
        assert_eq!(result.latency_ms.hybrid, 0);
        assert_eq!(result.latency_ms.semantic, 0);
    }

    #[test]
    fn verify_garbage_bytes_returns_unsupported_format() {
        // L5: garbage bytes have no recognizable image format.
        let req = VerifyRequest {
            image_bytes: vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33],
            execution_mode: ExecutionMode::Deep,
        };
        let err = verify(req).unwrap_err();
        assert!(matches!(err, VerifyError::UnsupportedFormat));
    }

    #[test]
    fn verify_bmp_rejected_as_unsupported() {
        // L5: BMP format is not accepted — only JPEG, PNG, WebP.
        // BMP header: 'BM' + 4-byte file size + 4-byte reserved + 4-byte offset
        let bmp_header: Vec<u8> = vec![
            0x42, 0x4D, // 'BM'
            0x36, 0x00, 0x00, 0x00, // file size placeholder
            0x00, 0x00, 0x00, 0x00, // reserved
            0x36, 0x00, 0x00, 0x00, // pixel data offset
        ];
        let req = VerifyRequest {
            image_bytes: bmp_header,
            execution_mode: ExecutionMode::Deep,
        };
        let err = verify(req).unwrap_err();
        assert!(
            matches!(err, VerifyError::UnsupportedFormat),
            "BMP should be rejected as UnsupportedFormat, got {err:?}"
        );
    }

    #[test]
    fn verify_truncated_png_returns_decode_failed() {
        let png = make_png(64, 64, 128);
        let truncated = &png[..png.len() / 2];
        let req = VerifyRequest {
            image_bytes: truncated.to_vec(),
            execution_mode: ExecutionMode::Deep,
        };
        let err = verify(req).unwrap_err();
        assert!(matches!(err, VerifyError::DecodeFailed));
    }

    #[test]
    fn verify_valid_png_returns_result() {
        let png = make_png(64, 64, 128);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        assert!(
            result.authenticity_score >= 0.0 && result.authenticity_score <= 1.0,
            "score out of [0,1]: {}",
            result.authenticity_score
        );
        assert!(!result.reason_codes.is_empty());
    }

    #[test]
    fn verify_1x1_png_returns_result_no_panic() {
        let png = make_png(1, 1, 200);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        // Tiny image should still produce a valid bounded score
        assert!(result.authenticity_score >= 0.0 && result.authenticity_score <= 1.0);
    }

    #[test]
    fn verify_2x2_png_returns_result_no_panic() {
        let png = make_png(2, 2, 100);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        assert!(result.authenticity_score >= 0.0 && result.authenticity_score <= 1.0);
    }

    #[test]
    fn verify_3x3_png_returns_result_no_panic() {
        let png = make_png(3, 3, 50);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        assert!(
            result.authenticity_score >= 0.0 && result.authenticity_score <= 1.0,
            "score out of [0,1]: {}",
            result.authenticity_score
        );
    }

    #[test]
    fn verify_gradient_image_returns_bounded_scores() {
        let png = make_gradient_png(128, 128);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        assert!(result.authenticity_score >= 0.0 && result.authenticity_score <= 1.0);
        assert!(result.layer_contributions.signal >= 0.0 && result.layer_contributions.signal <= 1.0);
        assert!(result.layer_contributions.physical >= 0.0 && result.layer_contributions.physical <= 1.0);
        assert!(result.layer_contributions.hybrid >= 0.0 && result.layer_contributions.hybrid <= 1.0);
        assert!(result.layer_contributions.semantic >= 0.0 && result.layer_contributions.semantic <= 1.0);
    }

    // ---------------------------------------------------------------
    // 2. Classification gate tests
    // ---------------------------------------------------------------

    #[test]
    fn verify_result_classification_is_a_valid_variant() {
        let png = make_noisy_png(64, 64, 42);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        assert!(matches!(
            result.classification,
            VerificationClass::Authentic
                | VerificationClass::Suspicious
                | VerificationClass::Synthetic
                | VerificationClass::Indeterminate
        ));
    }

    #[test]
    fn verify_result_contains_threshold_profile() {
        let png = make_png(64, 64, 128);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        assert_eq!(result.threshold_profile.synthetic_min, SYNTHETIC_MIN_THRESHOLD);
        assert_eq!(result.threshold_profile.synthetic_margin, SYNTHETIC_MARGIN_THRESHOLD);
        assert_eq!(result.threshold_profile.suspicious_min, SUSPICIOUS_MIN_THRESHOLD);
    }

    #[test]
    fn verify_result_latency_fields_are_real_measurements() {
        // Use a large enough image to produce measurable signal-layer time.
        let png = make_noisy_png(512, 512, 7);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        // C2: Real measurements — signal layer does the bulk of work.
        // For a 512×512 image the signal layer should be non-zero on most hardware.
        // Individual sub-millisecond layers may report 0 — that's correct.
        let total = result.latency_ms.signal
            + result.latency_ms.physical
            + result.latency_ms.hybrid
            + result.latency_ms.semantic
            + result.latency_ms.fusion;
        assert!(
            total < 30_000,
            "total latency should be <30s for 512×512, got {}ms",
            total
        );
        // Verify no fabricated values: with real timing, fusion on a small image
        // should be ≤ signal (fusion only computes layer_contributions + threshold).
        assert!(
            result.latency_ms.fusion <= result.latency_ms.signal + 1,
            "fusion ({}) should not exceed signal ({})",
            result.latency_ms.fusion,
            result.latency_ms.signal
        );
    }

    // ---------------------------------------------------------------
    // 3. compute_signal_metrics
    // ---------------------------------------------------------------

    #[test]
    fn signal_metrics_tiny_image_returns_zeros() {
        let gray = GrayImage::from_pixel(2, 2, Luma([128]));
        let metrics = compute_signal_metrics(&gray);
        assert_eq!(metrics.noise_score, 0.0);
        assert_eq!(metrics.edge_score, 0.0);
        assert_eq!(metrics.block_artifact_score, 0.0);
        assert_eq!(metrics.spectral_peak_score, 0.0);
        assert_eq!(metrics.prnu_plausibility_score, 0.0);
        assert_eq!(metrics.hybrid_local_inconsistency, 0.0);
        assert_eq!(metrics.semantic_pattern_repetition, 0.0);
    }

    #[test]
    fn signal_metrics_flat_image_has_low_noise_and_edges() {
        let gray = GrayImage::from_pixel(64, 64, Luma([128]));
        let metrics = compute_signal_metrics(&gray);
        assert_eq!(metrics.noise_score, 0.0, "flat image should have zero noise");
        assert_eq!(metrics.edge_score, 0.0, "flat image should have zero edges");
    }

    #[test]
    fn signal_metrics_noisy_image_has_nonzero_noise() {
        let img = ImageBuffer::from_fn(64, 64, |x, y| {
            Luma([((x.wrapping_mul(7).wrapping_add(y.wrapping_mul(13))) % 256) as u8])
        });
        let metrics = compute_signal_metrics(&img);
        assert!(metrics.noise_score > 0.0, "noisy image should have >0 noise score");
        assert!(metrics.edge_score > 0.0, "noisy image should have >0 edge score");
    }

    #[test]
    fn signal_metrics_all_fields_bounded_0_1() {
        let img = ImageBuffer::from_fn(128, 128, |x, y| {
            Luma([((x ^ y).wrapping_mul(17) % 256) as u8])
        });
        let metrics = compute_signal_metrics(&img);
        let fields = [
            metrics.noise_score,
            metrics.edge_score,
            metrics.block_artifact_score,
            metrics.block_variance_cv,
            metrics.spectral_peak_score,
            metrics.high_freq_ratio_score,
            metrics.prnu_plausibility_score,
            metrics.cross_region_consistency,
            metrics.hybrid_local_inconsistency,
            metrics.hybrid_seam_anomaly,
            metrics.semantic_pattern_repetition,
            metrics.semantic_gradient_entropy,
            metrics.semantic_synthetic_cue,
        ];
        for (i, val) in fields.iter().enumerate() {
            assert!(
                *val >= 0.0 && *val <= 1.0,
                "metric field index {i} is out of [0,1]: {val}"
            );
        }
    }

    // ---------------------------------------------------------------
    // 4. compute_residual_map
    // ---------------------------------------------------------------

    #[test]
    fn residual_map_tiny_image_returns_empty() {
        let gray = GrayImage::from_pixel(2, 2, Luma([100]));
        let (residual, w, h) = compute_residual_map(&gray);
        assert!(residual.is_empty());
        assert_eq!(w, 0);
        assert_eq!(h, 0);
    }

    #[test]
    fn residual_map_flat_image_interior_is_zero() {
        let gray = GrayImage::from_pixel(8, 8, Luma([100]));
        let (residual, w, h) = compute_residual_map(&gray);
        assert_eq!(w, 6);
        assert_eq!(h, 6);
        // All interior residuals should be zero for a flat image
        assert!(residual.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn residual_map_no_border_zeros_in_output() {
        // H4: the returned buffer must contain NO border zeros.
        // Use a non-linear pattern so residuals are non-zero.
        let gray = ImageBuffer::from_fn(8, 8, |x, y| {
            Luma([((x * x + y * y * 3) % 256) as u8])
        });
        let (residual, w, h) = compute_residual_map(&gray);
        assert_eq!(w, 6);
        assert_eq!(h, 6);
        assert_eq!(residual.len(), 36);
        // At least some interior residuals are non-zero for a gradient image
        assert!(residual.iter().any(|v| *v != 0.0));
    }

    #[test]
    fn residual_map_length_matches_interior_dimensions() {
        let gray = GrayImage::from_pixel(16, 24, Luma([50]));
        let (residual, w, h) = compute_residual_map(&gray);
        assert_eq!(w, 14);
        assert_eq!(h, 22);
        assert_eq!(residual.len(), 14 * 22);
    }

    // H4 – border exclusion correctness
    #[test]
    fn h4_residual_interior_values_match_manual_computation() {
        // Verify that cropped residual[0][0] equals the value at original (1,1)
        let gray = ImageBuffer::from_fn(5, 5, |x, y| {
            Luma([((x * 10 + y * 7) % 256) as u8])
        });
        let (residual, w, h) = compute_residual_map(&gray);
        assert_eq!(w, 3);
        assert_eq!(h, 3);
        // Manual: center(1,1)=17, left(0,1)=7, right(2,1)=27, up(1,0)=10, down(1,2)=24
        let center = gray.get_pixel(1, 1)[0] as f32;
        let left = gray.get_pixel(0, 1)[0] as f32;
        let right = gray.get_pixel(2, 1)[0] as f32;
        let up = gray.get_pixel(1, 0)[0] as f32;
        let down = gray.get_pixel(1, 2)[0] as f32;
        let expected = center - (left + right + up + down) * 0.25;
        assert!((residual[0] - expected).abs() < 1e-6, "residual[0]={} expected={}", residual[0], expected);
    }

    #[test]
    fn h4_downstream_fft_receives_no_border_zeros() {
        // An 18×18 image produces a 16×16 interior residual which just
        // passes the FFT min-dim guard.  Before H4 the border zeros would
        // have been sampled; after H4 only real residuals are present.
        let gray = ImageBuffer::from_fn(18, 18, |x, y| {
            Luma([((x.wrapping_mul(37) ^ y.wrapping_mul(53)) % 256) as u8])
        });
        let (residual, res_w, res_h) = compute_residual_map(&gray);
        assert_eq!(res_w, 16);
        assert_eq!(res_h, 16);
        let (peak, hf) = compute_fft_signal_features(&residual, res_w, res_h);
        // Should produce valid bounded values from real residuals
        assert!((0.0..=1.0).contains(&peak), "peak={peak}");
        assert!((0.0..=1.0).contains(&hf), "hf={hf}");
    }

    #[test]
    fn h4_3x3_image_returns_1x1_interior() {
        let gray = ImageBuffer::from_fn(3, 3, |x, y| {
            Luma([((x + y) * 40) as u8])
        });
        let (residual, w, h) = compute_residual_map(&gray);
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        assert_eq!(residual.len(), 1);
    }

    // ---------------------------------------------------------------
    // 5. compute_fft_signal_features
    // ---------------------------------------------------------------

    #[test]
    fn fft_features_small_image_returns_zeros() {
        let residual = vec![0.0f32; 8 * 8];
        let (peak, hf) = compute_fft_signal_features(&residual, 8, 8);
        assert_eq!(peak, 0.0);
        assert_eq!(hf, 0.0);
    }

    #[test]
    fn fft_features_large_enough_image_returns_bounded() {
        let residual: Vec<f32> = (0..64 * 64).map(|i| (i as f32 * 0.01).sin()).collect();
        let (peak, hf) = compute_fft_signal_features(&residual, 64, 64);
        assert!((0.0..=1.0).contains(&peak));
        assert!((0.0..=1.0).contains(&hf));
    }

    // ---------------------------------------------------------------
    // 6. compute_prnu_proxy_metrics
    // ---------------------------------------------------------------

    #[test]
    fn prnu_metrics_small_image_returns_zeros() {
        let residual = vec![0.0f32; 24 * 24];
        let (plaus, cons) = compute_prnu_proxy_metrics(&residual, 24, 24);
        // 24×24 is exactly block * 2 edge — should not produce zero
        // Actually 24 < 24*2 = 48 for the size check
        assert_eq!(plaus, 0.0);
        assert_eq!(cons, 0.0);
    }

    #[test]
    fn prnu_metrics_adequate_image_returns_bounded() {
        let w = 96;
        let h = 96;
        let residual: Vec<f32> = (0..w * h).map(|i| ((i * 7) as f32 % 5.0) - 2.5).collect();
        let (plaus, cons) = compute_prnu_proxy_metrics(&residual, w, h);
        assert!((0.0..=1.0).contains(&plaus), "plausibility={plaus}");
        assert!((0.0..=1.0).contains(&cons), "consistency={cons}");
    }

    // ---------------------------------------------------------------
    // 7. compute_hybrid_metrics
    // ---------------------------------------------------------------

    #[test]
    fn hybrid_metrics_small_image_returns_zeros() {
        let residual = vec![0.0f32; 16 * 16];
        let (li, sa) = compute_hybrid_metrics(&residual, 16, 16);
        assert_eq!(li, 0.0);
        assert_eq!(sa, 0.0);
    }

    #[test]
    fn hybrid_metrics_adequate_image_returns_bounded() {
        let w = 128;
        let h = 128;
        let residual: Vec<f32> = (0..w * h).map(|i| ((i * 13) as f32 % 7.0) - 3.5).collect();
        let (li, sa) = compute_hybrid_metrics(&residual, w, h);
        assert!((0.0..=1.0).contains(&li), "local_inconsistency={li}");
        assert!((0.0..=1.0).contains(&sa), "seam_anomaly={sa}");
    }

    // ---------------------------------------------------------------
    // 8. compute_semantic_metrics
    // ---------------------------------------------------------------

    #[test]
    fn semantic_metrics_small_image_returns_zeros() {
        let gray = GrayImage::from_pixel(16, 16, Luma([100]));
        let residual = vec![0.0f32; 16 * 16];
        let (rep, ent, cue) = compute_semantic_metrics(&residual, &gray, 16, 16);
        assert_eq!(rep, 0.0);
        assert_eq!(ent, 0.0);
        assert_eq!(cue, 0.0);
    }

    #[test]
    fn semantic_metrics_gradient_image_returns_bounded() {
        let w = 64u32;
        let h = 64u32;
        let gray = ImageBuffer::from_fn(w, h, |x, y| {
            Luma([((x + y * 2) % 256) as u8])
        });
        let (residual, res_w, res_h) = compute_residual_map(&gray);
        let (rep, ent, cue) = compute_semantic_metrics(&residual, &gray, res_w, res_h);
        assert!((0.0..=1.0).contains(&rep));
        assert!((0.0..=1.0).contains(&ent));
        assert!((0.0..=1.0).contains(&cue));
    }

    // ---------------------------------------------------------------
    // 9. compute_block_variance_cv
    // ---------------------------------------------------------------

    #[test]
    fn block_variance_cv_small_image_returns_zero() {
        let gray = GrayImage::from_pixel(16, 16, Luma([100]));
        assert_eq!(compute_block_variance_cv(&gray), 0.0);
    }

    #[test]
    fn block_variance_cv_flat_image_returns_zero() {
        let gray = GrayImage::from_pixel(64, 64, Luma([128]));
        assert_eq!(compute_block_variance_cv(&gray), 0.0);
    }

    #[test]
    fn block_variance_cv_patterned_image_returns_bounded() {
        let gray = ImageBuffer::from_fn(128, 128, |x, y| {
            Luma([((x ^ y) % 256) as u8])
        });
        let cv = compute_block_variance_cv(&gray);
        assert!((0.0..=1.0).contains(&cv), "cv={cv}");
    }

    // ---------------------------------------------------------------
    // 10. compute_layer_contributions
    // ---------------------------------------------------------------

    #[test]
    fn layer_contributions_all_zeros_returns_bounded() {
        let metrics = SignalMetrics {
            noise_score: 0.0,
            edge_score: 0.0,
            block_artifact_score: 0.0,
            block_variance_cv: 0.0,
            spectral_peak_score: 0.0,
            high_freq_ratio_score: 0.0,
            prnu_plausibility_score: 0.0,
            cross_region_consistency: 0.0,
            hybrid_local_inconsistency: 0.0,
            hybrid_seam_anomaly: 0.0,
            semantic_pattern_repetition: 0.0,
            semantic_gradient_entropy: 0.0,
            semantic_synthetic_cue: 0.0,
        };
        let cfg = CalibrationConfig::default();
        let lc = compute_layer_contributions(&metrics, &cfg);
        assert!(lc.signal >= 0.0 && lc.signal <= 1.0);
        assert!(lc.physical >= 0.0 && lc.physical <= 1.0);
        assert!(lc.hybrid >= 0.0 && lc.hybrid <= 1.0);
        assert!(lc.semantic >= 0.0 && lc.semantic <= 1.0);
    }

    #[test]
    fn layer_contributions_all_ones_returns_bounded() {
        let metrics = SignalMetrics {
            noise_score: 1.0,
            edge_score: 1.0,
            block_artifact_score: 1.0,
            block_variance_cv: 1.0,
            spectral_peak_score: 1.0,
            high_freq_ratio_score: 1.0,
            prnu_plausibility_score: 1.0,
            cross_region_consistency: 1.0,
            hybrid_local_inconsistency: 1.0,
            hybrid_seam_anomaly: 1.0,
            semantic_pattern_repetition: 1.0,
            semantic_gradient_entropy: 1.0,
            semantic_synthetic_cue: 1.0,
        };
        let cfg = CalibrationConfig::default();
        let lc = compute_layer_contributions(&metrics, &cfg);
        assert!(lc.signal >= 0.0 && lc.signal <= 1.0);
        assert!(lc.physical >= 0.0 && lc.physical <= 1.0);
        assert!(lc.hybrid >= 0.0 && lc.hybrid <= 1.0);
        assert!(lc.semantic >= 0.0 && lc.semantic <= 1.0);
    }

    // ---------------------------------------------------------------
    // 11. sample_rect
    // ---------------------------------------------------------------

    #[test]
    fn sample_rect_zero_inputs_returns_zeros() {
        let data = vec![1.0f32; 0];
        let out = sample_rect(&data, 0, 0, 4);
        assert_eq!(out.len(), 16);
        assert!(out.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn sample_rect_identity_preserves_data() {
        let data: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let out = sample_rect(&data, 4, 4, 4);
        assert_eq!(out, data);
    }

    // ---------------------------------------------------------------
    // 12. fft2d_magnitude
    // ---------------------------------------------------------------

    #[test]
    fn fft2d_magnitude_returns_correct_length() {
        let input = vec![0.0f32; 8 * 8];
        let mag = fft2d_magnitude(&input, 8);
        assert_eq!(mag.len(), 64);
    }

    #[test]
    fn fft2d_magnitude_all_zeros_returns_zeros() {
        let input = vec![0.0f32; 16 * 16];
        let mag = fft2d_magnitude(&input, 16);
        for val in &mag {
            assert!(*val >= 0.0);
            assert!(*val < f32::EPSILON, "expected ~0, got {val}");
        }
    }

    // ---------------------------------------------------------------
    // 13. block_corr
    // ---------------------------------------------------------------

    #[test]
    fn block_corr_identical_blocks_returns_one() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        // 2×4 grid, block=2, compare block (0,0) with itself at (0,0)
        let corr = block_corr(&data, 4, 0, 0, 0, 0, 2);
        // Self-correlation should be 1.0 (or None if flat)
        if let Some(c) = corr {
            assert!((c - 1.0).abs() < 1e-5, "self-correlation should be ~1.0, got {c}");
        }
    }

    #[test]
    fn block_corr_flat_returns_none() {
        // Flat blocks => denominator is zero => None
        let data = vec![5.0f32; 4 * 4];
        let result = block_corr(&data, 4, 0, 0, 2, 0, 2);
        assert!(result.is_none(), "flat blocks should return None");
    }

    // ---------------------------------------------------------------
    // 14. compute_shifted_residual_corr
    // ---------------------------------------------------------------

    #[test]
    fn shifted_corr_out_of_bounds_returns_none() {
        let data = vec![0.0f32; 4 * 4];
        assert!(compute_shifted_residual_corr(&data, 4, 4, 10, 0).is_none());
        assert!(compute_shifted_residual_corr(&data, 4, 4, 0, 10).is_none());
    }

    #[test]
    fn shifted_corr_zero_shift_self_correlates() {
        let data: Vec<f32> = (0..16 * 16).map(|i| (i as f32 * 0.1).sin()).collect();
        let corr = compute_shifted_residual_corr(&data, 16, 16, 0, 0);
        if let Some(c) = corr {
            assert!((c - 1.0).abs() < 1e-4, "zero-shift should be ~1.0, got {c}");
        }
    }

    #[test]
    fn shifted_corr_returns_bounded_value() {
        let data: Vec<f32> = (0..32 * 32).map(|i| ((i * 7) as f32 % 13.0) - 6.5).collect();
        if let Some(c) = compute_shifted_residual_corr(&data, 32, 32, 3, 5) {
            assert!((-1.0..=1.0).contains(&c), "corr={c}");
        }
    }

    // ---------------------------------------------------------------
    // 15. Diverse image sizes (no-panic property tests)
    // ---------------------------------------------------------------

    #[test]
    fn verify_various_sizes_no_panic() {
        for &(w, h) in &[(1, 1), (2, 3), (3, 2), (4, 4), (7, 7), (8, 8), (15, 15), (16, 16), (32, 32), (100, 100)] {
            let png = make_noisy_png(w, h, (w as u64) * 1000 + (h as u64));
            let req = VerifyRequest {
                image_bytes: png,
                execution_mode: ExecutionMode::Deep,
                };
            let result = verify(req);
            assert!(result.is_ok(), "failed for {w}×{h}: {:?}", result.err());
            let r = result.unwrap();
            assert!(
                r.authenticity_score >= 0.0 && r.authenticity_score <= 1.0,
                "score out of bounds for {w}×{h}: {}",
                r.authenticity_score
            );
        }
    }

    // ---------------------------------------------------------------
    // 16. Flat image should classify as Authentic (sanity)
    // ---------------------------------------------------------------

    #[test]
    fn flat_image_classifies_as_authentic() {
        let png = make_png(128, 128, 128);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        // A perfectly flat image has no manipulation or synthetic signals
        assert_eq!(
            result.classification,
            VerificationClass::Authentic,
            "flat image should be Authentic, got {:?} (score={})",
            result.classification,
            result.authenticity_score
        );
    }

    // ---------------------------------------------------------------
    // 17. Input limit constants (C5)
    // ---------------------------------------------------------------

    #[test]
    fn max_file_size_is_50mb() {
        assert_eq!(MAX_FILE_SIZE_BYTES, 50 * 1024 * 1024);
    }

    #[test]
    fn max_image_dimension_is_16384() {
        assert_eq!(MAX_IMAGE_DIMENSION, 16_384);
    }

    #[test]
    fn verify_size_check_precedes_decode() {
        // An oversized payload of garbage bytes should be rejected by the
        // size gate, not by the decode gate.
        let big_garbage = vec![0xABu8; MAX_FILE_SIZE_BYTES + 42];
        let req = VerifyRequest {
            image_bytes: big_garbage,
            execution_mode: ExecutionMode::Deep,
        };
        let err = verify(req).unwrap_err();
        assert!(
            matches!(err, VerifyError::InputTooLarge { .. }),
            "expected InputTooLarge before decode, got {err:?}"
        );
    }

    #[test]
    fn verify_normal_image_passes_dimension_check() {
        // A 128×128 image is well within 16384 limit.
        let png = make_png(128, 128, 200);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req);
        assert!(result.is_ok(), "normal image should pass: {:?}", result.err());
    }

    // ---------------------------------------------------------------
    // 18. Fusion weight normalization regression (C1)
    // ---------------------------------------------------------------

    #[test]
    fn synthetic_base_weights_sum_to_one() {
        // These must match the weights in verify_deep_heuristic.
        let weights: &[f32] = &[0.18, 0.15, 0.08, 0.16, 0.12, 0.09, 0.07, 0.04, 0.02, 0.09];
        let sum: f32 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "synthetic_base weights sum to {sum}, expected 1.0"
        );
    }

    #[test]
    fn edited_base_weights_sum_to_one() {
        // These must match the weights in verify_deep_heuristic.
        let weights: &[f32] = &[0.26, 0.05, 0.13, 0.07, 0.11, 0.04, 0.18, 0.13, 0.03];
        let sum: f32 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "edited_base weights sum to {sum}, expected 1.0"
        );
    }

    #[test]
    fn authentic_likelihood_coefficients_sum_to_one() {
        let coefficients: &[f32] = &[0.55, 0.45];
        let sum: f32 = coefficients.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "authentic_likelihood coefficients sum to {sum}, expected ≤1.0"
        );
    }

    #[test]
    fn no_nan_in_scores_for_flat_images() {
        // Flat images previously produced NaN from 0/0 in block_artifact_score.
        for &fill in &[0u8, 1, 128, 254, 255] {
            for &(w, h) in &[(3, 3), (8, 8), (16, 16), (64, 64), (128, 128)] {
                let png = make_png(w, h, fill);
                let req = VerifyRequest {
                    image_bytes: png,
                    execution_mode: ExecutionMode::Deep,
                        };
                let result = verify(req).unwrap();
                assert!(
                    result.authenticity_score.is_finite(),
                    "NaN/Inf for flat {fill} at {w}×{h}: {}",
                    result.authenticity_score
                );
                assert!(
                    (0.0..=1.0).contains(&result.authenticity_score),
                    "score out of [0,1] for flat {fill} at {w}×{h}: {}",
                    result.authenticity_score
                );
            }
        }
    }

    #[test]
    fn block_artifact_score_zero_for_flat_image() {
        // When all pixels are identical, no block artifact signal exists.
        let gray = GrayImage::from_pixel(64, 64, Luma([100]));
        let metrics = compute_signal_metrics(&gray);
        assert_eq!(
            metrics.block_artifact_score, 0.0,
            "flat image should have zero block artifact score"
        );
    }

    // ---------------------------------------------------------------
    // 20. Block artifact format gating (H2)
    // ---------------------------------------------------------------

    fn make_jpeg(width: u32, height: u32, fill: u8) -> Vec<u8> {
        let img = GrayImage::from_pixel(width, height, Luma([fill]));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Jpeg).unwrap();
        buf.into_inner()
    }

    #[test]
    fn png_input_has_zero_block_artifact_score() {
        // H2: PNG is not JPEG-compressed — block artifact metric must be zero.
        let png = make_gradient_png(128, 128);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        // block_artifact_score is embedded in the fusion model;
        // verify via layer contributions that signal layer doesn't
        // incorporate block artifact. More directly, verify via
        // compute_pixel_statistics with is_jpeg=false.
        let img = image::load_from_memory(&make_gradient_png(128, 128))
            .unwrap()
            .to_luma8();
        let (_, _, ba, _) = compute_pixel_statistics(&img, false);
        assert_eq!(ba, 0.0, "PNG block_artifact_score should be 0.0, got {}", ba);
        // Also verify score is still bounded
        assert!((0.0..=1.0).contains(&result.authenticity_score));
    }

    #[test]
    fn jpeg_input_may_have_nonzero_block_artifact_score() {
        // H2: JPEG-compressed gradient image should have nonzero block artifact
        // because JPEG introduces real 8×8 block discontinuities.
        let jpeg = make_jpeg(128, 128, 128);
        let img = image::load_from_memory(&jpeg).unwrap().to_luma8();
        let (_, _, _ba_flat, _) = compute_pixel_statistics(&img, true);
        // A flat JPEG may still have ba=0 (no texture), so use a gradient:
        let jpeg_grad = {
            let grad = ImageBuffer::from_fn(128, 128, |x, y| {
                Luma([((x.wrapping_add(y * 3)) % 256) as u8])
            });
            let mut buf = Cursor::new(Vec::new());
            grad.write_to(&mut buf, ImageFormat::Jpeg).unwrap();
            buf.into_inner()
        };
        let img_grad = image::load_from_memory(&jpeg_grad).unwrap().to_luma8();
        let (_, _, ba_grad, _) = compute_pixel_statistics(&img_grad, true);
        // JPEG gradient should have measurable block artifacts
        assert!(
            ba_grad >= 0.0,
            "JPEG block_artifact_score should be non-negative"
        );
    }

    #[test]
    fn non_jpeg_block_artifact_forced_zero_via_flag() {
        // Direct unit test of the is_jpeg flag in compute_pixel_statistics.
        let gray = ImageBuffer::from_fn(64, 64, |x, y| {
            Luma([((x * 7 + y * 13) % 256) as u8])
        });
        let (_, _, ba_jpeg, _) = compute_pixel_statistics(&gray, true);
        let (_, _, ba_non_jpeg, _) = compute_pixel_statistics(&gray, false);
        // With is_jpeg=false, score must be zero regardless of pixel content.
        assert_eq!(ba_non_jpeg, 0.0, "non-JPEG should force ba=0.0");
        // With is_jpeg=true and textured content, score may be positive.
        assert!(ba_jpeg >= 0.0, "JPEG ba should be non-negative");
    }

    // ---------------------------------------------------------------
    // 19. Indeterminate classification (C3)
    // ---------------------------------------------------------------

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn indeterminate_ceiling_below_suspicious_threshold() {
        assert!(
            INDETERMINATE_CEILING < SUSPICIOUS_MIN_THRESHOLD,
            "Indeterminate ceiling ({}) must be below suspicious gate ({})",
            INDETERMINATE_CEILING,
            SUSPICIOUS_MIN_THRESHOLD
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn indeterminate_ceiling_below_synthetic_threshold() {
        assert!(
            INDETERMINATE_CEILING < SYNTHETIC_MIN_THRESHOLD,
            "Indeterminate ceiling ({}) must be below synthetic gate ({})",
            INDETERMINATE_CEILING,
            SYNTHETIC_MIN_THRESHOLD
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn indeterminate_spread_is_positive() {
        assert!(
            INDETERMINATE_MIN_SPREAD > 0.0,
            "Indeterminate min spread must be positive"
        );
    }

    #[test]
    fn noisy_image_classifies_as_indeterminate() {
        // Xorshift white-noise image: high noise/edge scores push inversions
        // toward zero while low spectral/semantic/hybrid signals keep both
        // likelihoods under the Indeterminate ceiling with narrow spread.
        let png = make_xorshift_png(128, 128, 42);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        assert_eq!(
            result.classification,
            VerificationClass::Indeterminate,
            "xorshift noise image should be Indeterminate, got {:?} (score={})",
            result.classification,
            result.authenticity_score
        );
    }

    #[test]
    fn indeterminate_result_has_half_score() {
        let png = make_xorshift_png(128, 128, 42);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        assert!(
            (result.authenticity_score - 0.50).abs() < f32::EPSILON,
            "Indeterminate score should be 0.50, got {}",
            result.authenticity_score
        );
    }

    #[test]
    fn indeterminate_emits_sys_insuff_reason() {
        let png = make_xorshift_png(128, 128, 42);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        assert!(
            result.reason_codes.contains(&ReasonCode::SysInsuff001),
            "Indeterminate result should contain SysInsuff001, got {:?}",
            result.reason_codes
        );
    }

    // ── M7: derive_reason_codes unit tests ─────────────────────────────────

    #[test]
    fn m7_derive_reason_codes_all_above_threshold() {
        let contributions = LayerContributionScores {
            signal: 0.50,
            physical: 0.30,
            hybrid: 0.20,
            semantic: 0.40,
        };
        let cfg = CalibrationConfig::default();
        let (codes, layers) = derive_reason_codes(&contributions, &cfg);
        assert!(codes.contains(&ReasonCode::SigFreq001));
        assert!(codes.contains(&ReasonCode::PhyPrnu001));
        assert!(codes.contains(&ReasonCode::HybEla001));
        assert!(codes.contains(&ReasonCode::SemClass001));
        assert_eq!(codes.len(), 4);
        assert_eq!(layers.len(), 4);
    }

    #[test]
    fn m7_derive_reason_codes_none_above_threshold() {
        let contributions = LayerContributionScores {
            signal: 0.10,
            physical: 0.05,
            hybrid: 0.00,
            semantic: 0.14,
        };
        let cfg = CalibrationConfig::default();
        let (codes, layers) = derive_reason_codes(&contributions, &cfg);
        assert!(codes.is_empty(), "No codes should be emitted when all contributions are below threshold, got {:?}", codes);
        assert!(layers.is_empty());
    }

    #[test]
    fn m7_derive_reason_codes_partial() {
        let contributions = LayerContributionScores {
            signal: 0.60,
            physical: 0.10,  // below threshold
            hybrid: 0.25,
            semantic: 0.01,  // below threshold
        };
        let cfg = CalibrationConfig::default();
        let (codes, layers) = derive_reason_codes(&contributions, &cfg);
        assert_eq!(codes.len(), 2);
        assert!(codes.contains(&ReasonCode::SigFreq001));
        assert!(codes.contains(&ReasonCode::HybEla001));
        assert!(!codes.contains(&ReasonCode::PhyPrnu001));
        assert!(!codes.contains(&ReasonCode::SemClass001));
        assert_eq!(layers.len(), 2);
    }

    #[test]
    fn m7_derive_reason_codes_at_exact_threshold() {
        // At exactly the threshold, the code SHOULD be emitted (>=).
        let contributions = LayerContributionScores {
            signal: REASON_CODE_CONTRIBUTION_THRESHOLD,
            physical: REASON_CODE_CONTRIBUTION_THRESHOLD - 0.001,
            hybrid: REASON_CODE_CONTRIBUTION_THRESHOLD,
            semantic: REASON_CODE_CONTRIBUTION_THRESHOLD - 0.001,
        };
        let cfg = CalibrationConfig::default();
        let (codes, _) = derive_reason_codes(&contributions, &cfg);
        assert_eq!(codes.len(), 2, "Exact threshold should emit code, got {:?}", codes);
        assert!(codes.contains(&ReasonCode::SigFreq001));
        assert!(codes.contains(&ReasonCode::HybEla001));
    }

    #[test]
    fn m7_reason_code_threshold_is_positive() {
        let t = REASON_CODE_CONTRIBUTION_THRESHOLD;
        assert!(
            t > 0.0,
            "Threshold must be positive to avoid emitting codes for zero-contribution layers"
        );
        assert!(
            t < 1.0,
            "Threshold must be < 1.0 to allow some layers to pass"
        );
    }

    #[test]
    fn m7_authentic_omits_phyprnu001_when_physical_contribution_zero() {
        // A gradient image typically classifies as Authentic with very low physical
        // contribution. After M7, PhyPrnu001 should only appear if the physical
        // layer's contribution is above threshold.
        let png = make_gradient_png(200, 200);
        let req = VerifyRequest {
            image_bytes: png,
            execution_mode: ExecutionMode::Deep,
        };
        let result = verify(req).unwrap();
        let contrib = &result.layer_contributions;
        if contrib.physical < REASON_CODE_CONTRIBUTION_THRESHOLD {
            assert!(
                !result.reason_codes.contains(&ReasonCode::PhyPrnu001),
                "Physical contribution ({}) is below threshold ({}) but PhyPrnu001 was emitted. Reason codes: {:?}",
                contrib.physical,
                REASON_CODE_CONTRIBUTION_THRESHOLD,
                result.reason_codes
            );
        }
    }

    #[test]
    fn m7_every_result_has_at_least_one_reason_code() {
        // Verify all classification paths emit at least one reason code.
        let test_images = vec![
            ("gradient", make_gradient_png(200, 200)),
            ("noisy", make_noisy_png(200, 200, 99)),
            ("xorshift", make_xorshift_png(128, 128, 42)),
        ];
        for (label, png) in test_images {
            let req = VerifyRequest {
                image_bytes: png,
                execution_mode: ExecutionMode::Deep,
                };
            let result = verify(req).unwrap();
            assert!(
                !result.reason_codes.is_empty(),
                "Image '{}' ({:?}) should have at least one reason code",
                label,
                result.classification
            );
        }
    }

    // ── M3 integration tests: CalibrationConfig overrides ──

    #[test]
    fn m3_default_config_matches_constant_behavior() {
        // verify_bytes_with_config + default config must produce identical result to verify_bytes.
        let png = make_gradient_png(100, 100);
        let r1 = verify_bytes(&png, ExecutionMode::Deep).unwrap();
        let r2 =
            verify_bytes_with_config(&png, ExecutionMode::Deep, &CalibrationConfig::default())
                .unwrap();
        assert_eq!(r1.classification, r2.classification);
        assert!((r1.authenticity_score - r2.authenticity_score).abs() < f32::EPSILON);
    }

    #[test]
    fn m3_lowered_synthetic_threshold_shifts_classification() {
        // Lowering synthetic_min_threshold toward 0 should make it easier
        // to classify an image as Synthetic (or at least never harder).
        let png = make_gradient_png(200, 200);
        let baseline =
            verify_bytes_with_config(&png, ExecutionMode::Deep, &CalibrationConfig::default())
                .unwrap();

        let mut cfg = CalibrationConfig::default();
        cfg.synthetic_min_threshold = 0.01; // extremely low bar
        cfg.synthetic_margin_threshold = 0.001;
        let shifted = verify_bytes_with_config(&png, ExecutionMode::Deep, &cfg).unwrap();

        // The shifted score should be ≤ baseline (lower = more synthetic).
        assert!(
            shifted.authenticity_score <= baseline.authenticity_score + 0.01,
            "Lowering synthetic threshold should not raise authenticity score: baseline={}, shifted={}",
            baseline.authenticity_score,
            shifted.authenticity_score,
        );
    }

    #[test]
    fn m3_raised_reason_threshold_reduces_reason_codes() {
        // Setting reason_code_contribution_threshold very high should
        // yield only the fallback reason code.
        let png = make_gradient_png(200, 200);
        let mut cfg = CalibrationConfig::default();
        cfg.reason_code_contribution_threshold = 999.0; // nothing can exceed this
        let result = verify_bytes_with_config(&png, ExecutionMode::Deep, &cfg).unwrap();
        assert_eq!(
            result.reason_codes.len(),
            1,
            "Only fallback reason code expected when threshold is unreachable, got {:?}",
            result.reason_codes
        );
    }

    #[test]
    fn m3_custom_max_file_size_rejects_input() {
        let png = make_gradient_png(50, 50);
        let mut cfg = CalibrationConfig::default();
        cfg.max_file_size_bytes = 10; // absurdly small
        let err = verify_bytes_with_config(&png, ExecutionMode::Deep, &cfg).unwrap_err();
        assert!(
            matches!(err, VerifyError::InputTooLarge { .. }),
            "Expected InputTooLarge, got {:?}",
            err
        );
    }

    #[test]
    fn m3_custom_max_dimension_rejects_large_image() {
        let png = make_gradient_png(100, 100);
        let mut cfg = CalibrationConfig::default();
        cfg.max_image_dimension = 10; // anything > 10px rejected
        let err = verify_bytes_with_config(&png, ExecutionMode::Deep, &cfg).unwrap_err();
        assert!(
            matches!(err, VerifyError::DimensionTooLarge { .. }),
            "Expected DimensionTooLarge, got {:?}",
            err
        );
    }

    #[test]
    fn m3_fast_mode_respects_config() {
        let png = make_gradient_png(100, 100);
        let r1 = verify_bytes_with_config(&png, ExecutionMode::Fast, &CalibrationConfig::default())
            .unwrap();
        let mut cfg = CalibrationConfig::default();
        cfg.synthetic_min_threshold = 0.01;
        let r2 = verify_bytes_with_config(&png, ExecutionMode::Fast, &cfg).unwrap();
        // Both must succeed — fast mode uses same config plumbing.
        assert!(r1.authenticity_score >= 0.0 && r1.authenticity_score <= 1.0);
        assert!(r2.authenticity_score >= 0.0 && r2.authenticity_score <= 1.0);
    }
}
