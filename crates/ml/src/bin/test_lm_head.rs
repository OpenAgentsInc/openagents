use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use ml::{
    dot_q8_0_row, find_tensor, load_gguf_model, read_q8_0_row, GptOssTokenizer, MlError, Result,
};

fn main() -> Result<()> {
    let mut gguf_path: Option<String> = None;
    let mut token_id: Option<u32> = None;
    let mut top_k: usize = 5;
    let mut limit_rows: Option<usize> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gguf" => gguf_path = args.next(),
            "--token" => token_id = args.next().and_then(|v| v.parse::<u32>().ok()),
            "--show-top" => {
                top_k = args
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(5);
            }
            "--limit-rows" => {
                limit_rows = args.next().and_then(|v| v.parse::<usize>().ok());
            }
            _ => {}
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig(
            "usage: test_lm_head --gguf <path> --token 123 --show-top 5".to_string(),
        )
    })?;
    let token_id = token_id.unwrap_or(0);
    let top_k = top_k.max(1);

    let model = load_gguf_model(&gguf_path)?;
    let tokenizer = model
        .metadata
        .tokenizer
        .clone()
        .ok_or_else(|| MlError::Model("gguf tokenizer metadata missing".to_string()))
        .and_then(|meta| GptOssTokenizer::from_gguf(meta).map_err(MlError::Model))?;

    let token_embd = find_tensor(&model.index, "token_embd.weight")?;
    let output_weight = find_tensor(&model.index, "output.weight")?;
    let path = PathBuf::from(&gguf_path);
    let input = read_q8_0_row(&path, token_embd, token_id as usize)?;

    let rows = output_weight.dims.get(0).copied().unwrap_or(0) as usize;
    let cols = output_weight.dims.get(1).copied().unwrap_or(0) as usize;
    if cols != input.len() || rows == 0 {
        return Err(MlError::Model(format!(
            "lm_head shape mismatch rows={rows} cols={cols} input={}",
            input.len()
        )));
    }
    let row_bytes = (cols / 32) * 34;
    if cols % 32 != 0 {
        return Err(MlError::Model(
            "lm_head cols not divisible by Q8_0 block size".to_string(),
        ));
    }

    let mut file = File::open(&path)?;
    file.seek(SeekFrom::Start(output_weight.absolute_offset))?;
    let total_rows = limit_rows.unwrap_or(rows).min(rows);
    let mut buf = vec![0u8; row_bytes];
    let mut top: Vec<(f32, usize)> = Vec::with_capacity(top_k);

    for row in 0..total_rows {
        file.read_exact(&mut buf)?;
        let score = dot_q8_0_row(&buf, &input)?;
        if top.len() < top_k {
            top.push((score, row));
            continue;
        }
        let mut min_idx = 0;
        let mut min_val = top[0].0;
        for (idx, entry) in top.iter().enumerate().skip(1) {
            if entry.0 < min_val {
                min_val = entry.0;
                min_idx = idx;
            }
        }
        if score > min_val {
            top[min_idx] = (score, row);
        }
    }

    top.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    println!("token_id={token_id} rows_scanned={total_rows}/{rows}");
    for (rank, (score, row)) in top.iter().enumerate() {
        let token_text = tokenizer.token_text(*row as u32);
        println!(
            "#{:02} token={} score={:.5} text={}",
            rank + 1,
            row,
            score,
            token_text.replace('\n', "\\n")
        );
    }

    Ok(())
}
