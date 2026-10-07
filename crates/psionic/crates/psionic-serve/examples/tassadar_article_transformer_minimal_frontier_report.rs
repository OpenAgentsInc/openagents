use std::{fs, path::Path};

use psionic_serve::{
    write_tassadar_article_transformer_minimal_frontier_report,
    TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_REPORT_REF,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_REPORT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = write_tassadar_article_transformer_minimal_frontier_report(path)?;
    println!(
        "wrote {} ({})",
        TASSADAR_ARTICLE_TRANSFORMER_MINIMAL_FRONTIER_REPORT_REF, report.report_digest
    );
    Ok(())
}
