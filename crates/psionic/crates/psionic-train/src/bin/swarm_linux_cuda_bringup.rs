use std::{env, path::PathBuf};

use psionic_train::{
    SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH, SWARM_LINUX_4080_SOURCE_INVENTORY_REPORT_PATH,
    write_first_swarm_linux_cuda_bringup_report,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let inventory_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(SWARM_LINUX_4080_SOURCE_INVENTORY_REPORT_PATH));
    let output_path = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH));
    let report = write_first_swarm_linux_cuda_bringup_report(&inventory_path, &output_path)?;
    println!(
        "wrote {} with disposition {:?}",
        output_path.display(),
        report.disposition
    );
    if let Some(refusal) = report.refusal {
        println!(
            "refusal subject={:?} detail={}",
            refusal.subject, refusal.detail
        );
    }
    Ok(())
}
