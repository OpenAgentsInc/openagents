use psionic_eval::{
    tassadar_program_family_frontier_report_path, write_tassadar_program_family_frontier_report,
};

fn main() {
    let path = tassadar_program_family_frontier_report_path();
    let report = write_tassadar_program_family_frontier_report(&path).expect("write report");
    println!(
        "wrote {} with {} architecture summaries and {} held-out ladder rows",
        path.display(),
        report.architecture_summaries.len(),
        report.held_out_family_ladder.len(),
    );
}
