//! Generates the canonical Tassadar Percepta CPU-transform training receipt
//! fixture by running a real, deterministic, CPU-only training rehearsal and
//! replaying the selected checkpoint against CPU-reference truth.
//!
//! Run from the repo root:
//!
//! ```bash
//! cargo run -q -p psionic-train \
//!   --example tassadar_percepta_cpu_transform_training_receipt_fixtures
//! ```

use std::{
    error::Error,
    path::{Path, PathBuf},
};

use psionic_train::{
    write_builtin_cpu_transform_training_receipt,
    TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_FIXTURE_PATH,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let receipt = write_builtin_cpu_transform_training_receipt(root.as_path())?;
    receipt.validate()?;
    println!(
        "wrote {} (receipt_ref={}, receipt_digest={}, exact_trace_case_count={}/{}, green_gate_satisfied={})",
        TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_FIXTURE_PATH,
        receipt.receipt_ref,
        receipt.receipt_digest,
        receipt.replay_verdict.exact_trace_case_count,
        receipt.replay_verdict.case_count,
        receipt.green_gate_satisfied,
    );
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("failed to resolve workspace root")?;
    Ok(root.to_path_buf())
}
