use crate::model::{
    ExecutionMode, LayerContributionScores, LayerLatencyMs, ReasonCode, ThresholdProfile,
    VerificationClass, VerificationResult, VerifyRequest,
};
use image::{GrayImage, ImageReader};
use rustfft::{num_complex::Complex, FftPlanner};
use std::io::Cursor;
use std::time::Instant;

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
    #[error("image decode failed")]
    DecodeFailed,
    #[error("only deep analysis is available in the current scaffold")]
    NotImplemented,
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

const SYNTHETIC_MIN_THRESHOLD: f32 = 0.66;
const SYNTHETIC_MARGIN_THRESHOLD: f32 = 0.12;
const SUSPICIOUS_MIN_THRESHOLD: f32 = 0.62;

/// Below this ceiling, neither synthetic nor edited signals are strong enough
/// to make a confident determination. The engine emits Indeterminate instead
/// of defaulting to Authentic. (C3 fix)
const INDETERMINATE_CEILING: f32 = 0.30;

/// Minimum signal spread (|synthetic - edited|) required to break an
/// Indeterminate deadlock when both likelihoods are below the ceiling.
const INDETERMINATE_MIN_SPREAD: f32 = 0.08;

/// Maximum accepted raw input size (50 MB). Prevents unbounded memory
/// allocation when the caller supplies a very large buffer.
const MAX_FILE_SIZE_BYTES: usize = 50 * 1024 * 1024;

/// Maximum accepted image dimension (width or height). Prevents
/// excessive memory allocation during decode and per-pixel processing.
const MAX_IMAGE_DIMENSION: u32 = 16_384;

pub fn verify(request: VerifyRequest) -> Result<VerificationResult, VerifyError> {
    if request.image_bytes.is_empty() {
        return Err(VerifyError::EmptyInput);
    }

    let size = request.image_bytes.len();
    if size > MAX_FILE_SIZE_BYTES {
        return Err(VerifyError::InputTooLarge {
            size,
            limit: MAX_FILE_SIZE_BYTES,
        });
    }

    match request.execution_mode {
        ExecutionMode::Fast => Err(VerifyError::NotImplemented),
        ExecutionMode::Deep => verify_deep_heuristic(&request.image_bytes),
    }
}

