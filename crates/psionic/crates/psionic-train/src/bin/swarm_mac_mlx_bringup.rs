use std::{env, path::PathBuf};

use psionic_train::{
    SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH, write_first_swarm_mac_mlx_bringup_report,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH));
    let report = write_first_swarm_mac_mlx_bringup_report(&output_path)?;
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
