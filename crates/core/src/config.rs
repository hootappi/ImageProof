//! M2: Named constants for all calibration-critical parameters.
//!
//! Centralizes magic numbers from the verification engine to support:
//! - Explicit documentation of each parameter's purpose
//! - Single point of change for threshold tuning
//! - M3: Runtime config loading from TOML via `CalibrationConfig`
//!
//! The compile-time constants below serve as defaults for `CalibrationConfig`.
//! Runtime overrides (loaded from TOML) take precedence when passed through
//! `verify_bytes_with_config`.

use serde::Deserialize;

// ─── Classification thresholds ────────────────────────────────────────────

/// Minimum synthetic likelihood to classify as Synthetic.
pub const SYNTHETIC_MIN_THRESHOLD: f32 = 0.62;

/// Minimum margin synthetic must exceed edited likelihood by for Synthetic.
pub const SYNTHETIC_MARGIN_THRESHOLD: f32 = 0.12;

/// Minimum edited likelihood to classify as Suspicious.
pub const SUSPICIOUS_MIN_THRESHOLD: f32 = 0.62;

/// Below this ceiling, neither synthetic nor edited signals are strong enough
/// to make a confident determination — emit Indeterminate. (C3)
/// Recalibrated from 0.30 → 0.32 after H3 FFT window increase to 256
/// which shifted spectral features and marginally raised likelihoods.
pub const INDETERMINATE_CEILING: f32 = 0.32;

/// Minimum |synthetic − edited| spread to break an Indeterminate deadlock.
/// Recalibrated from 0.08 → 0.12 after H3 FFT spectral resolution increase.
pub const INDETERMINATE_MIN_SPREAD: f32 = 0.12;

/// Indeterminate fixed score output.
pub const INDETERMINATE_SCORE: f32 = 0.50;

// ─── Reason code thresholds ───────────────────────────────────────────────

/// M7: Minimum layer contribution to emit a per-layer reason code.
pub const REASON_CODE_CONTRIBUTION_THRESHOLD: f32 = 0.15;

/// Semantic cue threshold for suspicious-branch escalation.
pub const SEMANTIC_CUE_ESCALATION_THRESHOLD: f32 = 0.55;

/// Semantic pattern repetition threshold for suspicious escalation.
pub const SEMANTIC_REPETITION_ESCALATION_THRESHOLD: f32 = 0.50;

/// Semantic gradient entropy upper bound for suspicious escalation.
pub const SEMANTIC_ENTROPY_ESCALATION_CEILING: f32 = 0.45;

// ─── Input limits ─────────────────────────────────────────────────────────

/// Maximum accepted raw input size (50 MB).
pub const MAX_FILE_SIZE_BYTES: usize = 50 * 1024 * 1024;

/// Maximum accepted image dimension (width or height, 16384 px).
pub const MAX_IMAGE_DIMENSION: u32 = 16_384;

// ─── FFT parameters ──────────────────────────────────────────────────────

/// H3: Maximum side length for the FFT analysis window.
pub const FFT_WINDOW_CAP: usize = 256;

/// Minimum residual dimension for FFT analysis.
pub const FFT_MIN_DIM: usize = 16;

/// FFT high-frequency radius ratio threshold.
pub const FFT_HIGH_FREQ_RADIUS: f32 = 0.62;

/// FFT spectral peak normalization divisor.
pub const FFT_PEAK_NORMALIZATION: f32 = 8.0;

// ─── Deep-mode fusion weights: synthetic base (sum = 1.00, C1) ──────────

pub const SYN_W_BLOCK_ARTIFACT: f32 = 0.18;
pub const SYN_W_NOISE_INV: f32 = 0.15;
pub const SYN_W_EDGE_INV: f32 = 0.08;
pub const SYN_W_SPECTRAL_PEAK: f32 = 0.16;
pub const SYN_W_HF_RATIO_INV: f32 = 0.12;
pub const SYN_W_PRNU_INV: f32 = 0.09;
pub const SYN_W_CONSISTENCY_INV: f32 = 0.07;
pub const SYN_W_HYBRID_LOCAL: f32 = 0.04;
pub const SYN_W_HYBRID_SEAM: f32 = 0.02;
pub const SYN_W_SEMANTIC_CUE: f32 = 0.09;

