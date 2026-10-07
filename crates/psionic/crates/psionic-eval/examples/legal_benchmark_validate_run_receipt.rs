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
            "usage: legal_benchmark_validate_run_receipt <run_receipt.json>",
        )
    })?;
    let receipt = serde_json::from_slice::<LegalRunReceipt>(&fs::read(PathBuf::from(path))?)?;
    receipt.validate()?;
    println!(
        "valid legal run receipt: {} digest={}",
        receipt.receipt_id,
        legal_run_receipt_digest(&receipt)?
    );
    Ok(())
}
