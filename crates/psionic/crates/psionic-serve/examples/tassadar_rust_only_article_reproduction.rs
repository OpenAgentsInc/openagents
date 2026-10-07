use psionic_serve::{
    tassadar_rust_only_article_reproduction_report_path,
    write_tassadar_rust_only_article_reproduction_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_rust_only_article_reproduction_report_path();
    let report = write_tassadar_rust_only_article_reproduction_report(&report_path)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
