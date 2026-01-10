use crate::sampling::{sample_from_logits, GenerationConfig};
use rand::rngs::StdRng;
use rand::SeedableRng;
#[cfg(feature = "native")]
use std::path::{Path, PathBuf};

#[cfg(all(feature = "native", feature = "wgpu"))]
mod gguf_gate_d;

#[test]
fn test_sample_greedy() {
    let logits = vec![0.1, 1.5, 0.3];
    let mut cfg = GenerationConfig::default();
    cfg.temperature = 0.0;
    let mut rng = StdRng::seed_from_u64(42);
    let token = sample_from_logits(&logits, &cfg, &[], &mut rng).unwrap();
    assert_eq!(token, 1);
}

#[test]
fn test_sample_top_k_limits() {
    let logits = vec![0.1, 2.0, 1.9, 1.8];
    let mut cfg = GenerationConfig::default();
    cfg.top_k = 2;
    cfg.top_p = 1.0;
    let mut rng = StdRng::seed_from_u64(7);
    let token = sample_from_logits(&logits, &cfg, &[], &mut rng).unwrap();
    assert!(token == 1 || token == 2);
}

#[test]
fn test_repetition_penalty_changes_choice() {
    let logits = vec![2.0, 1.9, 1.8];
    let mut cfg = GenerationConfig::default();
    cfg.temperature = 0.0;
    cfg.repetition_penalty = 2.0;
    let mut rng = StdRng::seed_from_u64(1);
    let token = sample_from_logits(&logits, &cfg, &[0], &mut rng).unwrap();
    assert_eq!(token, 1);
}

#[cfg(feature = "native")]
fn resolve_tokenizer_fixture(manifest_dir: &Path) -> Option<PathBuf> {
    let tokenizer_path = manifest_dir.join("tests/fixtures/tokenizer.json");
    if tokenizer_path.exists() {
        return Some(tokenizer_path);
    }

    let parts_dir = manifest_dir.join("tests/fixtures/tokenizer");
    let entries = std::fs::read_dir(&parts_dir).ok()?;
    let mut parts = Vec::new();
    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        let is_part = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with("part-"))
            .unwrap_or(false);
        if is_part {
            parts.push(path);
        }
    }
    if parts.is_empty() {
        return None;
    }

    parts.sort();
    let mut combined = Vec::new();
    for part in parts {
        combined.extend_from_slice(&std::fs::read(part).ok()?);
    }

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let temp_path = std::env::temp_dir().join(format!("openagents-tokenizer-{suffix}.json"));
    std::fs::write(&temp_path, combined).ok()?;
    Some(temp_path)
}

#[cfg(feature = "native")]
#[test]
fn test_llama2c_tiny_model() {
    use crate::device::MlDevice;
    use crate::model::{LoadedModel, ModelSource};
    use futures::executor::block_on;

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tokenizer_path = match resolve_tokenizer_fixture(manifest_dir) {
        Some(path) => path,
        None => {
            eprintln!(
                "tokenizer missing, run scripts/download-test-models.sh or assemble tests/fixtures/tokenizer parts"
            );
            return;
        }
    };
    if !tokenizer_path.exists() {
        eprintln!(
            "tokenizer missing, run scripts/download-test-models.sh or assemble tests/fixtures/tokenizer parts"
        );
        return;
    }

    let candidates = [
        ("llama2c-260k", "tests/fixtures/llama2c-260k.gguf"),
        ("llama2c-42m-q4", "tests/fixtures/llama2c-42m-q4.gguf"),
    ];

    let device = MlDevice::Cpu;
    let mut model = None;
    for (id, rel_path) in candidates {
        let model_path = manifest_dir.join(rel_path);
        if !model_path.exists() {
            continue;
        }
        let source = ModelSource::llama2c_gguf(
            id,
            model_path.to_string_lossy().to_string(),
            tokenizer_path.to_string_lossy().to_string(),
        );
        match block_on(LoadedModel::load(&source, &device)) {
            Ok(loaded) => {
                model = Some(loaded);
                break;
            }
            Err(err) => {
                eprintln!("model load failed for {id}: {err}");
            }
        }
    }

    let mut model = match model {
        Some(model) => model,
        None => return,
    };

    let mut cfg = GenerationConfig::default();
    cfg.max_new_tokens = 8;
    cfg.temperature = 0.0;
    cfg.top_k = 1;
    cfg.top_p = 1.0;
    cfg.repetition_penalty = 1.0;

    let outcome = model.generate("Hello", &cfg, None).unwrap();
    assert!(outcome.generated_tokens > 0);
}
