use std::env;
use std::path::PathBuf;

use ml::{load_gguf_index, MlError};

#[cfg(feature = "candle")]
fn main() -> Result<(), MlError> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut path: Option<PathBuf> = None;
    let mut limit: Option<usize> = Some(50);
    let mut json = false;

    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--limit" => {
                idx += 1;
                let value = args.get(idx).ok_or_else(|| {
                    MlError::InvalidConfig("missing value for --limit".to_string())
                })?;
                let parsed: usize = value.parse().map_err(|_| {
                    MlError::InvalidConfig(format!("invalid --limit value: {value}"))
                })?;
                limit = if parsed == 0 { None } else { Some(parsed) };
            }
            "--all" => {
                limit = None;
            }
            "--json" => {
                json = true;
            }
            arg => {
                if path.is_some() {
                    return Err(MlError::InvalidConfig(format!(
                        "unexpected argument: {arg}"
                    )));
                }
                path = Some(PathBuf::from(arg));
            }
        }
        idx += 1;
    }

    let path = path.ok_or_else(|| {
        MlError::InvalidConfig(
            "usage: gguf_dump <path> [--limit N|--all] [--json]".to_string(),
        )
    })?;

    let index = load_gguf_index(&path)?;
    if json {
        let payload = serde_json::to_string_pretty(&index)?;
        println!("{payload}");
        return Ok(());
    }

    println!("gguf: {}", path.display());
    println!("version: {}", index.version);
    println!("tensor_data_offset: {}", index.tensor_data_offset);
    println!("tensor_count: {}", index.tensor_count);

    match limit {
        Some(max) => {
            for (idx, tensor) in index.tensors.iter().take(max).enumerate() {
                println!(
                    "{idx:5} {} [{}:{}] dims={:?} offset={} bytes={} abs={}",
                    tensor.name,
                    tensor.ggml_type_name,
                    tensor.ggml_type,
                    tensor.dims,
                    tensor.offset,
                    tensor.nbytes,
                    tensor.absolute_offset
                );
            }
        }
        None => {
            for (idx, tensor) in index.tensors.iter().enumerate() {
                println!(
                    "{idx:5} {} [{}:{}] dims={:?} offset={} bytes={} abs={}",
                    tensor.name,
                    tensor.ggml_type_name,
                    tensor.ggml_type,
                    tensor.dims,
                    tensor.offset,
                    tensor.nbytes,
                    tensor.absolute_offset
                );
            }
        }
    }

    Ok(())
}

#[cfg(not(feature = "candle"))]
fn main() {
    eprintln!("gguf_dump requires the candle feature");
    std::process::exit(1);
}
