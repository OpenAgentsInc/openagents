use std::{env, path::PathBuf};

use psionic_train::{
    write_first_swarm_trusted_lan_rehearsal_report,
    FIRST_SWARM_TRUSTED_LAN_REHEARSAL_REPORT_FIXTURE_PATH,
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
        .unwrap_or_else(|| PathBuf::from(FIRST_SWARM_TRUSTED_LAN_REHEARSAL_REPORT_FIXTURE_PATH));
    let report = write_first_swarm_trusted_lan_rehearsal_report(&output_path)?;
    println!(
        "wrote {} with report_digest {}",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
