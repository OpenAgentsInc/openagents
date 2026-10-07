use psionic_eval::{
    tassadar_rust_article_profile_completeness_report_path,
    write_tassadar_rust_article_profile_completeness_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_rust_article_profile_completeness_report_path();
    let report = write_tassadar_rust_article_profile_completeness_report(&report_path)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
