use psionic_research::{
    tassadar_turing_completeness_closeout_summary_path,
    write_tassadar_turing_completeness_closeout_summary,
};

fn main() {
    let summary = write_tassadar_turing_completeness_closeout_summary(
        tassadar_turing_completeness_closeout_summary_path(),
    )
    .expect("write Turing-completeness closeout summary");
    println!(
        "wrote {} with claim_status={:?}",
        summary.report_id, summary.eval_report.claim_status
    );
}
