use std::env;

use ml::{load_gguf_model, GptOssTokenizer, MlError, Result};

fn main() -> Result<()> {
    let mut gguf_path: Option<String> = None;
    let mut text = "Hello, world!".to_string();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gguf" => gguf_path = args.next(),
            "--text" => text = args.next().unwrap_or_default(),
            _ => {}
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig("usage: test_tokenizer --gguf <path> --text <prompt>".to_string())
    })?;

    let model = load_gguf_model(&gguf_path)?;
    let tokenizer_meta = model.metadata.tokenizer.ok_or_else(|| {
        MlError::Model("gguf tokenizer metadata missing".to_string())
    })?;
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
