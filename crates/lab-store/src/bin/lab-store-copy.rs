//! Checked copies from an explicit CAD export. Dry-run is the default.
use lab_store::{
    backend::FsBackend,
    cad_legacy::{CadLegacy, DirectorySource},
};
use std::sync::Arc;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = None;
    let mut owner = None;
    let mut project = None;
    let mut target = None;
    let mut checksum = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "--source" => &mut source,
            "--owner" => &mut owner,
            "--project" => &mut project,
            "--target" => &mut target,
            "--apply" => &mut checksum,
            "--help" | "-h" => {
                println!(
                    "lab-store-copy --source ABSOLUTE_FOLDER --owner ACCOUNT [--project PROJECT] [--target ABSOLUTE_FOLDER --apply DRY_RUN_CHECKSUM]"
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown option: {arg}").into()),
        };
        if slot.is_some() {
            return Err(format!("Choose {arg} once.").into());
        }
        *slot = Some(
            args.next()
                .ok_or_else(|| format!("Enter a value for {arg}."))?,
        );
    }
    let source = Arc::new(DirectorySource::open(source.ok_or("Choose --source.")?)?);
    let owner = owner.ok_or("Choose --owner.")?;
    let legacy = match project {
        Some(project) => CadLegacy::project(source, &owner, &project)?,
        None => CadLegacy::personal(source, &owner)?,
    };
    let report = if let Some(checksum) = checksum {
        let target = FsBackend::open(target.ok_or("Choose --target when applying a copy.")?)?;
        legacy.copy(&target, &checksum).await?
    } else {
        if target.is_some() {
            return Err("Use --apply with a reviewed dry-run checksum.".into());
        }
        legacy.dry_run().await?
    };
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"checksum":report.checksum()?,"report":report})
        )?
    );
    Ok(())
}
