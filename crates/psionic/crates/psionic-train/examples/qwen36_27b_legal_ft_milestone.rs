use std::path::PathBuf;

use psionic_train::{run_qwen36_27b_legal_ft_milestone, Qwen36LegalFtMilestoneConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Qwen36LegalFtMilestoneConfig::default();
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

    let report = run_qwen36_27b_legal_ft_milestone(&config)?;
    println!(
        "qwen36_27b_legal_ft={} promoted={} decision={:?} champion_score_bps={} promoted_score_bps={} delta_bps={} report={} digest={}",
        report.run_id,
        report.promotion_receipt.promoted_candidate_id,
        report.promotion_receipt.decision,
        report.promotion_receipt.champion_score_bps,
        report.promotion_receipt.promoted_score_bps,
        report.promotion_receipt.score_delta_bps,
        report.report_path,
        report.report_digest
    );
    Ok(())
}
