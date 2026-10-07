use std::path::PathBuf;

use psionic_train::{
    run_qwen_legal_distributed_run_milestone, QwenLegalDistributedRunMilestoneConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = QwenLegalDistributedRunMilestoneConfig::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--suite" => {
                let Some(value) = args.next() else {
                    return Err("--suite requires a path".into());
                };
                config.suite_path = PathBuf::from(value);
            }
            "--out" => {
                let Some(value) = args.next() else {
                    return Err("--out requires a path".into());
                };
                config.output_dir = PathBuf::from(value);
            }
            "--report" => {
                let Some(value) = args.next() else {
                    return Err("--report requires a path".into());
                };
                config.report_path = PathBuf::from(value);
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }
    let report = run_qwen_legal_distributed_run_milestone(&config)?;
    println!(
        "distributed_run={} workers={} champion_score_bps={} candidate_score_bps={} delta_bps={} promotion={:?} report={} digest={}",
        report.run_id,
        report.worker_count,
        report.champion_score_bps,
        report.candidate_score_bps,
        report.score_delta_bps,
        report.promotion_decision,
        report.report_path,
        report.report_digest
    );
    Ok(())
}
