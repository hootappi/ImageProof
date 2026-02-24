use imageproof_core::{
    verify, ExecutionMode, HardwareTier, VerificationClass, VerifyError, VerifyRequest,
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

    if args.len() >= 3 && args[1].eq_ignore_ascii_case("stress") {
        let dataset_root = PathBuf::from(&args[2]);
        if let Err(err) = run_stress_test(&dataset_root) {
            eprintln!("Stress test failed: {err}");
            std::process::exit(1);
        }
        return;
    }

    println!("ImageProof launch successful.");
    println!("Run stress test with: cargo run -p imageproof-cli -- stress <dataset_root>");
    println!("Expected dataset folders: authentic/, edited/, synthetic/");
}

fn run_stress_test(dataset_root: &Path) -> Result<(), String> {
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

            let request = VerifyRequest {
                image_bytes: bytes,
                execution_mode: ExecutionMode::Deep,
                hardware_tier: HardwareTier::CpuOnly,
            };

            match verify(request) {
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

        if entry_path.is_dir() {
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
    let full = path.to_string_lossy().to_ascii_lowercase();

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
        if full.contains(needle) {
            let label = label.to_string();
            if !tags.contains(&label) {
                tags.push(label);
            }
        }
    }

    tags
}
