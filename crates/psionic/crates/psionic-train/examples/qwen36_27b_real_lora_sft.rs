use std::{env, error::Error, path::PathBuf};

use psionic_train::{
    Qwen36RealLoraSftConfig, run_qwen36_real_lora_sft, run_qwen36_real_lora_sft_config_path,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let artifacts = if let Some(config_path) = optional_flag(&args, "--config") {
        run_qwen36_real_lora_sft_config_path(PathBuf::from(config_path))?
    } else {
        let mut config = Qwen36RealLoraSftConfig::default();
        if let Some(model_dir) = optional_flag(&args, "--model-dir") {
            config.model_dir = model_dir;
        }
        if let Some(output_dir) = optional_flag(&args, "--output-dir") {
            config.output_dir = output_dir;
        }
        run_qwen36_real_lora_sft(&config)?
    };
    println!("{}", serde_json::to_string_pretty(&artifacts.receipt)?);
    Ok(())
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
