use imageproof_core::{
    verify_bytes_with_config, CalibrationConfig, ExecutionMode, VerificationClass,
    VerifyError,
};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ExpectedClass {
    Authentic,
    Suspicious,
    Synthetic,
}

const MIN_SAMPLES_PER_CLASS: u32 = 25;
const MAX_AUTHENTIC_FALSE_POSITIVE_RATE: f32 = 0.01;
const MAX_SUSPICIOUS_MISS_RATE: f32 = 0.10;
const MAX_SYNTHETIC_MISS_RATE: f32 = 0.10;

impl ExpectedClass {
    fn folder_name(self) -> &'static str {
        match self {
            Self::Authentic => "authentic",
            Self::Suspicious => "edited",
            Self::Synthetic => "synthetic",
        }
    }

    fn as_label(self) -> &'static str {
        match self {
            Self::Authentic => "Authentic",
            Self::Suspicious => "Suspicious",
            Self::Synthetic => "Synthetic",
        }
    }
}

#[derive(Default)]
struct GroupStats {
    total: u32,
    matched: u32,
    as_authentic: u32,
    as_suspicious: u32,
    as_synthetic: u32,
}

impl GroupStats {
    fn record(&mut self, expected: ExpectedClass, predicted: VerificationClass) {
        self.total += 1;

        match predicted {
            VerificationClass::Authentic => self.as_authentic += 1,
            VerificationClass::Suspicious => self.as_suspicious += 1,
            VerificationClass::Synthetic => self.as_synthetic += 1,
            VerificationClass::Indeterminate => {}
        }

        let is_match = matches!(
            (expected, predicted),
            (ExpectedClass::Authentic, VerificationClass::Authentic)
                | (ExpectedClass::Suspicious, VerificationClass::Suspicious)
                | (ExpectedClass::Synthetic, VerificationClass::Synthetic)
        );

        if is_match {
            self.matched += 1;
        }
    }