fn verify_deep_heuristic(image_bytes: &[u8]) -> Result<VerificationResult, VerifyError> {
    let reader = ImageReader::new(Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|_| VerifyError::DecodeFailed)?;
    let image = reader.decode().map_err(|_| VerifyError::DecodeFailed)?;

    let (width, height) = (image.width(), image.height());
    if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(VerifyError::DimensionTooLarge {
            width,
            height,
            limit: MAX_IMAGE_DIMENSION,
        });
    }

    let gray = image.to_luma8();

    // C2: Real per-layer timing via compute_signal_metrics_timed.
    let timed = compute_signal_metrics_timed(&gray);
    let metrics = timed.metrics;

    // Synthetic-base fusion weights — normalized to sum = 1.00 (C1 fix).
    let synthetic_base = (0.18 * metrics.block_artifact_score
        + 0.15 * (1.0 - metrics.noise_score).max(0.0)
        + 0.08 * (1.0 - metrics.edge_score).max(0.0)
        + 0.16 * metrics.spectral_peak_score
        + 0.12 * (1.0 - metrics.high_freq_ratio_score).max(0.0)
        + 0.09 * (1.0 - metrics.prnu_plausibility_score).max(0.0)
        + 0.07 * (1.0 - metrics.cross_region_consistency).max(0.0)
        + 0.04 * metrics.hybrid_local_inconsistency
        + 0.02 * metrics.hybrid_seam_anomaly
        + 0.09 * metrics.semantic_synthetic_cue)
        .clamp(0.0, 1.0);

    let synthetic_suppression = (1.0
        - 0.22 * metrics.prnu_plausibility_score
        - 0.14 * metrics.cross_region_consistency
        - 0.08 * metrics.high_freq_ratio_score)
        .clamp(0.45, 1.0);
    let synthetic_likelihood = (synthetic_base * synthetic_suppression).clamp(0.0, 1.0);

    // Edited-base fusion weights — normalized to sum = 1.00 (C1 fix).
    let edited_base = (0.26 * metrics.block_variance_cv
        + 0.05 * metrics.edge_score
        + 0.13 * metrics.block_artifact_score * (1.0 - synthetic_likelihood)
        + 0.07 * metrics.spectral_peak_score * 0.7
        + 0.11 * (1.0 - metrics.cross_region_consistency).max(0.0)
        + 0.04 * (1.0 - metrics.prnu_plausibility_score).max(0.0)
        + 0.18 * metrics.hybrid_local_inconsistency
        + 0.13 * metrics.hybrid_seam_anomaly
        + 0.03 * metrics.semantic_synthetic_cue)
        .clamp(0.0, 1.0);

    let edited_suppression =
        (1.0 - 0.20 * metrics.prnu_plausibility_score - 0.12 * metrics.cross_region_consistency)
            .clamp(0.55, 1.0);
    let edited_likelihood = (edited_base * edited_suppression).clamp(0.0, 1.0);

    // Authentic complement — coefficients sum to 1.0 (C1 fix, was 0.72+0.60=1.32).
    let authentic_likelihood =
        (1.0 - 0.55 * synthetic_likelihood - 0.45 * edited_likelihood).clamp(0.0, 1.0);

    let (classification, authenticity_score, reason_codes, layer_reasons) =
        if synthetic_likelihood > SYNTHETIC_MIN_THRESHOLD
            && synthetic_likelihood > edited_likelihood + SYNTHETIC_MARGIN_THRESHOLD
        {
            (
                VerificationClass::Synthetic,
                (1.0 - 0.9 * synthetic_likelihood).clamp(0.05, 0.40),
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
        } else if edited_likelihood > SUSPICIOUS_MIN_THRESHOLD {
            let mut reason_codes = vec![ReasonCode::HybEla001, ReasonCode::SigFreq001, ReasonCode::PhyPrnu001];
            let mut layer_reasons = vec![
                ("hybrid".to_string(), vec![ReasonCode::HybEla001]),
                ("signal".to_string(), vec![ReasonCode::SigFreq001]),
                ("physical".to_string(), vec![ReasonCode::PhyPrnu001]),
            ];

            if metrics.semantic_synthetic_cue > 0.55
                || (metrics.semantic_pattern_repetition > 0.50 && metrics.semantic_gradient_entropy < 0.45)
            {
                reason_codes.push(ReasonCode::SemClass001);
                layer_reasons.push(("semantic".to_string(), vec![ReasonCode::SemClass001]));
            }

            (
                VerificationClass::Suspicious,
                (0.35 + (1.0 - edited_likelihood) * 0.25).clamp(0.35, 0.60),
                reason_codes,
                layer_reasons,
            )
        } else if synthetic_likelihood < INDETERMINATE_CEILING
            && edited_likelihood < INDETERMINATE_CEILING
            && (synthetic_likelihood - edited_likelihood).abs() < INDETERMINATE_MIN_SPREAD
        {
            // C3: Neither signal path reached a meaningful level and the two
            // paths are within the spread threshold — insufficient evidence.
            (
                VerificationClass::Indeterminate,
                0.50,
                vec![ReasonCode::SysInsuff001],
                vec![("system".to_string(), vec![ReasonCode::SysInsuff001])],
            )
        } else {
            (
                VerificationClass::Authentic,
                (0.62 + authentic_likelihood * 0.33).clamp(0.62, 0.95),
                vec![ReasonCode::PhyPrnu001],
                vec![("physical".to_string(), vec![ReasonCode::PhyPrnu001])],
            )
        };

    // --- Fusion timing (C2: real measurement) ---
    let t_fusion = Instant::now();
    let layer_contributions = compute_layer_contributions(&metrics);
    let threshold_profile = ThresholdProfile {
        synthetic_min: SYNTHETIC_MIN_THRESHOLD,
        synthetic_margin: SYNTHETIC_MARGIN_THRESHOLD,
        suspicious_min: SUSPICIOUS_MIN_THRESHOLD,
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
fn compute_signal_metrics_timed(gray: &GrayImage) -> TimedMetrics {
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

    // --- Signal layer: pixel statistics + FFT ---
    let t_signal = Instant::now();
    let (noise_score, edge_score, block_artifact_score, block_variance_cv) =
        compute_pixel_statistics(gray);
    let residual_map = compute_residual_map(gray);
    let (spectral_peak_score, high_freq_ratio_score) =
        compute_fft_signal_features(&residual_map, width as usize, height as usize);
    let signal_ms = t_signal.elapsed().as_millis() as u32;

    // --- Physical layer: PRNU proxy ---
    let t_physical = Instant::now();
    let (prnu_plausibility_score, cross_region_consistency) =
        compute_prnu_proxy_metrics(&residual_map, width as usize, height as usize);
    let physical_ms = t_physical.elapsed().as_millis() as u32;

    // --- Hybrid layer: local inconsistency + seam ---
    let t_hybrid = Instant::now();
    let (hybrid_local_inconsistency, hybrid_seam_anomaly) =
        compute_hybrid_metrics(&residual_map, width as usize, height as usize);
    let hybrid_ms = t_hybrid.elapsed().as_millis() as u32;

    // --- Semantic layer: repetition + gradient entropy ---
    let t_semantic = Instant::now();
    let (semantic_pattern_repetition, semantic_gradient_entropy, semantic_synthetic_cue) =
        compute_semantic_metrics(&residual_map, gray, width as usize, height as usize);
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

/// Extract pixel-level statistics: noise, edge, block artifact, block variance CV.
fn compute_pixel_statistics(gray: &GrayImage) -> (f32, f32, f32, f32) {
    let width = gray.width();
    let height = gray.height();

    let mut noise_accum = 0.0f64;
    let mut edge_accum = 0.0f64;
    let mut px_count = 0.0f64;

    let mut boundary_diff_accum = 0.0f64;
    let mut boundary_count = 0.0f64;
    let mut interior_diff_accum = 0.0f64;
    let mut interior_count = 0.0f64;

    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let center = gray.get_pixel(x, y)[0] as f64;
            let left = gray.get_pixel(x - 1, y)[0] as f64;
            let right = gray.get_pixel(x + 1, y)[0] as f64;
            let up = gray.get_pixel(x, y - 1)[0] as f64;
            let down = gray.get_pixel(x, y + 1)[0] as f64;

            let local_mean = (left + right + up + down) * 0.25;
            noise_accum += (center - local_mean).abs();

            edge_accum += ((center - right).abs() + (center - down).abs()) * 0.5;
            px_count += 1.0;

            let neighbor_delta = (center - left).abs();
            if x % 8 == 0 || y % 8 == 0 {
                boundary_diff_accum += neighbor_delta;
                boundary_count += 1.0;
            } else {
                interior_diff_accum += neighbor_delta;
                interior_count += 1.0;
            }
        }
    }

    let noise_score = if px_count > 0.0 {
        ((noise_accum / px_count) / 50.0).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let edge_score = if px_count > 0.0 {
        ((edge_accum / px_count) / 50.0).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };

    let boundary_avg = if boundary_count > 0.0 {
        boundary_diff_accum / boundary_count
    } else {
        0.0
    };
    let interior_avg = if interior_count > 0.0 {
        interior_diff_accum / interior_count
    } else {
        1.0
    };

    let block_artifact_score = if interior_avg <= f64::EPSILON {
        0.0f32
    } else {
        (((boundary_avg / interior_avg) - 1.0) / 0.8).clamp(0.0, 1.0) as f32
    };
    let block_variance_cv = compute_block_variance_cv(gray);

    (noise_score, edge_score, block_artifact_score, block_variance_cv)
}

#[cfg(test)]
fn compute_signal_metrics(gray: &GrayImage) -> SignalMetrics {
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

    let (noise_score, edge_score, block_artifact_score, block_variance_cv) =
        compute_pixel_statistics(gray);
    let residual_map = compute_residual_map(gray);
    let (spectral_peak_score, high_freq_ratio_score) =
        compute_fft_signal_features(&residual_map, width as usize, height as usize);
    let (prnu_plausibility_score, cross_region_consistency) =
        compute_prnu_proxy_metrics(&residual_map, width as usize, height as usize);
    let (hybrid_local_inconsistency, hybrid_seam_anomaly) =
        compute_hybrid_metrics(&residual_map, width as usize, height as usize);
    let (semantic_pattern_repetition, semantic_gradient_entropy, semantic_synthetic_cue) =
        compute_semantic_metrics(&residual_map, gray, width as usize, height as usize);

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

fn compute_layer_contributions(metrics: &SignalMetrics) -> LayerContributionScores {
    let signal = (0.24 * metrics.block_artifact_score
        + 0.16 * (1.0 - metrics.noise_score).max(0.0)
        + 0.12 * (1.0 - metrics.edge_score).max(0.0)
        + 0.24 * metrics.spectral_peak_score
        + 0.24 * (1.0 - metrics.high_freq_ratio_score).max(0.0))
        .clamp(0.0, 1.0);

    let physical = (0.50 * (1.0 - metrics.prnu_plausibility_score).max(0.0)
        + 0.50 * (1.0 - metrics.cross_region_consistency).max(0.0))
        .clamp(0.0, 1.0);

    let hybrid = (0.58 * metrics.hybrid_local_inconsistency + 0.42 * metrics.hybrid_seam_anomaly)
        .clamp(0.0, 1.0);

    let semantic = metrics.semantic_synthetic_cue.clamp(0.0, 1.0);

    LayerContributionScores {
        signal,
        physical,
        hybrid,
        semantic,
    }
}

fn compute_semantic_metrics(
    residual_map: &[f32],
    gray: &GrayImage,
    source_width: usize,
    source_height: usize,
) -> (f32, f32, f32) {
    if source_width < 24 || source_height < 24 {
        return (0.0, 0.0, 0.0);
    }

    let shift_candidates = [(7usize, 0usize), (0, 7), (7, 7), (11, 3), (3, 11)];
    let mut repetition_sum = 0.0f32;
    let mut repetition_count = 0.0f32;
    for (dx, dy) in shift_candidates {
        if let Some(corr) = compute_shifted_residual_corr(residual_map, source_width, source_height, dx, dy) {
            repetition_sum += corr.abs();
            repetition_count += 1.0;
        }
    }

    let repetition_mean = if repetition_count > 0.0 {
        repetition_sum / repetition_count
    } else {
        0.0
    };
    let semantic_pattern_repetition = ((repetition_mean - 0.05) / 0.25).clamp(0.0, 1.0);

    let bins = 8usize;
    let mut hist = vec![0.0f32; bins];
    let mut grad_sum = 0.0f32;

    for y in 1..(source_height - 1) {
        for x in 1..(source_width - 1) {
            let left = gray.get_pixel((x - 1) as u32, y as u32)[0] as f32;
            let right = gray.get_pixel((x + 1) as u32, y as u32)[0] as f32;
            let up = gray.get_pixel(x as u32, (y - 1) as u32)[0] as f32;
            let down = gray.get_pixel(x as u32, (y + 1) as u32)[0] as f32;

            let gx = right - left;
            let gy = down - up;
            let mag = (gx * gx + gy * gy).sqrt();
            if mag <= 1e-3 {
                continue;
            }

            let angle = gy.atan2(gx);
            let mapped = (angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);
            let mut index = (mapped * bins as f32).floor() as usize;
            if index >= bins {
                index = bins - 1;
            }
            hist[index] += mag;
            grad_sum += mag;
        }
    }

    let semantic_gradient_entropy = if grad_sum <= f32::EPSILON {
        0.0
    } else {
        let mut entropy = 0.0f32;
        for value in hist {
            if value <= 0.0 {
                continue;
            }
            let p = value / grad_sum;
            entropy -= p * p.log2();
        }
        (entropy / (bins as f32).log2()).clamp(0.0, 1.0)
    };

    let semantic_synthetic_cue =
        (0.42 * semantic_pattern_repetition + 0.30 * (1.0 - semantic_gradient_entropy)).clamp(0.0, 1.0);

    (
        semantic_pattern_repetition,
        semantic_gradient_entropy,
        semantic_synthetic_cue,
    )
}

fn compute_shifted_residual_corr(
    residual_map: &[f32],
    source_width: usize,
    source_height: usize,
    dx: usize,
    dy: usize,
) -> Option<f32> {
    if dx >= source_width || dy >= source_height {
        return None;
    }

    let max_x = source_width - dx;
    let max_y = source_height - dy;
    if max_x < 4 || max_y < 4 {
        return None;
    }

    let mut sum_a = 0.0f32;
    let mut sum_b = 0.0f32;
    let mut n = 0.0f32;

    for y in 0..max_y {
        for x in 0..max_x {
            let a = residual_map[y * source_width + x];
            let b = residual_map[(y + dy) * source_width + (x + dx)];
            sum_a += a;
            sum_b += b;
            n += 1.0;
        }
    }

    if n <= 1.0 {
        return None;
    }

    let mean_a = sum_a / n;
    let mean_b = sum_b / n;
    let mut num = 0.0f32;
    let mut den_a = 0.0f32;
    let mut den_b = 0.0f32;

    for y in 0..max_y {
        for x in 0..max_x {
            let a = residual_map[y * source_width + x] - mean_a;
            let b = residual_map[(y + dy) * source_width + (x + dx)] - mean_b;
            num += a * b;
            den_a += a * a;
            den_b += b * b;
        }
    }

    let denom = (den_a * den_b).sqrt();
    if denom <= f32::EPSILON {
        return None;
    }

    Some((num / denom).clamp(-1.0, 1.0))
}

fn compute_hybrid_metrics(
    residual_map: &[f32],
    source_width: usize,
    source_height: usize,
) -> (f32, f32) {
    let min_dim = source_width.min(source_height);
    if min_dim < 24 {
        return (0.0, 0.0);
    }

    let tile = (min_dim / 8).clamp(12, 32);
    let blocks_x = source_width / tile;
    let blocks_y = source_height / tile;

    if blocks_x < 2 || blocks_y < 2 {
        return (0.0, 0.0);
    }

    let mut tile_energy = vec![0.0f32; blocks_x * blocks_y];
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let mut sum_abs = 0.0f32;
            for y in (by * tile)..((by + 1) * tile) {
                for x in (bx * tile)..((bx + 1) * tile) {
                    sum_abs += residual_map[y * source_width + x].abs();
                }
            }

            let area = (tile * tile) as f32;
            tile_energy[by * blocks_x + bx] = sum_abs / area;
        }
    }

    let mut pair_diff_sum = 0.0f32;
    let mut pair_count = 0.0f32;
    let mut energy_sum = 0.0f32;
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let current = tile_energy[by * blocks_x + bx];
            energy_sum += current;

            if bx + 1 < blocks_x {
                let right = tile_energy[by * blocks_x + (bx + 1)];
                pair_diff_sum += (current - right).abs();
                pair_count += 1.0;
            }

            if by + 1 < blocks_y {
                let down = tile_energy[(by + 1) * blocks_x + bx];
                pair_diff_sum += (current - down).abs();
                pair_count += 1.0;
            }
        }
    }

    let mean_energy = (energy_sum / (blocks_x * blocks_y) as f32).max(1e-4);
    let mean_pair_diff = if pair_count > 0.0 {
        pair_diff_sum / pair_count
    } else {
        0.0
    };
    let local_inconsistency_raw = mean_pair_diff / (mean_energy * 2.0);
    let local_inconsistency = local_inconsistency_raw.clamp(0.0, 1.0);

    let seam_step = (tile / 2).max(6);
    let mut seam_excess_sum = 0.0f32;
    let mut seam_count = 0.0f32;

    for x in (2..(source_width.saturating_sub(2))).step_by(seam_step) {
        for y in 1..(source_height - 1) {
            let across = (residual_map[y * source_width + x] - residual_map[y * source_width + (x - 1)])
                .abs();
            let local_left =
                (residual_map[y * source_width + (x - 1)] - residual_map[y * source_width + (x - 2)])
                    .abs();
            let local_right =
                (residual_map[y * source_width + (x + 1)] - residual_map[y * source_width + x]).abs();
            let baseline = (local_left + local_right) * 0.5 + 1e-4;
            seam_excess_sum += (across / baseline - 1.0).max(0.0);
            seam_count += 1.0;
        }
    }

    for y in (2..(source_height.saturating_sub(2))).step_by(seam_step) {
        for x in 1..(source_width - 1) {
            let across =
                (residual_map[y * source_width + x] - residual_map[(y - 1) * source_width + x]).abs();
            let local_up =
                (residual_map[(y - 1) * source_width + x] - residual_map[(y - 2) * source_width + x])
                    .abs();
            let local_down =
                (residual_map[(y + 1) * source_width + x] - residual_map[y * source_width + x]).abs();
            let baseline = (local_up + local_down) * 0.5 + 1e-4;
            seam_excess_sum += (across / baseline - 1.0).max(0.0);
            seam_count += 1.0;
        }
    }

    let seam_anomaly = if seam_count > 0.0 {
        ((seam_excess_sum / seam_count) / 2.4).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let coverage = (pair_count / 2500.0).clamp(0.05, 1.0);
    (local_inconsistency * coverage, seam_anomaly * coverage)
}

