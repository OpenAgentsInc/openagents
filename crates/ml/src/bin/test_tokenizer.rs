use std::collections::HashMap;
use std::env;

use ml::{load_gguf_model, GptOssTokenizer, MlError, Result};

fn main() -> Result<()> {
    let mut gguf_path: Option<String> = None;
    let mut text = "Hello, world!".to_string();
    let mut debug = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gguf" => gguf_path = args.next(),
            "--text" => text = args.next().unwrap_or_default(),
            "--debug" => debug = true,
            _ => {}
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig(
            "usage: test_tokenizer --gguf <path> --text <prompt> [--debug]".to_string(),
        )
    })?;

    let model = load_gguf_model(&gguf_path)?;
    let tokenizer_meta = model.metadata.tokenizer.ok_or_else(|| {
        MlError::Model("gguf tokenizer metadata missing".to_string())
    })?;
    if debug {
        let mut type_counts: HashMap<i32, usize> = HashMap::new();
        for token_type in &tokenizer_meta.token_types {
            *type_counts.entry(*token_type).or_insert(0) += 1;
        }
        println!("token_types: {type_counts:?}");
        let mut sample_by_type: HashMap<i32, Vec<(usize, String)>> = HashMap::new();
        for (idx, token) in tokenizer_meta.tokens.iter().enumerate() {
            let token_type = tokenizer_meta
                .token_types
                .get(idx)
                .copied()
                .unwrap_or(0);
            let entry = sample_by_type.entry(token_type).or_default();
            if entry.len() < 5 {
                entry.push((idx, token.clone()));
            }
        }
        for (token_type, samples) in sample_by_type.iter() {
            if *token_type != 1 {
                println!("type {token_type} samples: {samples:?}");
            }
        }
        for (idx, token) in tokenizer_meta.tokens.iter().take(8).enumerate() {
            let token_type = tokenizer_meta
                .token_types
                .get(idx)
                .copied()
                .unwrap_or(0);
            println!("token[{idx}] type={token_type} text={token:?}");
        }
        println!("pattern: {}", tokenizer_meta.pattern);
        println!("model: {:?}", tokenizer_meta.model);
        println!("pre: {:?}", tokenizer_meta.pre);
        let probes = [
            "Hello",
            " Hello",
            "\u{0120}Hello",
            "world",
            " world",
            "\u{0120}world",
            "!",
            ",",
        ];
        for probe in probes {
            let found = tokenizer_meta.tokens.iter().position(|token| token == probe);
            println!("probe token {probe:?}: {found:?}");
        }
    }

    let tokenizer = GptOssTokenizer::from_gguf(tokenizer_meta)
        .map_err(|err| MlError::Model(err))?;

    let tokens = tokenizer
        .encode_with_special_tokens(&text)
        .map_err(|err| MlError::Model(err))?;
    let decoded = tokenizer.decode_utf8_lossy(&tokens);

    println!("text: {text}");
    println!("tokens: {}", tokens.len());
    println!("token_ids: {:?}", tokens);
    println!("decoded: {decoded}");
    Ok(())
}
