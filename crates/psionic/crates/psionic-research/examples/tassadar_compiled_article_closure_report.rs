use psionic_research::{
    tassadar_compiled_article_closure_report_path, write_tassadar_compiled_article_closure_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_compiled_article_closure_report_path();
    let report = write_tassadar_compiled_article_closure_report(&report_path)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