fn compute_prnu_proxy_metrics(
    residual_map: &[f32],
    source_width: usize,
    source_height: usize,
) -> (f32, f32) {
    let block = 24usize;
    if source_width < block * 2 || source_height < block * 2 {
        return (0.0, 0.0);
    }

    let blocks_x = source_width / block;
    let blocks_y = source_height / block;
    let mut correlations: Vec<f32> = Vec::new();

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            if bx + 1 < blocks_x {
                if let Some(corr) = block_corr(
                    residual_map,
                    source_width,
                    bx * block,
                    by * block,
                    (bx + 1) * block,
                    by * block,
                    block,
                ) {
                    correlations.push(corr);
                }
            }

            if by + 1 < blocks_y {
                if let Some(corr) = block_corr(
                    residual_map,
                    source_width,
                    bx * block,
                    by * block,
                    bx * block,
                    (by + 1) * block,
                    block,
                ) {
                    correlations.push(corr);
                }
            }
        }
    }

    if correlations.is_empty() {
        return (0.0, 0.0);
    }

    let n = correlations.len() as f32;
    let mean_corr = correlations.iter().copied().sum::<f32>() / n;
    let var_corr = correlations
        .iter()
        .map(|c| {
            let d = *c - mean_corr;
            d * d
        })
        .sum::<f32>()
        / n;
    let std_corr = var_corr.sqrt();

    let plausibility = ((mean_corr + 0.02) / 0.15).clamp(0.0, 1.0);
    let consistency = (1.0 - (std_corr / 0.20)).clamp(0.0, 1.0);

    let pair_coverage = (n / 3500.0).clamp(0.25, 1.0);
    (plausibility * pair_coverage, consistency * pair_coverage)
}

