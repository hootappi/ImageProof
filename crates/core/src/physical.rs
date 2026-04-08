//! Physical Intelligence Layer — PRNU plausibility proxy and cross-region consistency.

use crate::config::*;

pub(crate) fn compute_prnu_proxy_metrics(
    residual_map: &[f32],
    source_width: usize,
    source_height: usize,
) -> (f32, f32) {
    let block = PRNU_BLOCK_SIZE;
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

    let plausibility = ((mean_corr + PRNU_PLAUS_OFFSET) / PRNU_PLAUS_SCALE).clamp(0.0, 1.0);
    let consistency = (1.0 - (std_corr / PRNU_CONSIST_SCALE)).clamp(0.0, 1.0);

    let pair_coverage = (n / PRNU_COVERAGE_DENOM).clamp(PRNU_COVERAGE_FLOOR, 1.0);
    (plausibility * pair_coverage, consistency * pair_coverage)
}

pub(crate) fn block_corr(
    data: &[f32],
    width: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    block: usize,
) -> Option<f32> {
    // M5: use f64 accumulators to prevent precision loss in correlation sums
    let mut sum_a = 0.0f64;
    let mut sum_b = 0.0f64;
    let n = (block * block) as f64;

    for dy in 0..block {
        for dx in 0..block {
            let a = data[(y0 + dy) * width + (x0 + dx)] as f64;
            let b = data[(y1 + dy) * width + (x1 + dx)] as f64;
            sum_a += a;
            sum_b += b;
        }
    }

    let mean_a = sum_a / n;
    let mean_b = sum_b / n;

    let mut num = 0.0f64;
    let mut den_a = 0.0f64;
    let mut den_b = 0.0f64;

    for dy in 0..block {
        for dx in 0..block {
            let a = data[(y0 + dy) * width + (x0 + dx)] as f64 - mean_a;
            let b = data[(y1 + dy) * width + (x1 + dx)] as f64 - mean_b;
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
