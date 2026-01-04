use std::env;

use ml::{GptOssEngine, GptOssEngineConfig, GptOssTokenEvent, MlError, Result};

fn main() -> Result<()> {
    let mut gguf_path: Option<String> = None;
    let mut prompt = "Hello from GPT-OSS.".to_string();
    let mut max_tokens: usize = 20;
    let mut layer_limit: Option<usize> = None;
    let mut moe_fallback = false;
    let mut use_harmony_prompt = true;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gguf" => gguf_path = args.next(),
            "--prompt" => prompt = args.next().unwrap_or_default(),
            "--max-tokens" => {
                max_tokens = args
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(20);
            }
            "--layers" => {
                layer_limit = args.next().and_then(|v| v.parse::<usize>().ok());
            }
            "--moe-fallback" => moe_fallback = true,
            "--no-harmony" => use_harmony_prompt = false,
            _ => {}
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig(
            "usage: gptoss_cli --gguf <path> --prompt \"hi\" --max-tokens 20".to_string(),
        )
    })?;

    let mut engine = GptOssEngine::load(&gguf_path)?;
    let mut config = GptOssEngineConfig::default();
    config.generation.max_new_tokens = max_tokens;
    config.generation.temperature = 0.0;
    config.generation.top_k = 1;
    config.generation.top_p = 1.0;
    config.layer_limit = layer_limit;
    config.moe_fallback = moe_fallback;
    config.use_harmony_prompt = use_harmony_prompt;

    let mut step = 0usize;
    let mut on_token = |event: &GptOssTokenEvent| {
        let top1 = event
            .top_k
            .first()
            .map(|candidate| candidate.token_text.clone())
            .unwrap_or_else(|| event.token_text.clone());
        println!(
            "step={} token={} entropy={:.3} top1={}",
            step,
            event.token_id,
            event.entropy,
            top1.replace('\n', "\\n")
        );
        step += 1;
        Ok(())
    };

    let completion =
        engine.generate_with_callback(&prompt, &config, Some(&mut on_token), None)?;

    println!("output:\n{}", completion.text);
    Ok(())
}
