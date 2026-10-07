use psionic_runtime::{
    tassadar_semantic_window_migration_report_path, write_tassadar_semantic_window_migration_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_semantic_window_migration_report(
        tassadar_semantic_window_migration_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
