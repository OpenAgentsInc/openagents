use std::{env, fs, path::PathBuf};

use psionic_models::ParameterGolfPromotedRuntimeBundle;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
struct GoldenPromptCase {
    prompt_id: String,
    prompt: String,
    max_new_tokens: usize,
    mode: String,
    #[serde(default)]
    seed: Option<u64>,
    expected_text: String,
    detail: String,
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        eprintln!(
            "usage: cargo run -p psionic-train --example parameter_golf_promoted_prompt -- <bundle_dir> [--prompt <text>] [--max-new-tokens <n>] [--mode greedy|sample] [--seed <u64>]"
        );
        std::process::exit(1);
    }

    let bundle_dir = PathBuf::from(&args[0]);
    let mut prompt = None;
    let mut max_new_tokens = None;
    let mut mode = String::from("greedy");
    let mut seed = None;

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--prompt" => {
                index += 1;
                let value = args.get(index).expect("expected prompt after --prompt");
                prompt = Some(value.clone());
            }
            "--max-new-tokens" => {
                index += 1;
                let value = args
                    .get(index)
                    .expect("expected integer after --max-new-tokens");
                max_new_tokens = Some(value.parse().expect("max-new-tokens must be an integer"));
            }
            "--mode" => {
                index += 1;
                mode = args.get(index).expect("expected mode after --mode").clone();
            }
            "--seed" => {
                index += 1;
                let value = args.get(index).expect("expected integer after --seed");
                seed = Some(value.parse().expect("seed must be an integer"));
            }
            other => panic!("unsupported argument `{other}`"),
        }
        index += 1;
    }

    let bundle =
        ParameterGolfPromotedRuntimeBundle::load_dir(bundle_dir.as_path()).expect("load bundle");

    if let Some(prompt) = prompt {
        let mut options = if mode == "sample" {
            bundle.default_seeded_sampling_options(seed.unwrap_or(42))
        } else {
            bundle.default_greedy_generation_options()
        };
        if let Some(max_new_tokens) = max_new_tokens {
            options.max_new_tokens = max_new_tokens;
        }
        let output = bundle
            .generate_text(prompt.as_str(), &options)
            .expect("generate text");
        println!("{}", output.text);
        eprintln!(
            "profile={} termination={:?} prompt_tokens={} output_tokens={}",
            bundle.manifest().profile_id,
            output.termination,
            output.prompt_tokens.len(),
            output.generated_tokens.len()
        );
        return;
    }

    let suite_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../fixtures/parameter_golf/inference/parameter_golf_promoted_golden_prompts.json",
    );
    let suite = serde_json::from_slice::<Vec<GoldenPromptCase>>(
        fs::read(suite_path.as_path())
            .expect("read prompt suite")
            .as_slice(),
    )
    .expect("parse prompt suite");

    for case in suite {
        let mut options = if case.mode == "sample" {
            bundle.default_seeded_sampling_options(case.seed.unwrap_or(42))
        } else {
            bundle.default_greedy_generation_options()
        };
        options.max_new_tokens = case.max_new_tokens;
        let output = bundle
            .generate_text(case.prompt.as_str(), &options)
            .expect("generate suite prompt");
        assert_eq!(
            output.text, case.expected_text,
            "golden prompt `{}` drifted from the checked-in promoted PGOLF suite",
            case.prompt_id
        );
        println!(
            "[{}] mode={} termination={:?} prompt=\"{}\" output=\"{}\" expected=\"{}\" note=\"{}\"",
            case.prompt_id,
            case.mode,
            output.termination,
            case.prompt,
            output.text,
            case.expected_text,
            case.detail
        );
    }
}