// ─── Deep-mode fusion weights: synthetic suppression ─────────────────────

pub const SYN_SUPP_PRNU: f32 = 0.25;
pub const SYN_SUPP_CONSISTENCY: f32 = 0.18;
pub const SYN_SUPP_HF_RATIO: f32 = 0.10;
pub const SYN_SUPP_FLOOR: f32 = 0.40;

// ─── Deep-mode fusion weights: edited base (sum = 1.00, C1) ─────────────

pub const EDT_W_BLOCK_VAR_CV: f32 = 0.26;
pub const EDT_W_EDGE: f32 = 0.05;
pub const EDT_W_BLOCK_ARTIFACT: f32 = 0.13;
pub const EDT_W_SPECTRAL_PEAK: f32 = 0.07;
/// Spectral peak damping factor applied to edited-base spectral term.
pub const EDT_SPECTRAL_DAMP: f32 = 0.7;
pub const EDT_W_CONSISTENCY_INV: f32 = 0.11;
pub const EDT_W_PRNU_INV: f32 = 0.04;
pub const EDT_W_HYBRID_LOCAL: f32 = 0.18;
pub const EDT_W_HYBRID_SEAM: f32 = 0.13;
pub const EDT_W_SEMANTIC_CUE: f32 = 0.03;

// ─── Deep-mode fusion weights: edited suppression ────────────────────────

pub const EDT_SUPP_PRNU: f32 = 0.20;
pub const EDT_SUPP_CONSISTENCY: f32 = 0.12;
pub const EDT_SUPP_FLOOR: f32 = 0.55;

// ─── Deep-mode fusion: authentic complement ──────────────────────────────

pub const AUTH_W_SYNTHETIC: f32 = 0.55;
pub const AUTH_W_EDITED: f32 = 0.45;

// ─── Score mapping: output score formulas per classification ─────────────

/// Synthetic: `(1 - SYNTHETIC_SCORE_SCALE * likelihood)` clamped.
pub const SYNTHETIC_SCORE_SCALE: f32 = 0.9;
pub const SYNTHETIC_SCORE_MIN: f32 = 0.05;
pub const SYNTHETIC_SCORE_MAX: f32 = 0.40;

/// Suspicious: `(SUSPICIOUS_SCORE_BASE + (1 - likelihood) * SUSPICIOUS_SCORE_RANGE)` clamped.
pub const SUSPICIOUS_SCORE_BASE: f32 = 0.35;
pub const SUSPICIOUS_SCORE_RANGE: f32 = 0.25;
pub const SUSPICIOUS_SCORE_MIN: f32 = 0.35;
pub const SUSPICIOUS_SCORE_MAX: f32 = 0.60;

/// Authentic: `(AUTHENTIC_SCORE_BASE + likelihood * AUTHENTIC_SCORE_RANGE)` clamped.
pub const AUTHENTIC_SCORE_BASE: f32 = 0.62;
pub const AUTHENTIC_SCORE_RANGE: f32 = 0.33;
pub const AUTHENTIC_SCORE_MIN: f32 = 0.62;
pub const AUTHENTIC_SCORE_MAX: f32 = 0.95;

// ─── Layer contribution weights ──────────────────────────────────────────

pub const LC_SIGNAL_BLOCK_ART: f32 = 0.24;
pub const LC_SIGNAL_NOISE_INV: f32 = 0.16;
pub const LC_SIGNAL_EDGE_INV: f32 = 0.12;
pub const LC_SIGNAL_SPECTRAL: f32 = 0.24;
pub const LC_SIGNAL_HF_INV: f32 = 0.24;

pub const LC_PHYSICAL_PRNU_INV: f32 = 0.50;
pub const LC_PHYSICAL_CONSIST_INV: f32 = 0.50;

pub const LC_HYBRID_LOCAL: f32 = 0.58;
pub const LC_HYBRID_SEAM: f32 = 0.42;

