use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_eval::inspect_failed_trajectory_run_dir;

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let run_dir = args.get(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: legal_benchmark_inspect_failures <run-dir>",
        )
    })?;
    let inspection = inspect_failed_trajectory_run_dir(PathBuf::from(run_dir))?;
    let bad_run = inspection.bad_run;
    println!("bad run: {}", bad_run.example_id);
    println!("failure class: {:?}", bad_run.failure_class);
    println!("training eligible: {}", bad_run.training_eligible);
    println!("sft eligible: {}", bad_run.sft_eligible);
    println!("required files:");
    for file in &bad_run.required_files {
        println!(
            "- {} existed={} bytes={}",
            file.relative_path,
            file.existed,
            file.byte_len
                .map(|value| value.to_string())
                .unwrap_or_else(|| String::from("none"))
        );
    }
    println!("attempted writes:");
    for write in &bad_run.attempted_file_writes {
        println!("- {} via {}", write.relative_path, write.tool_call_id);
    }
    if let Some(raw) = &bad_run.raw_malformed_text {
        println!("raw malformed text bytes: {}", raw.len());
    }
    Ok(())
}
