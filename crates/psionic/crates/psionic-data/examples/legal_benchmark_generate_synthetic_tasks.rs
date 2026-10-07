use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_data::{
    generate_legal_synthetic_workflow_tasks, LegalSyntheticWorkflowTaskGeneratorConfig,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut count = None;
    let mut out = None;
    let mut suite_id = String::from("synthetic_legal_workflow_v1");
    let mut seed = 20260520_u64;
    let mut base_model = String::from("synthetic-base-workflow-policy-v1");
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--count" => {
                index += 1;
                count = args
                    .get(index)
                    .map(|value| value.parse::<usize>())
                    .transpose()?;
            }
            "--out" => {
                index += 1;
                out = args.get(index).map(PathBuf::from);
            }
            "--suite-id" => {
                index += 1;
                suite_id = args.get(index).cloned().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--suite-id requires a value")
                })?;
            }
            "--seed" => {
                index += 1;
                seed = args
                    .get(index)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--seed requires a value")
                    })?
                    .parse()?;
            }
            "--base-model" => {
                index += 1;
                base_model = args.get(index).cloned().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--base-model requires a value")
                })?;
            }
            other => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument `{other}`"),
                )));
            }
        }
        index += 1;
    }

    let usage = "usage: legal_benchmark_generate_synthetic_tasks --count <n> --out <dir> [--suite-id <id>] [--seed <u64>] [--base-model <id>]";
    let count = count.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;
    let out_dir = out.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;

    let result =
        generate_legal_synthetic_workflow_tasks(&LegalSyntheticWorkflowTaskGeneratorConfig {
            count,
            out_dir,
            suite_id,
            seed,
            base_model,
        })?;
    println!(
        "wrote synthetic legal workflow tasks: manifest={} tasks={} success_runs={} failed_runs={} sft_examples={} dpo_pairs={} manifest_hash={}",
        result.manifest_path.display(),
        result.manifest.task_count,
        result.manifest.successful_base_run_count,
        result.manifest.failed_base_run_count,
        result.manifest.sft_example_count,
        result.manifest.dpo_pair_count,
        result.manifest.manifest_hash
    );
    Ok(())
}