    fn accuracy(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.matched as f32 / self.total as f32
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // M3: Parse optional --config <path.toml> from any position in args.
    let cfg = match parse_config_arg(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    // Filter out --config and its value for positional arg parsing.
    let positional: Vec<&str> = filter_positional_args(&args);

    if positional.len() >= 2 && positional[0].eq_ignore_ascii_case("stress") {
        let dataset_root = PathBuf::from(positional[1]);
        if let Err(err) = run_stress_test(&dataset_root, &cfg) {
            eprintln!("Stress test failed: {err}");
            std::process::exit(1);
        }
        return;
    }

    println!("ImageProof launch successful.");
    println!("Run stress test with: cargo run -p imageproof-cli -- stress <dataset_root>");
    println!("Options: --config <path.toml> to override calibration parameters.");
    println!("Expected dataset folders: authentic/, edited/, synthetic/");
}

/// Parse --config <path.toml> and load a CalibrationConfig.
/// Returns default config if --config is not specified.
fn parse_config_arg(args: &[String]) -> Result<CalibrationConfig, String> {
    for i in 0..args.len() {
        if args[i] == "--config" {
            let path = args
                .get(i + 1)
                .ok_or_else(|| "--config requires a file path argument".to_string())?;
            let content = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read config file '{}': {e}", path))?;
            let cfg: CalibrationConfig = toml::from_str(&content)
                .map_err(|e| format!("Failed to parse config TOML '{}': {e}", path))?;
            return Ok(cfg);
        }
    }
    Ok(CalibrationConfig::default())
}

/// Return positional args (skip argv[0] and --config pairs).
fn filter_positional_args(args: &[String]) -> Vec<&str> {
    let mut result = Vec::new();
    let mut skip_next = false;
    for (i, arg) in args.iter().enumerate() {
        if i == 0 {
            continue; // skip binary name
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--config" {
            skip_next = true;
            continue;
        }
        result.push(arg.as_str());
    }
    result
}

fn run_stress_test(dataset_root: &Path, cfg: &CalibrationConfig) -> Result<(), String> {
    if !dataset_root.is_dir() {
        return Err(format!("Dataset path is not a directory: {}", dataset_root.display()));
    }

    let classes = [
        ExpectedClass::Authentic,
        ExpectedClass::Suspicious,
        ExpectedClass::Synthetic,
    ];

    let mut total_stats = GroupStats::default();
    let mut per_class: HashMap<ExpectedClass, GroupStats> = HashMap::new();
    let mut per_tag: HashMap<String, GroupStats> = HashMap::new();
    let mut decode_errors = 0u32;

    for expected in classes {
        let dir = dataset_root.join(expected.folder_name());
        if !dir.is_dir() {
            return Err(format!(
                "Missing required class folder: {}",
                dir.display()
            ));
        }

        let image_paths = collect_image_files(&dir)?;
        if image_paths.is_empty() {
            return Err(format!("No images found in {}", dir.display()));
        }

        for image_path in image_paths {
            let bytes = fs::read(&image_path)
                .map_err(|e| format!("Failed reading {}: {e}", image_path.display()))?;

            // M6: use verify_bytes to avoid unnecessary Vec<u8> copy.
            // M3: use config-aware variant for runtime threshold overrides.
            match verify_bytes_with_config(&bytes, ExecutionMode::Deep, cfg) {
                Ok(result) => {
                    let predicted = result.classification;
                    total_stats.record(expected, predicted);
                    per_class.entry(expected).or_default().record(expected, predicted);

                    for tag in derive_perturbation_tags(&image_path) {
                        per_tag.entry(tag).or_default().record(expected, predicted);
                    }
                }
                Err(VerifyError::DecodeFailed) => {
                    decode_errors += 1;
                }
                Err(other) => {
                    return Err(format!(
                        "Verification error on {}: {other}",
                        image_path.display()
                    ));
                }
            }
        }
    }

    println!("=== ImageProof Stress Test Report ===");
    println!("Dataset root: {}", dataset_root.display());
    println!("Samples evaluated: {}", total_stats.total);
    println!("Decode failures: {decode_errors}");
    println!("Overall accuracy: {:.2}%", total_stats.accuracy() * 100.0);
    println!();

    println!("Per-class accuracy:");
    for expected in classes {
        let stats = per_class.get(&expected);
        let matched = stats.map(|s| s.matched).unwrap_or(0);
        let total = stats.map(|s| s.total).unwrap_or(0);
        let accuracy = stats.map(|s| s.accuracy()).unwrap_or(0.0);
        let as_authentic = stats.map(|s| s.as_authentic).unwrap_or(0);
        let as_suspicious = stats.map(|s| s.as_suspicious).unwrap_or(0);
        let as_synthetic = stats.map(|s| s.as_synthetic).unwrap_or(0);
        println!(
            "- {:>10}: {:>4}/{:<4} ({:>6.2}%)  [A:{} S:{} G:{}]",
            expected.as_label(),
            matched,
            total,
            accuracy * 100.0,
            as_authentic,
            as_suspicious,
            as_synthetic
        );
    }

    if !per_tag.is_empty() {
        let mut tags: Vec<_> = per_tag.into_iter().collect();
        tags.sort_by(|a, b| a.0.cmp(&b.0));

        println!();
        println!("Perturbation-tag accuracy:");
        for (tag, stats) in tags {
            println!(
                "- {:>14}: {:>4}/{:<4} ({:>6.2}%)",
                tag,
                stats.matched,
                stats.total,
                stats.accuracy() * 100.0
            );
        }
    }

    println!();
    println!("Acceptance quality bar:");
    let quality = evaluate_acceptance_quality(&per_class);
    println!("- status: {}", if quality.passed { "PASS" } else { "FAIL" });
    println!("- min samples per class: {MIN_SAMPLES_PER_CLASS}");
    println!(
        "- authentic false-positive rate: {:.2}% (limit {:.2}%)",
        quality.authentic_false_positive_rate * 100.0,
        MAX_AUTHENTIC_FALSE_POSITIVE_RATE * 100.0
    );
    println!(
        "- edited miss rate (predicted authentic): {:.2}% (limit {:.2}%)",
        quality.suspicious_miss_rate * 100.0,
        MAX_SUSPICIOUS_MISS_RATE * 100.0
    );
    println!(
        "- synthetic miss rate (predicted authentic): {:.2}% (limit {:.2}%)",
        quality.synthetic_miss_rate * 100.0,
        MAX_SYNTHETIC_MISS_RATE * 100.0
    );

    if !quality.notes.is_empty() {
        println!("- notes:");
        for note in quality.notes {
            println!("  - {note}");
        }
    }

    Ok(())
}

struct QualityAssessment {
    passed: bool,
    authentic_false_positive_rate: f32,
    suspicious_miss_rate: f32,
    synthetic_miss_rate: f32,
    notes: Vec<String>,
}

fn evaluate_acceptance_quality(per_class: &HashMap<ExpectedClass, GroupStats>) -> QualityAssessment {
    let auth = per_class.get(&ExpectedClass::Authentic);
    let edit = per_class.get(&ExpectedClass::Suspicious);
    let synth = per_class.get(&ExpectedClass::Synthetic);

    let authentic_total = auth.map(|s| s.total).unwrap_or(0);
    let edited_total = edit.map(|s| s.total).unwrap_or(0);
    let synthetic_total = synth.map(|s| s.total).unwrap_or(0);

    let authentic_fp = auth
        .map(|s| s.as_suspicious + s.as_synthetic)
        .unwrap_or(0);
    let edited_miss = edit.map(|s| s.as_authentic).unwrap_or(0);
    let synthetic_miss = synth.map(|s| s.as_authentic).unwrap_or(0);

    let authentic_false_positive_rate = if authentic_total == 0 {
        1.0
    } else {
        authentic_fp as f32 / authentic_total as f32
    };
    let suspicious_miss_rate = if edited_total == 0 {
        1.0
    } else {
        edited_miss as f32 / edited_total as f32
    };
    let synthetic_miss_rate = if synthetic_total == 0 {
        1.0
    } else {
        synthetic_miss as f32 / synthetic_total as f32
    };

    let mut notes = Vec::new();
    if authentic_total < MIN_SAMPLES_PER_CLASS {
        notes.push(format!(
            "authentic sample size below minimum ({} < {})",
            authentic_total, MIN_SAMPLES_PER_CLASS
        ));
    }
    if edited_total < MIN_SAMPLES_PER_CLASS {
        notes.push(format!(
            "edited sample size below minimum ({} < {})",
            edited_total, MIN_SAMPLES_PER_CLASS
        ));
    }
    if synthetic_total < MIN_SAMPLES_PER_CLASS {
        notes.push(format!(
            "synthetic sample size below minimum ({} < {})",
            synthetic_total, MIN_SAMPLES_PER_CLASS
        ));
    }

    if authentic_false_positive_rate > MAX_AUTHENTIC_FALSE_POSITIVE_RATE {
        notes.push("authentic false-positive rate exceeds limit".to_string());
    }
    if suspicious_miss_rate > MAX_SUSPICIOUS_MISS_RATE {
        notes.push("edited miss rate exceeds limit".to_string());
    }
    if synthetic_miss_rate > MAX_SYNTHETIC_MISS_RATE {
        notes.push("synthetic miss rate exceeds limit".to_string());
    }

    QualityAssessment {
        passed: notes.is_empty(),
        authentic_false_positive_rate,
        suspicious_miss_rate,
        synthetic_miss_rate,
        notes,
    }
}

fn collect_image_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    collect_recursive(root, &mut out)?;
    Ok(out)
}

fn collect_recursive(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(path)
        .map_err(|e| format!("Failed reading directory {}: {e}", path.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed reading directory entry: {e}"))?;
        let entry_path = entry.path();

        // H6: detect symlinks via DirEntry::file_type (does NOT follow symlinks,
        // unlike Path::is_dir / Path::is_file which do follow them).
        let file_type = entry.file_type()
            .map_err(|e| format!("Failed reading file type for {}: {e}", entry_path.display()))?;

        if file_type.is_symlink() {
            eprintln!("WARN: skipping symlink: {}", entry_path.display());
            continue;
        }

        if file_type.is_dir() {
            collect_recursive(&entry_path, out)?;
            continue;
        }

        if is_supported_image(&entry_path) {
            out.push(entry_path);
        }
    }

    Ok(())
}

fn is_supported_image(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };

    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "webp"
    )
}

