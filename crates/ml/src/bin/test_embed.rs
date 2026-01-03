use std::env;
use std::path::PathBuf;

use ml::{find_tensor, load_gguf_model, read_q8_0_row, MlError, Result};

fn main() -> Result<()> {
    let mut gguf_path: Option<String> = None;
    let mut tokens: Vec<u32> = Vec::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gguf" => gguf_path = args.next(),
            "--tokens" => {
                if let Some(raw) = args.next() {
                    tokens = raw
                        .split(',')
                        .filter_map(|v| v.trim().parse::<u32>().ok())
                        .collect();
                }
            }
            _ => {}
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig("usage: test_embed --gguf <path> --tokens 123,456".to_string())
    })?;
    if tokens.is_empty() {
        tokens.push(0);
    }

    let model = load_gguf_model(&gguf_path)?;
    let token_embd = find_tensor(&model.index, "token_embd.weight")?;
    let path = PathBuf::from(&gguf_path);

    for token_id in tokens {
        let row = read_q8_0_row(&path, token_embd, token_id as usize)?;
        let preview: Vec<String> = row.iter().take(8).map(|v| format!("{v:.4}")).collect();
        println!(
            "token={token_id} dim={} preview=[{}]",
            row.len(),
            preview.join(", ")
        );
    }

    Ok(())
}
