use psionic_research::{
    tassadar_article_fast_route_exactness_summary_path,
    write_tassadar_article_fast_route_exactness_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_fast_route_exactness_summary_path();
    let summary = write_tassadar_article_fast_route_exactness_summary(&path)?;
    println!(
        "wrote {} ({})",
        path.display(),
        serde_json::to_string_pretty(&summary)?
    );
    Ok(())
}
