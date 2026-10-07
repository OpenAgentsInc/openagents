use std::{env, error::Error, path::PathBuf};

use psionic_models::{GgufDecoderAdapterLoader, PromptMessage, PromptMessageRole};
use psionic_serve::{
    DistributedGemma4PeerConfig, DistributedGemma4TextGenerationService, GenerationOptions,
    GenerationRequest, TextGenerationExecutor,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let model_path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("/Users/christopherdavid/models/gemma4/gemma4-e4b-ollama.gguf")
    });
    let peer_base_url = args
        .next()
        .unwrap_or_else(|| String::from("http://100.108.56.85:18130"));
    let prompt = args.next().unwrap_or_else(|| String::from("Say yes."));
    let max_output_tokens = args
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2);
    let split_layer = args
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(21);

    let peer = DistributedGemma4PeerConfig {
        peer_base_url,
        split_layer,
        shared_key: None,
    };
    let adapter = GgufDecoderAdapterLoader.load_path(&model_path)?;
    let rendered = adapter.render_prompt(
        None,
        &[PromptMessage::new(PromptMessageRole::User, prompt)],
        true,
    )?;
    let tokens = adapter
        .prompt_renderer()
        .tokenize_rendered_prompt(rendered.text.as_str())?;

    let mut service = DistributedGemma4TextGenerationService::from_gguf_path(&model_path, peer)?;
    let mut options = GenerationOptions::greedy(max_output_tokens);
    options.stop_sequences = rendered.stop_sequences.clone();
    let request = GenerationRequest::new_tokens(
        String::from("distributed-gemma4-live-smoke"),
        service.model_descriptor().clone(),
        None,
        tokens,
        options,
    );
    let response = service.generate(&request)?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}
