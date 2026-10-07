use std::{env, path::PathBuf};

use psionic_train::{
    ParameterGolfSingleH100BringupConfig, write_parameter_golf_single_h100_bringup_report,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_dataset_root);
    let tokenizer_path = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(default_tokenizer_path);
    let output_path = env::args()
        .nth(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/parameter_golf_single_h100_bringup.json"));
    let config =
        ParameterGolfSingleH100BringupConfig::challenge_defaults(dataset_root, tokenizer_path);
    let report = write_parameter_golf_single_h100_bringup_report(&output_path, &config)?;
    println!(
        "wrote {} with disposition {:?}",
        output_path.display(),
        report.disposition
    );
    if let Some(refusal) = report.refusal {
        println!(
            "refusal subject={:?} detail={}",
            refusal.subject, refusal.detail
        );
    }
    Ok(())
}

fn default_dataset_root() -> PathBuf {
    PathBuf::from(
        env::var("HOME").expect("HOME should exist for the default parameter-golf dataset root"),
    )
    .join("code/parameter-golf/data/datasets/fineweb10B_sp1024")
}

fn default_tokenizer_path() -> PathBuf {
    PathBuf::from(
        env::var("HOME").expect("HOME should exist for the default parameter-golf tokenizer path"),
    )
    .join("code/parameter-golf/data/tokenizers/fineweb_1024_bpe.model")
}
