use std::{env, path::PathBuf, process};

use psionic_train::write_parameter_golf_homegolf_track_contract_report;

fn main() {
    let output_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                "/tmp/parameter_golf_homegolf_track_contract.json",
            )
        });
    if let Err(error) = write_parameter_golf_homegolf_track_contract_report(output_path.as_path())
    {
        eprintln!("error: {error}");
        process::exit(1);
    }
    println!("wrote {}", output_path.display());
}
