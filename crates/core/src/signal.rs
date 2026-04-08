//! Signal Intelligence Layer — pixel-level statistics, FFT spectral features,
//! and color forensic metrics for synthetic image detection.

use crate::config::*;
use image::{GrayImage, RgbImage};
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

// ─── Color Forensic Metrics ──────────────────────────────────────────────

/// Compute color-channel noise correlation and noise-brightness dependency.
///
/// Returns `(channel_noise_corr, noise_brightness_corr)`:
/// - `channel_noise_corr` in [0,1]: high = inter-channel noise is correlated (synthetic cue).
///   Real cameras have independent per-channel sensor noise; AI generators produce
///   correlated channel noise from a shared latent representation.
/// - `noise_brightness_corr` in [0,1]: high = noise variance tracks brightness (authentic cue).
///   Real camera noise follows shot-noise physics (variance ∝ signal level);
///   AI-generated noise has no brightness dependency.
///
/// Returns `(0.0, 0.5)` (neutral) for grayscale images or images too small to analyze.
pub(crate) fn compute_color_forensics(
    rgb: &RgbImage,
    gray: &GrayImage,
) -> (f32, f32) {
    let width = rgb.width();
    let height = rgb.height();

    if width < 3 || height < 3 {
        return (0.0, 0.5);
    }

    // ── Grayscale detection ──
    // If R≈G≈B for all pixels, the image is effectively grayscale and
    // channel correlation is meaningless (would read 1.0 trivially).
    let mut color_diff_sum = 0.0f64;
    let mut color_diff_count = 0u64;
    let step = ((width * height) / 2000).max(1);
    let mut idx = 0u32;
    'outer: for y in 0..height {
        for x in 0..width {
            if idx.is_multiple_of(step) {
                let [r, g, b] = rgb.get_pixel(x, y).0;
                color_diff_sum += (r as f64 - g as f64).abs()
                    + (r as f64 - b as f64).abs()
                    + (g as f64 - b as f64).abs();
                color_diff_count += 1;
                if color_diff_count >= 2000 {
                    break 'outer;
                }
            }
            idx += 1;
        }
    }
    let is_grayscale = color_diff_count == 0
        || color_diff_sum / (color_diff_count as f64) < GRAYSCALE_MEAN_DIFF_THRESHOLD;

    if is_grayscale {
        return (0.0, 0.5);
    }

    // ── Channel noise correlation ──
    // Compute per-channel 4-neighbor residuals and Pearson correlation.
    let cap = ((width - 2) as usize) * ((height - 2) as usize);
    let mut r_res = Vec::with_capacity(cap);
    let mut g_res = Vec::with_capacity(cap);
    let mut b_res = Vec::with_capacity(cap);

    // Also accumulate noise-brightness bins during the same pass.
    let n_bins = NOISE_BRIGHTNESS_BINS;
    let mut bin_noise_sum = vec![0.0f64; n_bins];
    let mut bin_noise_sq = vec![0.0f64; n_bins];
    let mut bin_count = vec![0u64; n_bins];

    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let [rc, gc, bc] = rgb.get_pixel(x, y).0;
            let [rl, gl, bl] = rgb.get_pixel(x - 1, y).0;
            let [rr, gr, br] = rgb.get_pixel(x + 1, y).0;
            let [ru, gu, bu] = rgb.get_pixel(x, y - 1).0;
            let [rd, gd, bd] = rgb.get_pixel(x, y + 1).0;

            let r_mean = (rl as f64 + rr as f64 + ru as f64 + rd as f64) * 0.25;
            let g_mean = (gl as f64 + gr as f64 + gu as f64 + gd as f64) * 0.25;
            let b_mean = (bl as f64 + br as f64 + bu as f64 + bd as f64) * 0.25;

            r_res.push(rc as f64 - r_mean);
            g_res.push(gc as f64 - g_mean);
            b_res.push(bc as f64 - b_mean);

            // Noise-brightness binning (using grayscale luminance)
            let luma = gray.get_pixel(x, y)[0];
            let bin = ((luma as usize) * n_bins / 256).min(n_bins - 1);
            let gray_residual = {
                let cl = gray.get_pixel(x - 1, y)[0] as f64;
                let cr = gray.get_pixel(x + 1, y)[0] as f64;
                let cu = gray.get_pixel(x, y - 1)[0] as f64;
                let cd = gray.get_pixel(x, y + 1)[0] as f64;
                luma as f64 - (cl + cr + cu + cd) * 0.25
            };
            bin_noise_sum[bin] += gray_residual.abs();
            bin_noise_sq[bin] += gray_residual * gray_residual;
            bin_count[bin] += 1;
        }
    }

    // ── Pearson correlations between channel residuals ──
    let rg = pearson_corr_f64(&r_res, &g_res);
    let rb = pearson_corr_f64(&r_res, &b_res);
    let gb = pearson_corr_f64(&g_res, &b_res);

    // Average and clamp to [0,1] — negative correlations map to 0.
    let channel_noise_corr = ((rg + rb + gb) / 3.0).clamp(0.0, 1.0) as f32;

    // ── Noise-brightness correlation ──
    // Collect bins with enough samples, compute variance per bin,
    // then Pearson correlation between bin brightness and noise variance.
    let mut valid_xs = Vec::new();
    let mut valid_ys = Vec::new();
    for i in 0..n_bins {
        if bin_count[i] >= NOISE_BRIGHTNESS_MIN_SAMPLES {
            let n = bin_count[i] as f64;
            let mean_abs = bin_noise_sum[i] / n;
            let mean_sq = bin_noise_sq[i] / n;
            let variance = (mean_sq - mean_abs * mean_abs).max(0.0);
            valid_xs.push(i as f64);
            valid_ys.push(variance);
        }
    }

    let noise_brightness_corr = if valid_xs.len() >= 3 {
        pearson_corr_f64(&valid_xs, &valid_ys).clamp(0.0, 1.0) as f32
    } else {
        0.5 // neutral — insufficient brightness range
    };

    (channel_noise_corr, noise_brightness_corr)
}

/// Pearson correlation coefficient for two equal-length f64 slices.
/// Returns 0.0 if inputs are empty, have different lengths, or if either
/// series has zero variance (flat).
fn pearson_corr_f64(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len();
    if n == 0 || n != ys.len() {
        return 0.0;
    }
    let nf = n as f64;

    let mean_x = xs.iter().sum::<f64>() / nf;
    let mean_y = ys.iter().sum::<f64>() / nf;

    let mut cov = 0.0f64;
    let mut var_x = 0.0f64;
    let mut var_y = 0.0f64;

    for i in 0..n {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    let denom = (var_x * var_y).sqrt();
    if denom < 1e-15 {
        return 0.0;
    }

    cov / denom
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
