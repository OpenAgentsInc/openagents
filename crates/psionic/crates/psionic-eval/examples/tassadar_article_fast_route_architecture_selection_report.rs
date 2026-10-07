use psionic_eval::{
    tassadar_article_fast_route_architecture_selection_report_path,
    write_tassadar_article_fast_route_architecture_selection_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_fast_route_architecture_selection_report_path();
    let report = write_tassadar_article_fast_route_architecture_selection_report(&path)?;
    println!(
        "wrote {} with selected_candidate_kind={} and fast_route_selection_green={}",
        path.display(),
        report.selected_candidate_kind.label(),
        report.fast_route_selection_green,
    );
    Ok(())
}
