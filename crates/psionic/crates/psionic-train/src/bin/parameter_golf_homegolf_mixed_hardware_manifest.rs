use std::path::PathBuf;

use psionic_train::{
    write_parameter_golf_homegolf_mixed_hardware_manifest_report,
    PARAMETER_GOLF_HOMEGOLF_MIXED_HARDWARE_MANIFEST_FIXTURE_PATH,
};

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(PARAMETER_GOLF_HOMEGOLF_MIXED_HARDWARE_MANIFEST_FIXTURE_PATH));
    let report = write_parameter_golf_homegolf_mixed_hardware_manifest_report(output.as_path())
        .expect("write HOMEGOLF mixed hardware manifest report");
    println!(
        "wrote {} optional_h100_admitted={} manifest_entries={} comparison_policy_preserved={}",
        output.display(),
        report.optional_h100_admitted,
        report.manifest_entries.len(),
        report.comparison_policy_preserved
    );
}
