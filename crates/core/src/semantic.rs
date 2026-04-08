//! Semantic Intelligence Layer — residual pattern repetition, gradient-orientation
//! entropy, and synthetic-cue fusion.

use crate::config::*;
use image::GrayImage;

pub(crate) fn compute_semantic_metrics(
    residual_map: &[f32],
    gray: &GrayImage,
    source_width: usize,
    source_height: usize,
) -> (f32, f32, f32) {
    if source_width < SEMANTIC_MIN_DIM || source_height < SEMANTIC_MIN_DIM {
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
    let semantic_pattern_repetition = ((repetition_mean - SEMANTIC_REP_OFFSET) / SEMANTIC_REP_SCALE).clamp(0.0, 1.0);

    let bins = SEMANTIC_GRADIENT_BINS;
    let mut hist = vec![0.0f32; bins];
    let mut grad_sum = 0.0f32;

    // H4: gradient orientation uses the original gray image dimensions,
    // independent of the cropped residual dimensions.
    let gray_w = gray.width() as usize;
    let gray_h = gray.height() as usize;
    for y in 1..(gray_h - 1) {
        for x in 1..(gray_w - 1) {
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
        (SEMANTIC_CUE_W_REPETITION * semantic_pattern_repetition + SEMANTIC_CUE_W_ENTROPY_INV * (1.0 - semantic_gradient_entropy)).clamp(0.0, 1.0);

    (
        semantic_pattern_repetition,
        semantic_gradient_entropy,
        semantic_synthetic_cue,
    )
}

pub(crate) fn compute_shifted_residual_corr(
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

    // M5: use f64 accumulators to prevent precision loss on large images
    let mut sum_a = 0.0f64;
    let mut sum_b = 0.0f64;
    let mut n = 0.0f64;

    for y in 0..max_y {
        for x in 0..max_x {
            let a = residual_map[y * source_width + x] as f64;
            let b = residual_map[(y + dy) * source_width + (x + dx)] as f64;
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
    let mut num = 0.0f64;
    let mut den_a = 0.0f64;
    let mut den_b = 0.0f64;

    for y in 0..max_y {
        for x in 0..max_x {
            let a = residual_map[y * source_width + x] as f64 - mean_a;
            let b = residual_map[(y + dy) * source_width + (x + dx)] as f64 - mean_b;
            num += a * b;
            den_a += a * a;
            den_b += b * b;
        }
    }

    let denom = (den_a * den_b).sqrt();
    if denom <= f64::EPSILON {
        return None;
    }

    Some(((num / denom).clamp(-1.0, 1.0)) as f32)
}