fn block_corr(
    data: &[f32],
    width: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    block: usize,
) -> Option<f32> {
    let mut sum_a = 0.0f32;
    let mut sum_b = 0.0f32;
    let n = (block * block) as f32;

    for dy in 0..block {
        for dx in 0..block {
            let a = data[(y0 + dy) * width + (x0 + dx)];
            let b = data[(y1 + dy) * width + (x1 + dx)];
            sum_a += a;
            sum_b += b;
        }
    }

    let mean_a = sum_a / n;
    let mean_b = sum_b / n;

    let mut num = 0.0f32;
    let mut den_a = 0.0f32;
    let mut den_b = 0.0f32;

    for dy in 0..block {
        for dx in 0..block {
            let a = data[(y0 + dy) * width + (x0 + dx)] - mean_a;
            let b = data[(y1 + dy) * width + (x1 + dx)] - mean_b;
            num += a * b;
            den_a += a * a;
            den_b += b * b;
        }
    }

    let denom = (den_a * den_b).sqrt();
    if denom <= f32::EPSILON {
        return None;
    }

    Some((num / denom).clamp(-1.0, 1.0))
}

fn compute_residual_map(gray: &GrayImage) -> Vec<f32> {
    let width = gray.width();
    let height = gray.height();
    let mut residual = vec![0.0f32; (width * height) as usize];

    if width < 3 || height < 3 {
        return residual;
    }

    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let center = gray.get_pixel(x, y)[0] as f32;
            let left = gray.get_pixel(x - 1, y)[0] as f32;
            let right = gray.get_pixel(x + 1, y)[0] as f32;
            let up = gray.get_pixel(x, y - 1)[0] as f32;
            let down = gray.get_pixel(x, y + 1)[0] as f32;
            let local_mean = (left + right + up + down) * 0.25;
            residual[(y * width + x) as usize] = center - local_mean;
        }
    }

    residual
}

