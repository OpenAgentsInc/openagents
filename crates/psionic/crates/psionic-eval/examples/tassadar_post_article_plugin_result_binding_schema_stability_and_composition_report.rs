use psionic_eval::{
    tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report_path,
    write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report_path();
    let report =
        write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report(
            &output_path,
        )?;
    println!("wrote {} ({})", output_path.display(), report.report_digest);
    println!(
        "dependency_rows={} binding_rows={} evidence_boundary_rows={} composition_rows={} negative_rows={} validation_rows={}",
        report.dependency_rows.len(),
        report.binding_rows.len(),
        report.evidence_boundary_rows.len(),
        report.composition_rows.len(),
        report.negative_rows.len(),
        report.validation_rows.len(),
    );
    Ok(())
}
