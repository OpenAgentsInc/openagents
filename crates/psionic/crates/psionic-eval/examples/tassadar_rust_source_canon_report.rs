use psionic_eval::{
    tassadar_rust_source_canon_report_path, write_tassadar_rust_source_canon_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_rust_source_canon_report(tassadar_rust_source_canon_report_path())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("Rust source canon report should serialize"),
    );
    Ok(())
}
