use std::env;
use std::io::{self, Write};

use autopilot::preflight::PreflightConfig;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cwd = env::current_dir()?;

    println!("=== Autopilot Preflight Demo ===\n");

    println!("Step 1: Running preflight checks...");
    let config = PreflightConfig::run(&cwd)?;

    println!("\n--- Preflight Results ---");
    println!("{}", config.to_system_prompt());

    let gpt_oss_available = config
        .inference
        .local_backends
        .iter()
        .any(|b| b.name == "gpt-oss" && b.available);

    if !gpt_oss_available {
        println!("\n[ERROR] GPT-OSS not available on localhost:8000");
        println!("Start llama-server with:");
        println!("  ~/code/llama.cpp/build/bin/llama-server \\");
        println!("    -m ~/models/gpt-oss/gpt-oss-120b-mxfp4.gguf \\");
        println!("    -c 0 -fa on --jinja --reasoning-format none \\");
        println!("    --host 0.0.0.0 --port 8000 -ngl 0");
        return Ok(());
    }

    println!("\n[OK] GPT-OSS detected on localhost:8000");

    println!("\nStep 2: Saving config...");
    let config_path = config.save()?;
    println!("Saved to: {}", config_path.display());

    println!("\nStep 3: Running demo inference (streaming)...\n");
    println!("--- GPT-OSS Response ---");

    let prompt = format!(
        "You are an AI assistant analyzing a developer environment. \
         Based on this configuration, give a brief (2-3 sentence) assessment:\n\n{}",
        config.to_system_prompt()
    );

    stream_gpt_oss_response(&prompt).await?;

    println!("\n\n--- Demo Complete ---");

    Ok(())
}

async fn stream_gpt_oss_response(prompt: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": "gpt-oss-120b",
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "max_tokens": 256,
        "stream": true
    });

    let response = client
        .post("http://localhost:8000/v1/chat/completions")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("GPT-OSS error: {}", error_text);
    }

    let mut stream = response.bytes_stream();
    let mut stdout = io::stdout();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" {
                    break;
                }

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(content) = json
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        print!("{}", content);
                        stdout.flush()?;
                    }
                }
            }
        }
    }

    Ok(())
}
