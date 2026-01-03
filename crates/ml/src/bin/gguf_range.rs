use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use ml::{load_gguf_index, MlError, Result};
use sha2::{Digest, Sha256};

#[derive(Debug)]
struct RangeSpec {
    offset: u64,
    len: u64,
}

#[derive(Debug, serde::Serialize)]
struct RangeHash {
    path: String,
    offset: u64,
    len: u64,
    sha256: String,
    repeat: usize,
    consistent: bool,
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Err(MlError::InvalidConfig(
            "usage: gguf_range <path> (--offset N --len N | --tensor NAME | --tensor-index N) [--offset N] [--len N] [--repeat N] [--json]"
                .to_string(),
        ));
    }

    let mut path: Option<PathBuf> = None;
    let mut offset: Option<u64> = None;
    let mut len: Option<u64> = None;
    let mut tensor_name: Option<String> = None;
    let mut tensor_index: Option<usize> = None;
    let mut repeat: usize = 1;
    let mut json = false;

    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--offset" => {
                idx += 1;
                let value = args.get(idx).ok_or_else(|| {
                    MlError::InvalidConfig("missing value for --offset".to_string())
                })?;
                offset = Some(parse_u64(value, "--offset")?);
            }
            "--len" => {
                idx += 1;
                let value = args.get(idx).ok_or_else(|| {
                    MlError::InvalidConfig("missing value for --len".to_string())
                })?;
                len = Some(parse_u64(value, "--len")?);
            }
            "--tensor" => {
                idx += 1;
                let value = args.get(idx).ok_or_else(|| {
                    MlError::InvalidConfig("missing value for --tensor".to_string())
                })?;
                tensor_name = Some(value.to_string());
            }
            "--tensor-index" => {
                idx += 1;
                let value = args.get(idx).ok_or_else(|| {
                    MlError::InvalidConfig("missing value for --tensor-index".to_string())
                })?;
                tensor_index = Some(parse_usize(value, "--tensor-index")?);
            }
            "--repeat" => {
                idx += 1;
                let value = args.get(idx).ok_or_else(|| {
                    MlError::InvalidConfig("missing value for --repeat".to_string())
                })?;
                repeat = parse_usize(value, "--repeat")?;
                if repeat == 0 {
                    repeat = 1;
                }
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
        MlError::InvalidConfig("missing gguf path".to_string())
    })?;

    if tensor_name.is_some() && tensor_index.is_some() {
        return Err(MlError::InvalidConfig(
            "use either --tensor or --tensor-index, not both".to_string(),
        ));
    }

    let range = if let Some(name) = tensor_name {
        let index = load_gguf_index(&path)?;
        let tensor = index
            .tensors
            .iter()
            .find(|tensor| tensor.name == name)
            .ok_or_else(|| MlError::InvalidConfig(format!("tensor not found: {name}")))?;
        resolve_tensor_range(tensor.absolute_offset, tensor.nbytes, offset, len)?
    } else if let Some(tidx) = tensor_index {
        let index = load_gguf_index(&path)?;
        let tensor = index.tensors.get(tidx).ok_or_else(|| {
            MlError::InvalidConfig(format!("tensor index out of range: {tidx}"))
        })?;
        resolve_tensor_range(tensor.absolute_offset, tensor.nbytes, offset, len)?
    } else {
        let offset = offset.ok_or_else(|| {
            MlError::InvalidConfig("missing --offset".to_string())
        })?;
        let len = len.ok_or_else(|| MlError::InvalidConfig("missing --len".to_string()))?;
        RangeSpec { offset, len }
    };

    let mut file = File::open(&path)?;
    let file_size = file.metadata()?.len();
    if range.offset.checked_add(range.len).unwrap_or(u64::MAX) > file_size {
        return Err(MlError::InvalidConfig(format!(
            "range exceeds file size: offset={} len={} size={}",
            range.offset, range.len, file_size
        )));
    }

    let mut hashes = Vec::with_capacity(repeat);
    for _ in 0..repeat {
        hashes.push(hash_range(&mut file, range.offset, range.len)?);
    }

    let first = hashes
        .first()
        .cloned()
        .unwrap_or_else(|| String::new());
    let consistent = hashes.iter().all(|h| h == &first);

    if json {
        let payload = RangeHash {
            path: path.display().to_string(),
            offset: range.offset,
            len: range.len,
            sha256: first,
            repeat,
            consistent,
        };
        let json = serde_json::to_string_pretty(&payload)?;
        println!("{json}");
        return Ok(());
    }

    println!("path: {}", path.display());
    println!("offset: {}", range.offset);
    println!("len: {}", range.len);
    println!("sha256: {}", first);
    println!("repeat: {}", repeat);
    println!("consistent: {}", consistent);

    Ok(())
}

fn resolve_tensor_range(
    base_offset: u64,
    base_len: u64,
    rel_offset: Option<u64>,
    limit_len: Option<u64>,
) -> Result<RangeSpec> {
    let mut offset = base_offset;
    let mut len = base_len;
    if let Some(rel) = rel_offset {
        if rel >= base_len {
            return Err(MlError::InvalidConfig(format!(
                "tensor offset {rel} exceeds tensor length {base_len}"
            )));
        }
        offset = offset.saturating_add(rel);
        len = base_len - rel;
    }
    if let Some(limit) = limit_len {
        len = len.min(limit);
    }
    if len == 0 {
        return Err(MlError::InvalidConfig(
            "range length resolved to zero".to_string(),
        ));
    }
    Ok(RangeSpec { offset, len })
}

fn hash_range(file: &mut File, offset: u64, len: u64) -> Result<String> {
    file.seek(SeekFrom::Start(offset))?;
    let mut remaining = len;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    while remaining > 0 {
        let read_len = buffer.len().min(remaining as usize);
        let chunk = &mut buffer[..read_len];
        file.read_exact(chunk)?;
        hasher.update(chunk);
        remaining -= read_len as u64;
    }
    Ok(hex::encode(hasher.finalize()))
}

fn parse_u64(value: &str, flag: &str) -> Result<u64> {
    value.parse().map_err(|_| {
        MlError::InvalidConfig(format!("invalid value for {flag}: {value}"))
    })
}

fn parse_usize(value: &str, flag: &str) -> Result<usize> {
    value.parse().map_err(|_| {
        MlError::InvalidConfig(format!("invalid value for {flag}: {value}"))
    })
}