// ─── Pixel-level metric normalization ────────────────────────────────────

/// Noise score divisor: `noise_accum / px_count / NOISE_NORMALIZATION`.
pub const NOISE_NORMALIZATION: f64 = 50.0;

/// Edge score divisor: `edge_accum / px_count / EDGE_NORMALIZATION`.
pub const EDGE_NORMALIZATION: f64 = 50.0;

/// Block artifact normalization: `(boundary/interior - 1) / BA_NORMALIZATION`.
pub const BLOCK_ARTIFACT_NORMALIZATION: f64 = 0.8;

/// Block variance CV normalization divisor.
pub const BLOCK_VAR_CV_NORMALIZATION: f64 = 1.2;

/// Block side length for variance CV computation.
pub const BLOCK_VAR_BLOCK_SIZE: usize = 32;

// ─── PRNU proxy parameters ───────────────────────────────────────────────

/// Block size for PRNU block correlation.
pub const PRNU_BLOCK_SIZE: usize = 24;

/// Plausibility offset: `(mean_corr + PRNU_PLAUS_OFFSET) / PRNU_PLAUS_SCALE`.
pub const PRNU_PLAUS_OFFSET: f32 = 0.02;

/// Plausibility scale divisor.
pub const PRNU_PLAUS_SCALE: f32 = 0.15;

/// Consistency std divisor: `1 - std_corr / PRNU_CONSIST_SCALE`.
pub const PRNU_CONSIST_SCALE: f32 = 0.20;

/// PRNU pair coverage denominator.
pub const PRNU_COVERAGE_DENOM: f32 = 3500.0;

/// PRNU pair coverage floor clamp.
pub const PRNU_COVERAGE_FLOOR: f32 = 0.25;

// ─── Hybrid parameters ───────────────────────────────────────────────────

/// Minimum dimension for hybrid analysis.
pub const HYBRID_MIN_DIM: usize = 24;

/// Tile size clamp range.
pub const HYBRID_TILE_MIN: usize = 12;
pub const HYBRID_TILE_MAX: usize = 32;

/// Mean energy floor for local inconsistency normalization.
pub const HYBRID_ENERGY_FLOOR: f32 = 1e-4;

/// Seam anomaly normalization divisor.
pub const HYBRID_SEAM_NORMALIZATION: f32 = 2.4;

/// Hybrid pair coverage denominator.
pub const HYBRID_COVERAGE_DENOM: f32 = 2500.0;

/// Hybrid pair coverage floor clamp.
pub const HYBRID_COVERAGE_FLOOR: f32 = 0.05;

// ─── Semantic parameters ─────────────────────────────────────────────────

/// Minimum dimension for semantic analysis.
pub const SEMANTIC_MIN_DIM: usize = 24;

/// Number of gradient orientation histogram bins.
pub const SEMANTIC_GRADIENT_BINS: usize = 8;

/// Semantic repetition offset and scale.
pub const SEMANTIC_REP_OFFSET: f32 = 0.05;
pub const SEMANTIC_REP_SCALE: f32 = 0.25;

/// Semantic synthetic cue fusion weights.
pub const SEMANTIC_CUE_W_REPETITION: f32 = 0.42;
pub const SEMANTIC_CUE_W_ENTROPY_INV: f32 = 0.30;

// ─── Color forensic parameters ───────────────────────────────────────────

/// Number of brightness bins for noise-brightness dependency analysis.
pub const NOISE_BRIGHTNESS_BINS: usize = 8;

/// Minimum pixel count per brightness bin to include in correlation.
pub const NOISE_BRIGHTNESS_MIN_SAMPLES: u64 = 100;

/// Color boost: weight for inter-channel noise correlation.
pub const COLOR_SYNTH_W_CHANNEL_CORR: f32 = 0.55;

/// Color boost: weight for inverted noise-brightness correlation.
pub const COLOR_SYNTH_W_NOISE_BRIGHT_INV: f32 = 0.45;

/// Color boost: minimum color evidence to activate additive boost.
pub const COLOR_SYNTH_GATE: f32 = 0.40;

