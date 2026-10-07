use psionic_research::{
    tassadar_hungarian_10x10_article_reproducer_report_path,
    write_tassadar_hungarian_10x10_article_reproducer_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_hungarian_10x10_article_reproducer_report(
        tassadar_hungarian_10x10_article_reproducer_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
