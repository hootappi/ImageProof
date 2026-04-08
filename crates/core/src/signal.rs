//! Signal Intelligence Layer — pixel-level statistics and FFT spectral features.

use crate::config::*;
use image::GrayImage;
use rustfft::{num_complex::Complex, FftPlanner};

/// M4: Merged pixel-level statistics and residual map in a single interior-pixel pass.
/// Eliminates the duplicate iteration that existed when `compute_pixel_statistics`
/// and `compute_residual_map` were separate functions.
/// Returns `(noise_score, edge_score, block_artifact_score, block_variance_cv,
///           residual_map, res_w, res_h)`.
/// H2: `block_artifact_score` is forced to 0.0 when `is_jpeg` is false.
pub(crate) fn compute_pixel_stats_and_residual(
    gray: &GrayImage,
    is_jpeg: bool,
) -> (f32, f32, f32, f32, Vec<f32>, usize, usize) {
    let width = gray.width();
    let height = gray.height();

    if width < 3 || height < 3 {
        return (0.0, 0.0, 0.0, 0.0, Vec::new(), 0, 0);
    }

    let inner_w = (width - 2) as usize;
    let inner_h = (height - 2) as usize;
    let mut residual = vec![0.0f32; inner_w * inner_h];

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

            // Pixel statistics (was compute_pixel_statistics)
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

            // Residual map (was compute_residual_map)
            let iy = (y - 1) as usize;
            let ix = (x - 1) as usize;
            residual[iy * inner_w + ix] = (center - local_mean) as f32;
        }
    }

    let noise_score = if px_count > 0.0 {
        ((noise_accum / px_count) / NOISE_NORMALIZATION).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let edge_score = if px_count > 0.0 {
        ((edge_accum / px_count) / EDGE_NORMALIZATION).clamp(0.0, 1.0) as f32
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

    let block_artifact_score = if !is_jpeg || interior_avg <= f64::EPSILON {
        0.0f32
    } else {
        (((boundary_avg / interior_avg) - 1.0) / BLOCK_ARTIFACT_NORMALIZATION).clamp(0.0, 1.0) as f32
    };
    let block_variance_cv = compute_block_variance_cv(gray);

    (noise_score, edge_score, block_artifact_score, block_variance_cv, residual, inner_w, inner_h)
}

/// Extract pixel-level statistics only (used by fast-mode path).
/// H2: `block_artifact_score` is forced to 0.0 when `is_jpeg` is false.
pub(crate) fn compute_pixel_statistics(gray: &GrayImage, is_jpeg: bool) -> (f32, f32, f32, f32) {
    let (noise, edge, block_art, cv, _, _, _) = compute_pixel_stats_and_residual(gray, is_jpeg);
    (noise, edge, block_art, cv)
}

pub(crate) fn compute_fft_signal_features(
    residual_map: &[f32],
    source_width: usize,
    source_height: usize,
) -> (f32, f32) {
    if source_width < FFT_MIN_DIM || source_height < FFT_MIN_DIM {
        return (0.0, 0.0);
    }

    // H3: increased FFT window from 64 to min(dim, 256) for richer
    // spectral resolution on larger images while bounding compute cost.
    let n = source_width.min(source_height).min(FFT_WINDOW_CAP);
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
            if radius_ratio > FFT_HIGH_FREQ_RADIUS {
                high_freq_sum += magnitude;
            }
        }
    }

    if count <= 0.0 || sum <= f32::EPSILON {
        return (0.0, 0.0);
    }

    let mean = sum / count;
    let spectral_peak = ((max / mean - 1.0) / FFT_PEAK_NORMALIZATION).clamp(0.0, 1.0);
    let high_freq_ratio = (high_freq_sum / sum).clamp(0.0, 1.0);
    (spectral_peak, high_freq_ratio)
}

pub(crate) fn sample_rect(
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

pub(crate) fn fft2d_magnitude(input: &[f32], side: usize) -> Vec<f32> {
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

pub(crate) fn compute_block_variance_cv(gray: &GrayImage) -> f32 {
    let width = gray.width() as usize;
    let height = gray.height() as usize;
    let block = BLOCK_VAR_BLOCK_SIZE;

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
    ((std / mean) / BLOCK_VAR_CV_NORMALIZATION).clamp(0.0, 1.0) as f32
}

/// Returns `(interior_residual, interior_width, interior_height)`.
/// H4: border rows/cols are excluded; the returned buffer contains only
/// pixels in the `1..(height-1)`, `1..(width-1)` interior range so no
/// zero-padded border values contaminate downstream metrics.
#[cfg(test)]
pub(crate) fn compute_residual_map(gray: &GrayImage) -> (Vec<f32>, usize, usize) {
    let width = gray.width();
    let height = gray.height();

    if width < 3 || height < 3 {
        return (Vec::new(), 0, 0);
    }

    let inner_w = (width - 2) as usize;
    let inner_h = (height - 2) as usize;
    let mut residual = vec![0.0f32; inner_w * inner_h];

    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let center = gray.get_pixel(x, y)[0] as f32;
            let left = gray.get_pixel(x - 1, y)[0] as f32;
            let right = gray.get_pixel(x + 1, y)[0] as f32;
            let up = gray.get_pixel(x, y - 1)[0] as f32;
            let down = gray.get_pixel(x, y + 1)[0] as f32;
            let local_mean = (left + right + up + down) * 0.25;
            let iy = (y - 1) as usize;
            let ix = (x - 1) as usize;
            residual[iy * inner_w + ix] = center - local_mean;
        }
    }

    (residual, inner_w, inner_h)
}
