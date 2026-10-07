use std::{error::Error, path::PathBuf};

use psionic_train::{
    CS336_A1_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH,
    write_cs336_a1_full_port_conformance_fixture,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let report = write_cs336_a1_full_port_conformance_fixture(&root)?;
    println!(
        "wrote {} fully_green={} rows={}",
        root.join(CS336_A1_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH)
            .display(),
        report.fully_green,
        report.row_count,
    );
    Ok(())
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve repo root".into())
}
