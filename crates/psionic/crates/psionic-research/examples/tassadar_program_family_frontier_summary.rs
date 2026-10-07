use psionic_research::{
    tassadar_program_family_frontier_summary_path, write_tassadar_program_family_frontier_summary,
};

fn main() {
    let path = tassadar_program_family_frontier_summary_path();
    let summary = write_tassadar_program_family_frontier_summary(&path).expect("write summary");
    println!(
        "wrote {} with {} hybrid frontier families and {} held-out break families",
        path.display(),
        summary.hybrid_frontier_families.len(),
        summary.held_out_break_families.len(),
    );
}