/// Color boost: scale factor for evidence above gate.
pub const COLOR_SYNTH_BOOST_SCALE: f32 = 0.45;

/// Mean per-pixel |R-G|+|R-B|+|G-B| below which image is treated as grayscale.
pub const GRAYSCALE_MEAN_DIFF_THRESHOLD: f64 = 1.5;

// ─── Fast-mode fusion weights ────────────────────────────────────────────

pub const FAST_SYN_W_BLOCK_ART: f32 = 0.30;
pub const FAST_SYN_W_NOISE_INV: f32 = 0.25;
pub const FAST_SYN_W_EDGE_INV: f32 = 0.15;
pub const FAST_SYN_W_BLOCK_VAR: f32 = 0.30;

pub const FAST_EDT_W_BLOCK_VAR: f32 = 0.35;
pub const FAST_EDT_W_BLOCK_ART: f32 = 0.25;
pub const FAST_EDT_W_EDGE: f32 = 0.20;
pub const FAST_EDT_W_NOISE: f32 = 0.20;

pub const FAST_LC_BLOCK_ART: f32 = 0.35;
pub const FAST_LC_NOISE_INV: f32 = 0.25;
pub const FAST_LC_EDGE_INV: f32 = 0.20;
pub const FAST_LC_BLOCK_VAR: f32 = 0.20;

// ─── Runtime calibration config ──────────────────────────────────────────

