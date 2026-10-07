use std::{env, fs, path::PathBuf};

use psionic_models::{
    QWEN38_GGUF_PROMPT_TOKENIZER_FIXTURE_PATH, Qwen38GgufProfile, qualify_qwen38_gguf,
    run_qwen38_gguf_converter_parity,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut official_model_dir = PathBuf::from("target/models/qwen/Qwen3.8-27B");
    let mut gguf_dir = PathBuf::from("target/models/qwen/unsloth/Qwen3.8-27B-GGUF");
    let mut fixture_path = PathBuf::from(QWEN38_GGUF_PROMPT_TOKENIZER_FIXTURE_PATH);
    let mut output_dir = PathBuf::from("fixtures/qwen38/reports");
    let mut reuse_parity = None::<PathBuf>;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--official-model-dir" => {
                official_model_dir =
                    PathBuf::from(args.next().ok_or("--official-model-dir requires a path")?);
            }
            "--gguf-dir" => {
                gguf_dir = PathBuf::from(args.next().ok_or("--gguf-dir requires a path")?);
            }
            "--fixture" => {
                fixture_path = PathBuf::from(args.next().ok_or("--fixture requires a path")?);
            }
            "--output-dir" => {
                output_dir = PathBuf::from(args.next().ok_or("--output-dir requires a path")?);
            }
            "--reuse-parity" => {
                reuse_parity = Some(PathBuf::from(
                    args.next().ok_or("--reuse-parity requires a path")?,
                ));
            }
            "--help" | "-h" => {
                println!(
                    "usage: qwen38_gguf_qualification [--official-model-dir PATH] [--gguf-dir PATH] [--fixture PATH] [--output-dir PATH] [--reuse-parity PATH]"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument `{other}`").into()),
        }
    }

    fs::create_dir_all(&output_dir)?;
    let parity = if let Some(path) = reuse_parity {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        run_qwen38_gguf_converter_parity(&official_model_dir, &gguf_dir)?
    };
    write_report(
        output_dir.join("qwen38_gguf_converter_parity_v1.json"),
        &parity,
    )?;
    for (profile, output_name) in [
        (
            Qwen38GgufProfile::DynamicV3UdQ3KXl,
            "qwen38_gguf_dynamic_v3_qualification_v1.json",
        ),
        (
            Qwen38GgufProfile::Q3KM,
            "qwen38_gguf_q3_k_m_qualification_v1.json",
        ),
        (
            Qwen38GgufProfile::Q4KM,
            "qwen38_gguf_q4_k_m_qualification_v1.json",
        ),
    ] {
        let report = qualify_qwen38_gguf(
            gguf_dir.join(profile.filename()),
            &official_model_dir,
            &fixture_path,
            profile,
            parity.report_sha256.clone(),
        )?;
        write_report(output_dir.join(output_name), &report)?;
    }
    Ok(())
}

fn write_report(
    path: PathBuf,
    report: &impl serde::Serialize,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(&path, serde_json::to_vec_pretty(report)?)?;
    println!("{}", path.display());
    Ok(())
}
