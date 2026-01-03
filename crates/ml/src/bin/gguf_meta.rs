use std::env;

use ml::{load_gguf_model, GgufScalar, MlError};

fn main() -> Result<(), MlError> {
    let mut gguf_path: Option<String> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--gguf" => gguf_path = args.next(),
            _ => {
                if gguf_path.is_none() {
                    gguf_path = Some(arg);
                }
            }
        }
    }

    let gguf_path = gguf_path.ok_or_else(|| {
        MlError::InvalidConfig("usage: gguf_meta --gguf <path>".to_string())
    })?;

    let model = load_gguf_model(&gguf_path)?;
    let mut keys: Vec<_> = model.metadata.values.iter().collect();
    keys.sort_by(|a, b| a.0.cmp(b.0));

    println!("gguf: {gguf_path}");
    for (key, value) in keys {
        println!("{key} = {}", format_scalar(value));
    }
    Ok(())
}

fn format_scalar(value: &GgufScalar) -> String {
    match value {
        GgufScalar::U8(v) => v.to_string(),
        GgufScalar::I8(v) => v.to_string(),
        GgufScalar::U16(v) => v.to_string(),
        GgufScalar::I16(v) => v.to_string(),
        GgufScalar::U32(v) => v.to_string(),
        GgufScalar::I32(v) => v.to_string(),
        GgufScalar::U64(v) => v.to_string(),
        GgufScalar::I64(v) => v.to_string(),
        GgufScalar::F32(v) => format!("{v:.6}"),
        GgufScalar::F64(v) => format!("{v:.6}"),
        GgufScalar::Bool(v) => v.to_string(),
        GgufScalar::String(v) => format!("{v:?}"),
    }
}
