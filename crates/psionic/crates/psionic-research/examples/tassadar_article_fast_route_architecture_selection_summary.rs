use psionic_research::{
    tassadar_article_fast_route_architecture_selection_summary_path,
    write_tassadar_article_fast_route_architecture_selection_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_fast_route_architecture_selection_summary_path();
    let summary = write_tassadar_article_fast_route_architecture_selection_summary(&path)?;
    println!(
        "wrote {} with selected_candidate_kind={} and fast_route_selection_green={}",
        path.display(),
        summary.selected_candidate_kind,
        summary.fast_route_selection_green,
    );
    Ok(())
}