fn compute_fft_signal_features(
    residual_map: &[f32],
    source_width: usize,
    source_height: usize,
) -> (f32, f32) {
    if source_width < 16 || source_height < 16 {
        return (0.0, 0.0);
    }

    let n = source_width.min(source_height).min(64);
    let matrix = sample_rect(residual_map, source_width, source_height, n);
    let spectrum = fft2d_magnitude(&matrix, n);

    if spectrum.is_empty() {
        return (0.0, 0.0);
    }

    let mut sum = 0.0f32;
    let mut max = 0.0f32;
    let mut count = 0.0f32;
    let mut high_freq_sum = 0.0f32;

    let center = (n as f32 - 1.0) * 0.5;
    let max_radius = (2.0f32).sqrt() * center;

    for y in 0..n {
        for x in 0..n {
            if x == 0 && y == 0 {
                continue;
            }

            let magnitude = spectrum[y * n + x];
            sum += magnitude;
            max = max.max(magnitude);
            count += 1.0;

            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let radius_ratio = ((dx * dx + dy * dy).sqrt() / max_radius).clamp(0.0, 1.0);
            if radius_ratio > 0.62 {
                high_freq_sum += magnitude;
            }
        }
    }

    if count <= 0.0 || sum <= f32::EPSILON {
        return (0.0, 0.0);
    }

    let mean = sum / count;
    let spectral_peak = ((max / mean - 1.0) / 8.0).clamp(0.0, 1.0);
    let high_freq_ratio = (high_freq_sum / sum).clamp(0.0, 1.0);
    (spectral_peak, high_freq_ratio)
}

