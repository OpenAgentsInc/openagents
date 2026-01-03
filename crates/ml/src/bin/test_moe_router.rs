use std::env;
use std::path::PathBuf;

use ml::{
    find_tensor, load_gguf_model, matmul_f32, read_f32_tensor, read_meta_u32, read_q8_0_row,
    top_k_softmax, MlError, Result,
};

fn main() -> Result<()> {
    let mut gguf_path: Option<String> = None;
    let mut layer: usize = 0;
    let mut token_id: u32 = 0;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gguf" => gguf_path = args.next(),
            "--layer" => {
                layer = args
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);
            }
            "--token" => {
                token_id = args
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(0);
            }
            _ => {}
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig(
            "usage: test_moe_router --gguf <path> --layer 0 --token 0".to_string(),
        )
    })?;

    let model = load_gguf_model(&gguf_path)?;
    let experts_per_token = read_meta_u32(&model.metadata, "llama.expert_used_count")? as usize;
    let path = PathBuf::from(&gguf_path);

    let token_embd = find_tensor(&model.index, "token_embd.weight")?;
    let hidden = read_q8_0_row(&path, token_embd, token_id as usize)?;

    let gate_w = find_tensor(&model.index, &format!("blk.{layer}.ffn_gate_inp.weight"))?;
    let gate_b = find_tensor(&model.index, &format!("blk.{layer}.ffn_gate_inp.bias"))?;
    let weights = read_f32_tensor(&path, gate_w)?;
    let bias = read_f32_tensor(&path, gate_b)?;

    let n = gate_w.dims.get(0).copied().unwrap_or(0) as usize;
    let k = gate_w.dims.get(1).copied().unwrap_or(0) as usize;
    if k != hidden.len() || n == 0 {
        return Err(MlError::Model(format!(
            "router shape mismatch n={n} k={k} hidden={}",
            hidden.len()
        )));
    }
    let mut scores = matmul_f32(&weights, &hidden, k, n);
    if bias.len() == scores.len() {
        for (s, b) in scores.iter_mut().zip(bias.iter()) {
            *s += *b;
        }
    }
    let (indices, weights) = top_k_softmax(&scores, experts_per_token.max(1))?;
    println!("layer={layer} token={token_id} topk={}", indices.len());
    for (idx, weight) in indices.iter().zip(weights.iter()) {
        println!("expert={idx} weight={weight:.4}");
    }

    Ok(())
}
