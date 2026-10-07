use psionic_train::{
    Qwen36RealPylonRehearsalConfig, run_qwen36_real_pylon_rehearsal,
    run_qwen36_real_pylon_rehearsal_config_path,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Qwen36RealPylonRehearsalConfig::default();
    let mut config_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                let Some(value) = args.next() else {
                    return Err("--config requires a path".into());
                };
                config_path = Some(value);
            }
            "--model-dir" => {
                let Some(value) = args.next() else {
                    return Err("--model-dir requires a path".into());
                };
                config.model_dir = value;
            }
            "--out" => {
                let Some(value) = args.next() else {
                    return Err("--out requires a path".into());
                };
                config.output_dir = value;
            }
            "--report" => {
                let Some(value) = args.next() else {
                    return Err("--report requires a path".into());
                };
                config.report_path = value;
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }
    let report = if let Some(path) = config_path {
        run_qwen36_real_pylon_rehearsal_config_path(path)?
    } else {
        run_qwen36_real_pylon_rehearsal(&config)?
    };
    println!(
        "run={} workers={}/{} merged_adapter={} candidate_score_bps={} delta_bps={} payment_gate={:?} report={} digest={}",
        report.run_id,
        report.accepted_worker_count,
        report.worker_count,
        report.merged_adapter_sha256,
        report.candidate_score_bps,
        report.score_delta_bps,
        report.payment_closeout.promotion_payment_gate_status,
        report.report_path,
        report.report_digest
    );
    Ok(())
}
