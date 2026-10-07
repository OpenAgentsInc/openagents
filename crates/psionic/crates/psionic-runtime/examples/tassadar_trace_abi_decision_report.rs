use psionic_runtime::{
    tassadar_long_horizon_trace_fixture_root_path, tassadar_trace_abi_decision_report_path,
    write_tassadar_trace_abi_decision_artifacts,
};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let report_path = args.first().map_or_else(
        tassadar_trace_abi_decision_report_path,
        std::path::PathBuf::from,
    );
    let fixture_root = args.get(1).map_or_else(
        tassadar_long_horizon_trace_fixture_root_path,
        std::path::PathBuf::from,
    );

    let report = write_tassadar_trace_abi_decision_artifacts(&report_path, &fixture_root)
        .expect("trace-ABI decision artifacts should write");
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("trace-ABI decision report should serialize")
    );
}
