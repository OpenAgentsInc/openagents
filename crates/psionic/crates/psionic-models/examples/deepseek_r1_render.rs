//! Print a pinned GGUF conversation's prompt and token IDs for engine comparisons.

use std::{env, fs, path::Path};

use psionic_models::{GgufContent, GgufPromptTemplateRenderer, PromptMessage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let model = args
        .next()
        .ok_or("Pass the GGUF path and a messages JSON file.")?;
    let messages = args.next().ok_or("Pass a messages JSON file.")?;
    if args.next().is_some() {
        return Err("Pass exactly the GGUF path and a messages JSON file.".into());
    }
    let content = GgufContent::read_path(Path::new(&model))?;
    let renderer =
        GgufPromptTemplateRenderer::new(content.load_tokenizer()?, content.load_chat_templates()?);
    let messages: Vec<PromptMessage> = serde_json::from_slice(&fs::read(messages)?)?;
    let rendered = renderer.render(None, &messages, true)?;
    let tokens = renderer.tokenize_rendered_prompt(&rendered.text)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "template_sha256": rendered.template_digest,
            "prompt": rendered.text,
            "tokens": tokens.as_slice().iter().map(|id| id.as_u32()).collect::<Vec<_>>()
        }))?
    );
    Ok(())
}
