use std::{env, error::Error, path::PathBuf};

use psionic_train::{
    QwenLegalPylonDispatchMode, canonical_qwen_legal_loopback_dispatch_request,
    dispatch_qwen_legal_pylon_jobs,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return Ok(());
    }
    let mode = parse_mode(optional_flag(&args, "--mode").as_deref())?;
    let mode_label = mode_label(mode);
    let out = optional_flag(&args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "target/legal/pylon_dispatch/qwen-legal-{mode_label}-dispatch"
            ))
        });

    let mut request = canonical_qwen_legal_loopback_dispatch_request(out)?;
    request.mode = mode;
    request.run_id = format!("run.qwen-legal.pylon.{mode_label}-dispatch.000001");
    for job in &mut request.jobs {
        job.parent_run_id = request.run_id.clone();
    }
    if mode == QwenLegalPylonDispatchMode::Tailnet {
        request.nodes[0].network_addr = String::from("tcp://100.64.0.11:7447");
        request.nodes[1].network_addr = String::from("tcp://100.64.0.12:7447");
    } else if mode == QwenLegalPylonDispatchMode::Production {
        request.nodes[0].network_addr =
            String::from("pylon+tls://pylon-legal-01.openagents.internal:7447");
        request.nodes[1].network_addr =
            String::from("pylon+tls://pylon-legal-02.openagents.internal:7447");
    }

    let report = dispatch_qwen_legal_pylon_jobs(&request)?;
    println!(
        "{}",
        serde_json::json!({
            "run_id": report.run_id,
            "mode": report.mode,
            "status": report.status,
            "assignments": report.assignments.len(),
            "worker_receipts": report.worker_receipts.len(),
            "payment_decisions": report.payment_decisions.len(),
            "artifact_hashes": report.artifact_hashes.len(),
            "duplicate_successful_shards": report.duplicate_successful_shards.len(),
            "dispatch_report": request.output_dir.join("dispatch_report.json"),
            "report_digest": report.report_digest,
        })
    );
    Ok(())
}

fn print_usage() {
    eprintln!(
        "usage: qwen_legal_pylon_loopback_dispatch [--mode local-only|loopback|tailnet|production] [--out <dir>]"
    );
}

fn parse_mode(value: Option<&str>) -> Result<QwenLegalPylonDispatchMode, Box<dyn Error>> {
    match value {
        None | Some("loopback") => Ok(QwenLegalPylonDispatchMode::Loopback),
        Some("local-only") => Ok(QwenLegalPylonDispatchMode::LocalOnly),
        Some("tailnet") => Ok(QwenLegalPylonDispatchMode::Tailnet),
        Some("production") => Ok(QwenLegalPylonDispatchMode::Production),
        Some(other) => Err(format!("unsupported --mode `{other}`").into()),
    }
}

fn mode_label(mode: QwenLegalPylonDispatchMode) -> &'static str {
    match mode {
        QwenLegalPylonDispatchMode::LocalOnly => "local-only",
        QwenLegalPylonDispatchMode::Loopback => "loopback",
        QwenLegalPylonDispatchMode::Tailnet => "tailnet",
        QwenLegalPylonDispatchMode::Production => "production",
    }
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