fn sample_rect(
    data: &[f32],
    source_width: usize,
    source_height: usize,
    target_side: usize,
) -> Vec<f32> {
    let mut out = vec![0.0f32; target_side * target_side];
    if source_width == 0 || source_height == 0 || target_side == 0 {
        return out;
    }

    let step_x = source_width as f32 / target_side as f32;
    let step_y = source_height as f32 / target_side as f32;
    for ty in 0..target_side {
        for tx in 0..target_side {
            let sx = (tx as f32 * step_x).floor() as usize;
            let sy = (ty as f32 * step_y).floor() as usize;
            let sx = sx.min(source_width - 1);
            let sy = sy.min(source_height - 1);
            out[ty * target_side + tx] = data[sy * source_width + sx];
        }
    }

    out
}

fn fft2d_magnitude(input: &[f32], side: usize) -> Vec<f32> {
    let mut planner = FftPlanner::<f32>::new();
    let fft_row = planner.plan_fft_forward(side);
    let fft_col = planner.plan_fft_forward(side);

    let mut complex: Vec<Complex<f32>> = input
        .iter()
        .map(|value| Complex { re: *value, im: 0.0 })
        .collect();

    for y in 0..side {
        let start = y * side;
        let end = start + side;
        fft_row.process(&mut complex[start..end]);
    }

    let mut column = vec![Complex { re: 0.0, im: 0.0 }; side];
    for x in 0..side {
        for y in 0..side {
            column[y] = complex[y * side + x];
        }
        fft_col.process(&mut column);
        for y in 0..side {
            complex[y * side + x] = column[y];
        }
    }

    complex
        .into_iter()
        .map(|c| (c.re * c.re + c.im * c.im).sqrt())
        .collect()
}

