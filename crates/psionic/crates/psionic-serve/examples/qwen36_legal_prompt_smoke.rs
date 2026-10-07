use std::{
    collections::BTreeSet,
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

use psionic_models::{
    normalize_qwen36_target_model_id, run_qwen36_legal_prompt_smoke,
    write_qwen36_27b_smoke_safetensors, write_qwen36_35b_a3b_moe_smoke_safetensors,
    QWEN36_27B_MODEL_ID, QWEN36_27B_SMOKE_CONFIG_PATH, QWEN36_27B_SMOKE_SHARD_PATH,
    QWEN36_27B_SMOKE_TOKENIZER_PATH, QWEN36_35B_A3B_MODEL_ID, QWEN36_35B_A3B_SMOKE_CONFIG_PATH,
    QWEN36_35B_A3B_SMOKE_SHARD_PATH, QWEN36_35B_A3B_SMOKE_TOKENIZER_PATH,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let model =
        optional_flag(&args, "--model").unwrap_or_else(|| String::from(QWEN36_27B_MODEL_ID));
    let normalized_model = normalize_qwen36_target_model_id(model.as_str())?;
    let prompt = required_flag(&args, "--prompt")?;
    let model_dir = optional_flag(&args, "--model-dir").map(PathBuf::from);
    let config = optional_flag(&args, "--config").unwrap_or_else(|| {
        if let Some(model_dir) = &model_dir {
            return model_dir.join("config.json").display().to_string();
        }
        if normalized_model == QWEN36_35B_A3B_MODEL_ID {
            String::from(QWEN36_35B_A3B_SMOKE_CONFIG_PATH)
        } else {
            String::from(QWEN36_27B_SMOKE_CONFIG_PATH)
        }
    });
    let tokenizer = optional_flag(&args, "--tokenizer").unwrap_or_else(|| {
        if let Some(model_dir) = &model_dir {
            return model_dir.join("tokenizer.json").display().to_string();
        }
        if normalized_model == QWEN36_35B_A3B_MODEL_ID {
            String::from(QWEN36_35B_A3B_SMOKE_TOKENIZER_PATH)
        } else {
            String::from(QWEN36_27B_SMOKE_TOKENIZER_PATH)
        }
    });
    let shard_paths = if let Some(model_dir) = &model_dir {
        shard_paths_from_model_dir(model_dir)?
    } else {
        let shard = optional_flag(&args, "--shard").unwrap_or_else(|| {
            if normalized_model == QWEN36_35B_A3B_MODEL_ID {
                String::from(QWEN36_35B_A3B_SMOKE_SHARD_PATH)
            } else {
                String::from(QWEN36_27B_SMOKE_SHARD_PATH)
            }
        });
        let shard_path = PathBuf::from(&shard);
        if !shard_path.exists() {
            if normalized_model == QWEN36_35B_A3B_MODEL_ID {
                write_qwen36_35b_a3b_moe_smoke_safetensors(&shard_path)?;
            } else {
                write_qwen36_27b_smoke_safetensors(&shard_path)?;
            }
        }
        vec![shard_path]
    };

    let report = run_qwen36_legal_prompt_smoke(
        model.as_str(),
        PathBuf::from(prompt),
        PathBuf::from(config),
        PathBuf::from(tokenizer),
        shard_paths.as_slice(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn shard_paths_from_model_dir(model_dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let index_path = model_dir.join("model.safetensors.index.json");
    let index_bytes = fs::read(&index_path)?;
    let index = serde_json::from_slice::<serde_json::Value>(index_bytes.as_slice())?;
    let weight_map = index
        .get("weight_map")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "model.safetensors.index.json must contain weight_map",
            )
        })?;
    let mut names = BTreeSet::new();
    for value in weight_map.values() {
        let Some(name) = value.as_str() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "model.safetensors.index.json weight_map values must be strings",
            )
            .into());
        };
        names.insert(String::from(name));
    }
    if names.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "model.safetensors.index.json does not reference any safetensors shards",
        )
        .into());
    }
    let mut paths = Vec::with_capacity(names.len());
    let mut missing = Vec::new();
    for name in names {
        let path = model_dir.join(&name);
        if path.is_file() {
            paths.push(path);
        } else {
            missing.push(name);
        }
    }
    if !missing.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "model directory is incomplete; missing safetensors shards: {}",
                missing.join(", ")
            ),
        )
        .into());
    }
    Ok(paths)
}

fn required_flag(args: &[String], flag: &str) -> Result<String, io::Error> {
    optional_flag(args, flag).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: qwen36_legal_prompt_smoke --model Qwen3.6-27B --prompt fixtures/legal/smoke.prompt [--config path] [--tokenizer path] [--shard path] [--model-dir path]",
        )
    })
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
