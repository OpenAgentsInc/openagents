use std::{env, fs, path::PathBuf};

use psionic_train::build_parameter_golf_record_folder_replay_verification_report;

fn main() {
    let submission_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("resolve current directory"));
    let output_path = env::args().nth(2).map(PathBuf::from);
    let report = build_parameter_golf_record_folder_replay_verification_report(&submission_dir)
        .expect("build Parameter Golf record-folder replay verification report");
    let json = serde_json::to_string_pretty(&report)
        .expect("serialize Parameter Golf replay verification report");
    if let Some(path) = output_path {
        fs::write(&path, format!("{json}\n")).expect("write replay verification report");
        println!("wrote {}", path.display());
    } else {
        println!("{json}");
    }
}
