use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_train::{run_qwen_legal_three_task_milestone, QwenLegalThreeTaskMilestoneConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let mut config = QwenLegalThreeTaskMilestoneConfig::default();
    if let Some(suite) = optional_flag(&args, "--suite") {
        config.suite_path = PathBuf::from(suite);
    }
    if let Some(out) = optional_flag(&args, "--out") {
        config.output_dir = PathBuf::from(out);
    }
    if let Some(report) = optional_flag(&args, "--report") {
        config.report_path = PathBuf::from(report);
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: qwen_legal_three_task_milestone [--suite <suite.json>] [--out <dir>] [--report <report.md>]",
        )
        .into());
    }
    let report = run_qwen_legal_three_task_milestone(&config)?;
    println!(
        "milestone={} champion_score_bps={} candidate_score_bps={} delta_bps={} promoted={} report={} digest={}",
        report.milestone_id,
        report.champion_score_bps,
        report.candidate_score_bps,
        report.score_delta_bps,
        report.candidate_promoted,
        report.report_path,
        report.report_digest
    );
    Ok(())
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