/// M3: Runtime-overridable calibration parameters.
///
/// All fields default to the compile-time constants above. A partial TOML
/// file can override any subset. Fields use `snake_case` identifiers that
/// match the constant names (lowercased).
///
/// # Example TOML
/// ```toml
/// synthetic_min_threshold = 0.70
/// suspicious_min_threshold = 0.58
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CalibrationConfig {
    // ── Classification thresholds ────────────────────────────────────
    pub synthetic_min_threshold: f32,
    pub synthetic_margin_threshold: f32,
    pub suspicious_min_threshold: f32,
    pub indeterminate_ceiling: f32,
    pub indeterminate_min_spread: f32,
    pub indeterminate_score: f32,

    // ── Reason code thresholds ───────────────────────────────────────
    pub reason_code_contribution_threshold: f32,
    pub semantic_cue_escalation_threshold: f32,
    pub semantic_repetition_escalation_threshold: f32,
    pub semantic_entropy_escalation_ceiling: f32,

    // ── Input limits ─────────────────────────────────────────────────
    pub max_file_size_bytes: usize,
    pub max_image_dimension: u32,

    // ── FFT parameters ───────────────────────────────────────────────
    pub fft_window_cap: usize,
    pub fft_min_dim: usize,
    pub fft_high_freq_radius: f32,
    pub fft_peak_normalization: f32,

    // ── Deep-mode fusion: synthetic base ─────────────────────────────
    pub syn_w_block_artifact: f32,
    pub syn_w_noise_inv: f32,
    pub syn_w_edge_inv: f32,
    pub syn_w_spectral_peak: f32,
    pub syn_w_hf_ratio_inv: f32,
    pub syn_w_prnu_inv: f32,
    pub syn_w_consistency_inv: f32,
    pub syn_w_hybrid_local: f32,
    pub syn_w_hybrid_seam: f32,
    pub syn_w_semantic_cue: f32,

    // ── Deep-mode fusion: synthetic suppression ──────────────────────
    pub syn_supp_prnu: f32,
    pub syn_supp_consistency: f32,
    pub syn_supp_hf_ratio: f32,
    pub syn_supp_floor: f32,

    // ── Deep-mode fusion: edited base ────────────────────────────────
    pub edt_w_block_var_cv: f32,
    pub edt_w_edge: f32,
    pub edt_w_block_artifact: f32,
    pub edt_w_spectral_peak: f32,
    pub edt_spectral_damp: f32,
    pub edt_w_consistency_inv: f32,
    pub edt_w_prnu_inv: f32,
    pub edt_w_hybrid_local: f32,
    pub edt_w_hybrid_seam: f32,
    pub edt_w_semantic_cue: f32,

    // ── Deep-mode fusion: edited suppression ─────────────────────────
    pub edt_supp_prnu: f32,
    pub edt_supp_consistency: f32,
    pub edt_supp_floor: f32,

    // ── Deep-mode fusion: authentic complement ───────────────────────
    pub auth_w_synthetic: f32,
    pub auth_w_edited: f32,

    // ── Score mapping ────────────────────────────────────────────────
    pub synthetic_score_scale: f32,
    pub synthetic_score_min: f32,
    pub synthetic_score_max: f32,
    pub suspicious_score_base: f32,
    pub suspicious_score_range: f32,
    pub suspicious_score_min: f32,
    pub suspicious_score_max: f32,
    pub authentic_score_base: f32,
    pub authentic_score_range: f32,
    pub authentic_score_min: f32,
    pub authentic_score_max: f32,

    // ── Layer contribution weights ───────────────────────────────────
    pub lc_signal_block_art: f32,
    pub lc_signal_noise_inv: f32,
    pub lc_signal_edge_inv: f32,
    pub lc_signal_spectral: f32,
    pub lc_signal_hf_inv: f32,
    pub lc_physical_prnu_inv: f32,
    pub lc_physical_consist_inv: f32,
    pub lc_hybrid_local: f32,
    pub lc_hybrid_seam: f32,

    // ── Pixel-level metric normalization ─────────────────────────────
    pub noise_normalization: f64,
    pub edge_normalization: f64,
    pub block_artifact_normalization: f64,
    pub block_var_cv_normalization: f64,
    pub block_var_block_size: usize,

    // ── PRNU proxy parameters ────────────────────────────────────────
    pub prnu_block_size: usize,
    pub prnu_plaus_offset: f32,
    pub prnu_plaus_scale: f32,
    pub prnu_consist_scale: f32,
    pub prnu_coverage_denom: f32,
    pub prnu_coverage_floor: f32,

    // ── Hybrid parameters ────────────────────────────────────────────
    pub hybrid_min_dim: usize,
    pub hybrid_tile_min: usize,
    pub hybrid_tile_max: usize,
    pub hybrid_energy_floor: f32,
    pub hybrid_seam_normalization: f32,
    pub hybrid_coverage_denom: f32,
    pub hybrid_coverage_floor: f32,

    // ── Semantic parameters ──────────────────────────────────────────
    pub semantic_min_dim: usize,
    pub semantic_gradient_bins: usize,
    pub semantic_rep_offset: f32,
    pub semantic_rep_scale: f32,
    pub semantic_cue_w_repetition: f32,
    pub semantic_cue_w_entropy_inv: f32,

    // ── Color forensic parameters ────────────────────────────────────
    pub noise_brightness_bins: usize,
    pub noise_brightness_min_samples: u64,
    pub color_synth_w_channel_corr: f32,
    pub color_synth_w_noise_bright_inv: f32,
    pub color_synth_gate: f32,
    pub color_synth_boost_scale: f32,
    pub grayscale_mean_diff_threshold: f64,

    // ── Fast-mode fusion weights ─────────────────────────────────────
    pub fast_syn_w_block_art: f32,
    pub fast_syn_w_noise_inv: f32,
    pub fast_syn_w_edge_inv: f32,
    pub fast_syn_w_block_var: f32,
    pub fast_edt_w_block_var: f32,
    pub fast_edt_w_block_art: f32,
    pub fast_edt_w_edge: f32,
    pub fast_edt_w_noise: f32,
    pub fast_lc_block_art: f32,
    pub fast_lc_noise_inv: f32,
    pub fast_lc_edge_inv: f32,
    pub fast_lc_block_var: f32,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            synthetic_min_threshold: SYNTHETIC_MIN_THRESHOLD,
            synthetic_margin_threshold: SYNTHETIC_MARGIN_THRESHOLD,
            suspicious_min_threshold: SUSPICIOUS_MIN_THRESHOLD,
            indeterminate_ceiling: INDETERMINATE_CEILING,
            indeterminate_min_spread: INDETERMINATE_MIN_SPREAD,
            indeterminate_score: INDETERMINATE_SCORE,
            reason_code_contribution_threshold: REASON_CODE_CONTRIBUTION_THRESHOLD,
            semantic_cue_escalation_threshold: SEMANTIC_CUE_ESCALATION_THRESHOLD,
            semantic_repetition_escalation_threshold: SEMANTIC_REPETITION_ESCALATION_THRESHOLD,
            semantic_entropy_escalation_ceiling: SEMANTIC_ENTROPY_ESCALATION_CEILING,
            max_file_size_bytes: MAX_FILE_SIZE_BYTES,
            max_image_dimension: MAX_IMAGE_DIMENSION,
            fft_window_cap: FFT_WINDOW_CAP,
            fft_min_dim: FFT_MIN_DIM,
            fft_high_freq_radius: FFT_HIGH_FREQ_RADIUS,
            fft_peak_normalization: FFT_PEAK_NORMALIZATION,
            syn_w_block_artifact: SYN_W_BLOCK_ARTIFACT,
            syn_w_noise_inv: SYN_W_NOISE_INV,
            syn_w_edge_inv: SYN_W_EDGE_INV,
            syn_w_spectral_peak: SYN_W_SPECTRAL_PEAK,
            syn_w_hf_ratio_inv: SYN_W_HF_RATIO_INV,
            syn_w_prnu_inv: SYN_W_PRNU_INV,
            syn_w_consistency_inv: SYN_W_CONSISTENCY_INV,
            syn_w_hybrid_local: SYN_W_HYBRID_LOCAL,
            syn_w_hybrid_seam: SYN_W_HYBRID_SEAM,
            syn_w_semantic_cue: SYN_W_SEMANTIC_CUE,
            syn_supp_prnu: SYN_SUPP_PRNU,
            syn_supp_consistency: SYN_SUPP_CONSISTENCY,
            syn_supp_hf_ratio: SYN_SUPP_HF_RATIO,
            syn_supp_floor: SYN_SUPP_FLOOR,
            edt_w_block_var_cv: EDT_W_BLOCK_VAR_CV,
            edt_w_edge: EDT_W_EDGE,
            edt_w_block_artifact: EDT_W_BLOCK_ARTIFACT,
            edt_w_spectral_peak: EDT_W_SPECTRAL_PEAK,
            edt_spectral_damp: EDT_SPECTRAL_DAMP,
            edt_w_consistency_inv: EDT_W_CONSISTENCY_INV,
            edt_w_prnu_inv: EDT_W_PRNU_INV,
            edt_w_hybrid_local: EDT_W_HYBRID_LOCAL,
            edt_w_hybrid_seam: EDT_W_HYBRID_SEAM,
            edt_w_semantic_cue: EDT_W_SEMANTIC_CUE,
            edt_supp_prnu: EDT_SUPP_PRNU,
            edt_supp_consistency: EDT_SUPP_CONSISTENCY,
            edt_supp_floor: EDT_SUPP_FLOOR,
            auth_w_synthetic: AUTH_W_SYNTHETIC,
            auth_w_edited: AUTH_W_EDITED,
            synthetic_score_scale: SYNTHETIC_SCORE_SCALE,
            synthetic_score_min: SYNTHETIC_SCORE_MIN,
            synthetic_score_max: SYNTHETIC_SCORE_MAX,
            suspicious_score_base: SUSPICIOUS_SCORE_BASE,
            suspicious_score_range: SUSPICIOUS_SCORE_RANGE,
            suspicious_score_min: SUSPICIOUS_SCORE_MIN,
            suspicious_score_max: SUSPICIOUS_SCORE_MAX,
            authentic_score_base: AUTHENTIC_SCORE_BASE,
            authentic_score_range: AUTHENTIC_SCORE_RANGE,
            authentic_score_min: AUTHENTIC_SCORE_MIN,
            authentic_score_max: AUTHENTIC_SCORE_MAX,
            lc_signal_block_art: LC_SIGNAL_BLOCK_ART,
            lc_signal_noise_inv: LC_SIGNAL_NOISE_INV,
            lc_signal_edge_inv: LC_SIGNAL_EDGE_INV,
            lc_signal_spectral: LC_SIGNAL_SPECTRAL,
            lc_signal_hf_inv: LC_SIGNAL_HF_INV,
            lc_physical_prnu_inv: LC_PHYSICAL_PRNU_INV,
            lc_physical_consist_inv: LC_PHYSICAL_CONSIST_INV,
            lc_hybrid_local: LC_HYBRID_LOCAL,
            lc_hybrid_seam: LC_HYBRID_SEAM,
            noise_normalization: NOISE_NORMALIZATION,
            edge_normalization: EDGE_NORMALIZATION,
            block_artifact_normalization: BLOCK_ARTIFACT_NORMALIZATION,
            block_var_cv_normalization: BLOCK_VAR_CV_NORMALIZATION,
            block_var_block_size: BLOCK_VAR_BLOCK_SIZE,
            prnu_block_size: PRNU_BLOCK_SIZE,
            prnu_plaus_offset: PRNU_PLAUS_OFFSET,
            prnu_plaus_scale: PRNU_PLAUS_SCALE,
            prnu_consist_scale: PRNU_CONSIST_SCALE,
            prnu_coverage_denom: PRNU_COVERAGE_DENOM,
            prnu_coverage_floor: PRNU_COVERAGE_FLOOR,
            hybrid_min_dim: HYBRID_MIN_DIM,
            hybrid_tile_min: HYBRID_TILE_MIN,
            hybrid_tile_max: HYBRID_TILE_MAX,
            hybrid_energy_floor: HYBRID_ENERGY_FLOOR,
            hybrid_seam_normalization: HYBRID_SEAM_NORMALIZATION,
            hybrid_coverage_denom: HYBRID_COVERAGE_DENOM,
            hybrid_coverage_floor: HYBRID_COVERAGE_FLOOR,
            semantic_min_dim: SEMANTIC_MIN_DIM,
            semantic_gradient_bins: SEMANTIC_GRADIENT_BINS,
            semantic_rep_offset: SEMANTIC_REP_OFFSET,
            semantic_rep_scale: SEMANTIC_REP_SCALE,
            semantic_cue_w_repetition: SEMANTIC_CUE_W_REPETITION,
            semantic_cue_w_entropy_inv: SEMANTIC_CUE_W_ENTROPY_INV,
            noise_brightness_bins: NOISE_BRIGHTNESS_BINS,
            noise_brightness_min_samples: NOISE_BRIGHTNESS_MIN_SAMPLES,
            color_synth_w_channel_corr: COLOR_SYNTH_W_CHANNEL_CORR,
            color_synth_w_noise_bright_inv: COLOR_SYNTH_W_NOISE_BRIGHT_INV,
            color_synth_gate: COLOR_SYNTH_GATE,
            color_synth_boost_scale: COLOR_SYNTH_BOOST_SCALE,
            grayscale_mean_diff_threshold: GRAYSCALE_MEAN_DIFF_THRESHOLD,
            fast_syn_w_block_art: FAST_SYN_W_BLOCK_ART,
            fast_syn_w_noise_inv: FAST_SYN_W_NOISE_INV,
            fast_syn_w_edge_inv: FAST_SYN_W_EDGE_INV,
            fast_syn_w_block_var: FAST_SYN_W_BLOCK_VAR,
            fast_edt_w_block_var: FAST_EDT_W_BLOCK_VAR,
            fast_edt_w_block_art: FAST_EDT_W_BLOCK_ART,
            fast_edt_w_edge: FAST_EDT_W_EDGE,
            fast_edt_w_noise: FAST_EDT_W_NOISE,
            fast_lc_block_art: FAST_LC_BLOCK_ART,
            fast_lc_noise_inv: FAST_LC_NOISE_INV,
            fast_lc_edge_inv: FAST_LC_EDGE_INV,
            fast_lc_block_var: FAST_LC_BLOCK_VAR,
        }
    }
}