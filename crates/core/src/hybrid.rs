//! Hybrid Manipulation Layer — localized residual inconsistency and seam anomaly detection.

use crate::config::*;

pub(crate) fn compute_hybrid_metrics(
    residual_map: &[f32],
    source_width: usize,
    source_height: usize,
) -> (f32, f32) {
    let min_dim = source_width.min(source_height);
    if min_dim < HYBRID_MIN_DIM {
        return (0.0, 0.0);
    }

    let tile = (min_dim / 8).clamp(HYBRID_TILE_MIN, HYBRID_TILE_MAX);
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

    let mean_energy = (energy_sum / (blocks_x * blocks_y) as f32).max(HYBRID_ENERGY_FLOOR);
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
        ((seam_excess_sum / seam_count) / HYBRID_SEAM_NORMALIZATION).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let coverage = (pair_count / HYBRID_COVERAGE_DENOM).clamp(HYBRID_COVERAGE_FLOOR, 1.0);
    (local_inconsistency * coverage, seam_anomaly * coverage)
}