fn derive_perturbation_tags(path: &Path) -> Vec<String> {
    let mut tags = Vec::new();

    // H5: match keywords only against the filename stem — never against the
    // extension or directory path components. This prevents every `.jpg` file
    // from being spuriously tagged as "jpeg".
    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s.to_ascii_lowercase(),
        None => return tags,
    };

    let patterns = [
        ("jpeg", "jpeg"),
        ("jpg", "jpeg"),
        ("webp", "webp"),
        ("resize", "resized"),
        ("resized", "resized"),
        ("crop", "cropped"),
        ("cropped", "cropped"),
        ("recompress", "recompressed"),
        ("recompressed", "recompressed"),
        ("night", "lowlight"),
        ("lowlight", "lowlight"),
    ];

    for (needle, label) in patterns {
        if stem.contains(needle) {
            let label = label.to_string();
            if !tags.contains(&label) {
                tags.push(label);
            }
        }
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // is_supported_image
    // ---------------------------------------------------------------

    #[test]
    fn supported_image_jpg() {
        assert!(is_supported_image(Path::new("photo.jpg")));
    }

    #[test]
    fn supported_image_jpeg_uppercase() {
        assert!(is_supported_image(Path::new("photo.JPEG")));
    }

    #[test]
    fn supported_image_png() {
        assert!(is_supported_image(Path::new("image.png")));
    }

    #[test]
    fn supported_image_webp() {
        assert!(is_supported_image(Path::new("image.webp")));
    }

    #[test]
    fn unsupported_image_bmp() {
        assert!(!is_supported_image(Path::new("image.bmp")));
    }

    #[test]
    fn unsupported_image_gif() {
        assert!(!is_supported_image(Path::new("animation.gif")));
    }

    #[test]
    fn unsupported_image_no_extension() {
        assert!(!is_supported_image(Path::new("README")));
    }

    // ---------------------------------------------------------------
    // derive_perturbation_tags — H5 fixed: matches filename stem only,
    // not extension or directory path components.
    // ---------------------------------------------------------------

    #[test]
    fn perturbation_tag_recompressed_in_name() {
        let tags = derive_perturbation_tags(Path::new("photo_recompressed.png"));
        assert!(tags.contains(&"recompressed".to_string()));
    }

    #[test]
    fn perturbation_tag_resized_in_name() {
        let tags = derive_perturbation_tags(Path::new("photo_resized_50pct.jpg"));
        assert!(tags.contains(&"resized".to_string()));
    }

    #[test]
    fn perturbation_tag_cropped_in_name() {
        let tags = derive_perturbation_tags(Path::new("photo_cropped.png"));
        assert!(tags.contains(&"cropped".to_string()));
    }

    #[test]
    fn perturbation_tag_lowlight_in_name() {
        let tags = derive_perturbation_tags(Path::new("scene_night_01.jpg"));
        assert!(tags.contains(&"lowlight".to_string()));
    }

    #[test]
    fn perturbation_tag_no_false_tags_on_clean_name() {
        let tags = derive_perturbation_tags(Path::new("clean_photo.png"));
        assert!(tags.is_empty(), "expected no tags, got: {tags:?}");
    }

    // H5: extension must NOT produce tags
    #[test]
    fn h5_plain_jpg_no_jpeg_tag() {
        let tags = derive_perturbation_tags(Path::new("photo.jpg"));
        assert!(!tags.contains(&"jpeg".to_string()), "plain .jpg must not produce jpeg tag, got: {tags:?}");
    }

    #[test]
    fn h5_plain_jpeg_no_jpeg_tag() {
        let tags = derive_perturbation_tags(Path::new("photo.jpeg"));
        assert!(!tags.contains(&"jpeg".to_string()), "plain .jpeg must not produce jpeg tag, got: {tags:?}");
    }

    #[test]
    fn h5_plain_webp_no_webp_tag() {
        let tags = derive_perturbation_tags(Path::new("photo.webp"));
        assert!(!tags.contains(&"webp".to_string()), "plain .webp must not produce webp tag, got: {tags:?}");
    }

    // H5: stem keywords still produce correct tags
    #[test]
    fn h5_recompressed_jpeg80_in_stem_gets_tag() {
        let tags = derive_perturbation_tags(Path::new("photo_recompressed_jpeg80.jpg"));
        assert!(tags.contains(&"recompressed".to_string()));
        assert!(tags.contains(&"jpeg".to_string()));
    }

    // H5: directory path components must NOT produce tags
    #[test]
    fn h5_directory_name_ignored() {
        let tags = derive_perturbation_tags(Path::new("dataset/recompressed/photo.png"));
        assert!(tags.is_empty(), "directory component must not produce tags, got: {tags:?}");
    }

    // ---------------------------------------------------------------
    // GroupStats
    // ---------------------------------------------------------------

    #[test]
    fn group_stats_record_correct_match() {
        let mut stats = GroupStats::default();
        stats.record(ExpectedClass::Authentic, VerificationClass::Authentic);
        assert_eq!(stats.total, 1);
        assert_eq!(stats.matched, 1);
        assert_eq!(stats.as_authentic, 1);
    }

    #[test]
    fn group_stats_record_mismatch() {
        let mut stats = GroupStats::default();
        stats.record(ExpectedClass::Authentic, VerificationClass::Suspicious);
        assert_eq!(stats.total, 1);
        assert_eq!(stats.matched, 0);
        assert_eq!(stats.as_suspicious, 1);
    }

    #[test]
    fn group_stats_accuracy_calculation() {
        let mut stats = GroupStats::default();
        stats.record(ExpectedClass::Synthetic, VerificationClass::Synthetic);
        stats.record(ExpectedClass::Synthetic, VerificationClass::Authentic);
        stats.record(ExpectedClass::Synthetic, VerificationClass::Synthetic);
        stats.record(ExpectedClass::Synthetic, VerificationClass::Suspicious);
        assert_eq!(stats.total, 4);
        assert_eq!(stats.matched, 2);
        assert!((stats.accuracy() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn group_stats_empty_accuracy_is_zero() {
        let stats = GroupStats::default();
        assert_eq!(stats.accuracy(), 0.0);
    }

    #[test]
    fn group_stats_indeterminate_not_counted_as_match() {
        let mut stats = GroupStats::default();
        stats.record(ExpectedClass::Authentic, VerificationClass::Indeterminate);
        assert_eq!(stats.total, 1);
        assert_eq!(stats.matched, 0);
        assert_eq!(stats.as_authentic, 0);
        assert_eq!(stats.as_suspicious, 0);
        assert_eq!(stats.as_synthetic, 0);
    }

    // ---------------------------------------------------------------
    // H6: collect_recursive symlink protection
    // ---------------------------------------------------------------

    #[test]
    fn h6_collect_recursive_normal_files() {
        // Normal directory with image files — should collect them
        let tmp = std::env::temp_dir().join("imageproof_h6_normal");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("photo.jpg"), b"fake").unwrap();
        fs::write(tmp.join("readme.txt"), b"text").unwrap();
        let mut out = Vec::new();
        collect_recursive(&tmp, &mut out).unwrap();
        assert_eq!(out.len(), 1, "should collect only .jpg, got: {out:?}");
        assert!(out[0].ends_with("photo.jpg"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn h6_collect_recursive_nested_dirs() {
        let tmp = std::env::temp_dir().join("imageproof_h6_nested");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("sub")).unwrap();
        fs::write(tmp.join("a.png"), b"fake").unwrap();
        fs::write(tmp.join("sub").join("b.png"), b"fake").unwrap();
        let mut out = Vec::new();
        collect_recursive(&tmp, &mut out).unwrap();
        assert_eq!(out.len(), 2, "should collect both images, got: {out:?}");
        let _ = fs::remove_dir_all(&tmp);
    }

    /// Symlink integration test — only runs on Unix where symlinks
    /// don't require elevated privileges.
    #[test]
    #[cfg(unix)]
    fn h6_collect_recursive_skips_symlink_file() {
        let tmp = std::env::temp_dir().join("imageproof_h6_symfile");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("real.png"), b"fake").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", tmp.join("link.png")).unwrap();
        let mut out = Vec::new();
        collect_recursive(&tmp, &mut out).unwrap();
        assert_eq!(out.len(), 1, "symlink file should be skipped, got: {out:?}");
        assert!(out[0].ends_with("real.png"));
        let _ = fs::remove_dir_all(&tmp);
    }

    /// Symlink directory test — only runs on Unix.
    #[test]
    #[cfg(unix)]
    fn h6_collect_recursive_skips_symlink_dir() {
        let tmp = std::env::temp_dir().join("imageproof_h6_symdir");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("real.jpg"), b"fake").unwrap();
        std::os::unix::fs::symlink("/tmp", tmp.join("link_dir")).unwrap();
        let mut out = Vec::new();
        collect_recursive(&tmp, &mut out).unwrap();
        assert_eq!(out.len(), 1, "symlink dir should be skipped, got: {out:?}");
        let _ = fs::remove_dir_all(&tmp);
    }

    /// Windows symlink test — requires Developer Mode or elevated privileges.
    /// Marked #[ignore] because CI runners typically don't have symlink rights.
    #[test]
    #[cfg(windows)]
    #[ignore]
    fn h6_collect_recursive_skips_symlink_file_windows() {
        let tmp = std::env::temp_dir().join("imageproof_h6_win_sym");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let target = tmp.join("target.png");
        fs::write(&target, b"fake").unwrap();
        // Create a symlink alongside the real file
        std::os::windows::fs::symlink_file(&target, tmp.join("link.png")).unwrap();
        let mut out = Vec::new();
        collect_recursive(&tmp, &mut out).unwrap();
        // Should collect only the real file, not the symlink
        assert_eq!(out.len(), 1, "symlink should be skipped, got: {out:?}");
        assert!(out[0].ends_with("target.png"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
