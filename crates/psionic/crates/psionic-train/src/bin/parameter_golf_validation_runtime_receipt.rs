use std::{env, path::PathBuf};

use psionic_train::{
    write_parameter_golf_single_h100_validation_runtime_comparison_receipt,
    ParameterGolfSingleH100ValidationRuntimeComparisonConfig,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().collect::<Vec<_>>();
    let dataset_root = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_dataset_root);
    let tokenizer_path = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(default_tokenizer_path);
    let output_path = args
        .get(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/parameter_golf_validation_runtime_comparison.json"));
    let batch_sequences = args
        .get(4)
        .map(String::as_str)
        .map(|value| value.parse::<usize>())
        .transpose()?;
    let batch_limit = args
        .get(5)
        .map(String::as_str)
        .map(|value| value.parse::<usize>())
        .transpose()?;

    let mut config =
        ParameterGolfSingleH100ValidationRuntimeComparisonConfig::bounded_local_defaults(
            dataset_root,
            tokenizer_path,
        );
    if let Some(batch_sequences) = batch_sequences {
        config.batch_sequences = batch_sequences;
    }
    if let Some(batch_limit) = batch_limit {
        config.batch_limit = batch_limit;
    }

    let receipt = write_parameter_golf_single_h100_validation_runtime_comparison_receipt(
        &output_path,
        &config,
    )?;
    println!(
        "wrote {} with legacy_batch_ms={:.2} device_resident_batch_ms={:.2}",
        output_path.display(),
        receipt.legacy.average_batch_ms,
        receipt.device_resident.average_batch_ms,
    );
    println!("{}", receipt.summary);
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
