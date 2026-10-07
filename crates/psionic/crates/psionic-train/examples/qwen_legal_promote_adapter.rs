use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_train::{default_qwen_legal_adapter_registry_path, promote_qwen_legal_adapter};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let candidate = required_flag(&args, "--candidate")?;
    let suite = required_flag(&args, "--suite")?;
    let registry_path = optional_flag(&args, "--registry")
        .map(PathBuf::from)
        .unwrap_or_else(default_qwen_legal_adapter_registry_path);
    let receipt = promote_qwen_legal_adapter(registry_path, candidate.as_str(), suite.as_str())?;
    println!(
        "promotion decision={:?} candidate={} suite={} delta_bps={} receipt_hash={}",
        receipt.decision,
        receipt.candidate_adapter_id,
        receipt.suite_id,
        receipt.score_delta_bps,
        receipt.receipt_hash.value
    );
    Ok(())
}

fn required_flag(args: &[String], flag: &str) -> Result<String, io::Error> {
    optional_flag(args, flag).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: qwen_legal_promote_adapter --candidate <id> --suite <suite-id> [--registry <registry.json>]",
        )
    })
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
