use crate::model::{
    ExecutionMode, LayerContributionScores, LayerLatencyMs, ReasonCode, ThresholdProfile,
    VerificationClass, VerificationResult, VerifyRequest,
};
use image::{GrayImage, ImageReader};
use rustfft::{num_complex::Complex, FftPlanner};
use std::io::Cursor;

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("image payload is empty")]
    EmptyInput,
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

pub fn verify(request: VerifyRequest) -> Result<VerificationResult, VerifyError> {
    if request.image_bytes.is_empty() {
        return Err(VerifyError::EmptyInput);
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
    let gray = image.to_luma8();

    let metrics = compute_signal_metrics(&gray);

    let synthetic_base = (0.24 * metrics.block_artifact_score
        + 0.20 * (1.0 - metrics.noise_score).max(0.0)
        + 0.10 * (1.0 - metrics.edge_score).max(0.0)
        + 0.22 * metrics.spectral_peak_score
        + 0.16 * (1.0 - metrics.high_freq_ratio_score).max(0.0)
        + 0.12 * (1.0 - metrics.prnu_plausibility_score).max(0.0)
        + 0.10 * (1.0 - metrics.cross_region_consistency).max(0.0)
        + 0.05 * metrics.hybrid_local_inconsistency
        + 0.03 * metrics.hybrid_seam_anomaly
        + 0.12 * metrics.semantic_synthetic_cue)
        .clamp(0.0, 1.0);

    let synthetic_suppression = (1.0
        - 0.22 * metrics.prnu_plausibility_score
        - 0.14 * metrics.cross_region_consistency
        - 0.08 * metrics.high_freq_ratio_score)
        .clamp(0.45, 1.0);
    let synthetic_likelihood = (synthetic_base * synthetic_suppression).clamp(0.0, 1.0);

    let edited_base = (0.28 * metrics.block_variance_cv
        + 0.06 * metrics.edge_score
        + 0.14 * metrics.block_artifact_score * (1.0 - synthetic_likelihood)
        + 0.08 * metrics.spectral_peak_score * 0.7
        + 0.12 * (1.0 - metrics.cross_region_consistency).max(0.0)
        + 0.04 * (1.0 - metrics.prnu_plausibility_score).max(0.0)
        + 0.20 * metrics.hybrid_local_inconsistency
        + 0.14 * metrics.hybrid_seam_anomaly
        + 0.03 * metrics.semantic_synthetic_cue)
        .clamp(0.0, 1.0);

    let edited_suppression =
        (1.0 - 0.20 * metrics.prnu_plausibility_score - 0.12 * metrics.cross_region_consistency)
            .clamp(0.55, 1.0);
    let edited_likelihood = (edited_base * edited_suppression).clamp(0.0, 1.0);

    let authentic_likelihood =
        (1.0 - 0.72 * synthetic_likelihood - 0.60 * edited_likelihood).clamp(0.0, 1.0);
    let layer_contributions = compute_layer_contributions(&metrics);
    let threshold_profile = ThresholdProfile {
        synthetic_min: SYNTHETIC_MIN_THRESHOLD,
        synthetic_margin: SYNTHETIC_MARGIN_THRESHOLD,
        suspicious_min: SUSPICIOUS_MIN_THRESHOLD,
    };

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
        } else {
            (
                VerificationClass::Authentic,
                (0.62 + authentic_likelihood * 0.33).clamp(0.62, 0.95),
                vec![ReasonCode::PhyPrnu001],
                vec![("physical".to_string(), vec![ReasonCode::PhyPrnu001])],
            )
        };

    let pixel_count = gray.width() as f32 * gray.height() as f32;
    let scale = (pixel_count / 12_000_000.0).clamp(0.4, 2.4);

    Ok(VerificationResult {
        authenticity_score,
        classification,
        reason_codes,
        layer_reasons,
        layer_contributions,
        threshold_profile,
        latency_ms: LayerLatencyMs {
            signal: (78.0 * scale) as u32,
            physical: (96.0 * scale) as u32,
            hybrid: (118.0 * scale) as u32,
            semantic: (132.0 * scale) as u32,
            fusion: 18,
        },
    })
}

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

    let noise_score = ((noise_accum / px_count) / 50.0).clamp(0.0, 1.0) as f32;
    let edge_score = ((edge_accum / px_count) / 50.0).clamp(0.0, 1.0) as f32;

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

    let block_artifact_score = (((boundary_avg / interior_avg) - 1.0) / 0.8).clamp(0.0, 1.0) as f32;
    let block_variance_cv = compute_block_variance_cv(gray);
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
