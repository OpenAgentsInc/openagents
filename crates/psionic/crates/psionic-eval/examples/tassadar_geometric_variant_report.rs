use psionic_eval::{
    tassadar_geometric_variant_report_path, write_tassadar_geometric_variant_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_geometric_variant_report(tassadar_geometric_variant_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