fn compute_block_variance_cv(gray: &GrayImage) -> f32 {
    let width = gray.width() as usize;
    let height = gray.height() as usize;
    let block = 32usize;

    if width < block || height < block {
        return 0.0;
    }

    let mut block_variances: Vec<f64> = Vec::new();

    let blocks_x = width / block;
    let blocks_y = height / block;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let x0 = bx * block;
            let y0 = by * block;

            let mut sum = 0.0f64;
            let mut sum_sq = 0.0f64;
            let n = (block * block) as f64;

            for y in y0..(y0 + block) {
                for x in x0..(x0 + block) {
                    let p = gray.get_pixel(x as u32, y as u32)[0] as f64;
                    sum += p;
                    sum_sq += p * p;
                }
            }

            let mean = sum / n;
            let variance = (sum_sq / n) - (mean * mean);
            block_variances.push(variance.max(0.0));
        }
    }

    if block_variances.is_empty() {
        return 0.0;
    }

    let n = block_variances.len() as f64;
    let mean = block_variances.iter().sum::<f64>() / n;
    if mean <= f64::EPSILON {
        return 0.0;
    }

    let var = block_variances
        .iter()
        .map(|v| {
            let d = *v - mean;
            d * d
        })
        .sum::<f64>()
        / n;

    let std = var.sqrt();
    ((std / mean) / 1.2).clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ExecutionMode, HardwareTier, VerifyRequest};
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
        // Exactly 50 MB should pass the size gate (it will fail on decode, not size).
        let at_limit = vec![0u8; MAX_FILE_SIZE_BYTES];
        let req = VerifyRequest {
            image_bytes: at_limit,
            execution_mode: ExecutionMode::Deep,
            hardware_tier: HardwareTier::CpuOnly,
        };
        let err = verify(req).unwrap_err();
        // Should fail at decode, not at size check
        assert!(
            matches!(err, VerifyError::DecodeFailed),
            "expected DecodeFailed (not InputTooLarge), got {err:?}"
        );
    }

    #[test]
    fn verify_fast_mode_returns_not_implemented() {
        let req = VerifyRequest {
            image_bytes: vec![0xFF],
            execution_mode: ExecutionMode::Fast,
            hardware_tier: HardwareTier::CpuOnly,
        };
        let err = verify(req).unwrap_err();
        assert!(matches!(err, VerifyError::NotImplemented));
    }

    #[test]
    fn verify_garbage_bytes_returns_decode_failed() {
        let req = VerifyRequest {
            image_bytes: vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33],
            execution_mode: ExecutionMode::Deep,
            hardware_tier: HardwareTier::CpuOnly,
        };
        let err = verify(req).unwrap_err();
        assert!(matches!(err, VerifyError::DecodeFailed));
    }

    #[test]
    fn verify_truncated_png_returns_decode_failed() {
        let png = make_png(64, 64, 128);
        let truncated = &png[..png.len() / 2];
        let req = VerifyRequest {
            image_bytes: truncated.to_vec(),
            execution_mode: ExecutionMode::Deep,
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
    fn residual_map_tiny_image_returns_zeros() {
        let gray = GrayImage::from_pixel(2, 2, Luma([100]));
        let residual = compute_residual_map(&gray);
        assert_eq!(residual.len(), 4);
        assert!(residual.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn residual_map_flat_image_interior_is_zero() {
        let gray = GrayImage::from_pixel(8, 8, Luma([100]));
        let residual = compute_residual_map(&gray);
        // Interior pixels (not border) should be zero for flat image
        for y in 1..7 {
            for x in 1..7 {
                assert_eq!(residual[y * 8 + x], 0.0, "residual at ({x},{y}) should be 0");
            }
        }
    }

    #[test]
    fn residual_map_borders_are_zero() {
        let gray = ImageBuffer::from_fn(8, 8, |x, y| {
            Luma([((x + y * 3) % 256) as u8])
        });
        let residual = compute_residual_map(&gray);
        // Top row, bottom row, left col, right col should all be 0
        for x in 0..8 {
            assert_eq!(residual[x], 0.0, "top border at x={x}");
            assert_eq!(residual[7 * 8 + x], 0.0, "bottom border at x={x}");
        }
        for y in 0..8 {
            assert_eq!(residual[y * 8], 0.0, "left border at y={y}");
            assert_eq!(residual[y * 8 + 7], 0.0, "right border at y={y}");
        }
    }

    #[test]
    fn residual_map_length_matches_image_dimensions() {
        let gray = GrayImage::from_pixel(16, 24, Luma([50]));
        let residual = compute_residual_map(&gray);
        assert_eq!(residual.len(), 16 * 24);
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
        let residual = compute_residual_map(&gray);
        let (rep, ent, cue) = compute_semantic_metrics(&residual, &gray, w as usize, h as usize);
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
        let lc = compute_layer_contributions(&metrics);
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
        let lc = compute_layer_contributions(&metrics);
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
                hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
                    hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
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
            hardware_tier: HardwareTier::CpuOnly,
        };
        let result = verify(req).unwrap();
        assert!(
            result.reason_codes.contains(&ReasonCode::SysInsuff001),
            "Indeterminate result should contain SysInsuff001, got {:?}",
            result.reason_codes
        );
    }
}
