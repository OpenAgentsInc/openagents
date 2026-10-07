use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;

use psionic_eval::{
    LegalBenchmarkReportInput, ScoreReport, generate_legal_benchmark_static_report,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let score_path = args.get(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: legal_benchmark_report <score_report.json> <output_dir>",
        )
    })?;
    let output_dir = args.get(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: legal_benchmark_report <score_report.json> <output_dir>",
        )
    })?;
    let output_dir = PathBuf::from(output_dir);
    fs::create_dir_all(&output_dir)?;
    let score_report =
        serde_json::from_slice::<ScoreReport>(&fs::read(PathBuf::from(score_path))?)?;
    let report = generate_legal_benchmark_static_report(&LegalBenchmarkReportInput {
        report_id: String::from("legal_benchmark_report"),
        score_reports: vec![score_report],
        run_records: Vec::new(),
        output_manifests: Vec::new(),
    })?;
    fs::write(output_dir.join("report.md"), report.markdown)?;
    fs::write(
        output_dir.join("autopilot_report.json"),
        serde_json::to_vec_pretty(&report.autopilot_export)?,
    )?;
    fs::write(
        output_dir.join("failure_clusters.json"),
        serde_json::to_vec_pretty(&report.autopilot_export.failure_clusters)?,
    )?;
    Ok(())
}
