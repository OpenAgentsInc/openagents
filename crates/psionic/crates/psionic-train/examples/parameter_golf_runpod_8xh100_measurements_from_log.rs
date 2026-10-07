use std::{env, fs, path::PathBuf};

use psionic_train::build_parameter_golf_runpod_8xh100_measurements_from_train_log;

fn main() {
    let mut run_id = None;
    let mut mesh_id = None;
    let mut memory_source = None;

    let mut args = env::args().skip(1);
    let log_path = PathBuf::from(args.next().expect("missing execution log path"));
    let output_path = PathBuf::from(args.next().expect("missing output json path"));
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--run-id" => run_id = Some(args.next().expect("missing value for --run-id")),
            "--mesh-id" => mesh_id = Some(args.next().expect("missing value for --mesh-id")),
            "--memory-source" => {
                memory_source = Some(args.next().expect("missing value for --memory-source"))
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: parameter_golf_runpod_8xh100_measurements_from_log \\
  <execution_log_path> <output_json_path> [--run-id <id>] [--mesh-id <id>] [--memory-source <text>]"
                );
                std::process::exit(0);
            }
            value => panic!("unexpected argument `{value}`"),
        }
    }

    let log_text = fs::read_to_string(&log_path).expect("read execution log");
    let measurements = build_parameter_golf_runpod_8xh100_measurements_from_train_log(
        &log_text,
        run_id.as_deref(),
        mesh_id.as_deref(),
        memory_source.as_deref(),
    )
    .expect("build RunPod 8xH100 measurements from log");

    fs::write(
        &output_path,
        serde_json::to_vec_pretty(&measurements).expect("encode measurements"),
    )
    .expect("write measurements json");
    println!(
        "wrote {} ({})",
        output_path.display(),
        measurements.schema_version
    );
}
