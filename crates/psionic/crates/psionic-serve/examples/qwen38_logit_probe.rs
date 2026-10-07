use std::{env, error::Error, path::PathBuf};

use psionic_serve::{CpuGgufQwen35TextGenerationService, TokenId};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let gguf_path = PathBuf::from(
        args.next()
            .ok_or("usage: qwen38_logit_probe <decoder.gguf> <token-id,token-id,...>")?,
    );
    let token_ids_arg = args.next().ok_or("missing comma-separated token ids")?;
    if args.next().is_some() {
        return Err("unexpected trailing arguments".into());
    }
    let token_ids = token_ids_arg
        .split(',')
        .map(|value| value.parse::<u32>().map(TokenId))
        .collect::<Result<Vec<_>, _>>()?;

    let service = CpuGgufQwen35TextGenerationService::from_gguf_path(&gguf_path)?;
    let (_, logits) = service.final_hidden_and_logits_for_tokens(token_ids.as_slice())?;

    let mut ranked = logits
        .iter()
        .enumerate()
        .collect::<Vec<(usize, &f32)>>();
    ranked.sort_by(|left, right| right.1.total_cmp(left.1));
    for (index, value) in ranked.iter().take(5) {
        println!("rank token_id={} logit={:.6}", index, value);
    }
    Ok(())
}
