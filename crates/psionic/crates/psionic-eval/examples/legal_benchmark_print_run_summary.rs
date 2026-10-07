use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;

use psionic_eval::{LegalRunReceipt, legal_run_receipt_digest};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let path = args.get(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: legal_benchmark_print_run_summary <run_receipt.json>",
        )
    })?;
    let receipt = serde_json::from_slice::<LegalRunReceipt>(&fs::read(PathBuf::from(path))?)?;
    receipt.validate()?;

    println!("receipt: {}", receipt.receipt_id);
    println!("digest: {}", legal_run_receipt_digest(&receipt)?);
    println!("benchmark: {}", receipt.run_spec.benchmark_id);
    println!("visibility: {:?}", receipt.run_spec.benchmark_visibility);
    println!("task: {}", receipt.run_spec.task_id);
    println!("run: {}", receipt.run_spec.run_id);
    println!("base model: {}", receipt.run_spec.base_model_id);
    println!(
        "adapter: {}",
        receipt.run_spec.adapter_id.as_deref().unwrap_or("none")
    );
    println!("score bps: {}", receipt.score.criterion_pass_rate_bps);
    println!("integrity valid: {}", receipt.integrity.valid);
    println!("answer files:");
    for answer in &receipt.answer_files {
        println!(
            "- {} hash={} actor={:?}",
            answer.relative_path, answer.content_hash.value, answer.last_modifying_actor
        );
    }
    Ok(())
}
