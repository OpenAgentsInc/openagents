use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_train::{
    default_qwen_legal_adapter_registry_path, register_qwen_legal_adapter_manifest,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let manifest = args.get(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: qwen_legal_register_adapter <adapter-manifest> [--registry <registry.json>]",
        )
    })?;
    let registry_path = optional_flag(&args, "--registry")
        .map(PathBuf::from)
        .unwrap_or_else(default_qwen_legal_adapter_registry_path);
    let receipt = register_qwen_legal_adapter_manifest(manifest, registry_path)?;
    println!(
        "registered adapter={} entry_hash={} registry={}",
        receipt.adapter_id, receipt.entry_hash.value, receipt.registry_path
    );
    Ok(())
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
