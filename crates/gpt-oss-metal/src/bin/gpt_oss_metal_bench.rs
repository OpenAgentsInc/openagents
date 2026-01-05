use std::env;
use std::io::{self, Write};

use gpt_oss_metal::{GptOssMetalConfig, GptOssMetalEngine};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let mut prompt = None;
    let mut max_tokens = None;
    let mut temperature = None;
    let mut use_harmony = true;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--prompt" => {
                prompt = args.next();
            }
            "--max-tokens" => {
                let value = args
                    .next()
                    .ok_or("--max-tokens requires a value")?;
                max_tokens = Some(value.parse()?);
            }
            "--temperature" => {
                let value = args
                    .next()
                    .ok_or("--temperature requires a value")?;
                temperature = Some(value.parse()?);
            }
            "--no-harmony" => {
                use_harmony = false;
            }
            _ => {
                return Err(format!("unknown arg: {arg}").into());
            }
        }
    }

    let prompt = prompt.ok_or("--prompt is required")?;

    let config = GptOssMetalConfig::from_env()?;
    let engine = GptOssMetalEngine::new(config)?;
    let completion = engine.generate_with_callback(
        &prompt,
        max_tokens,
        temperature,
        None,
        use_harmony,
        |chunk| {
            print!("{chunk}");
            io::stdout().flush()?;
            Ok(())
        },
    )?;

    println!("\n\n[finish_reason={}]", completion.finish_reason);
    Ok(())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,gpt_oss_metal=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}
