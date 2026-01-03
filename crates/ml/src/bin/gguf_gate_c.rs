#[cfg(all(feature = "candle", feature = "wgpu"))]
mod gate {
    use std::env;
    use std::path::PathBuf;

    use ml::{run_q8_0_gate, GateConfig, MlError, Result};

    const DEFAULT_TENSOR: &str = "output.weight";
    const DEFAULT_K: usize = 128;
    const DEFAULT_N: usize = 64;
    const DEFAULT_TOLERANCE: f32 = 0.01;

    pub fn run() -> Result<()> {
        let args: Vec<String> = env::args().skip(1).collect();
        if args.is_empty() {
            return Err(MlError::InvalidConfig(
                "usage: gguf_gate_c <path> [--tensor NAME | --tensor-index N] [--k N] [--n N] [--tolerance F] [--dump N]".to_string(),
            ));
        }

        let mut path: Option<PathBuf> = None;
        let mut tensor_name: Option<String> = Some(DEFAULT_TENSOR.to_string());
        let mut tensor_index: Option<usize> = None;
        let mut k: usize = DEFAULT_K;
        let mut n: usize = DEFAULT_N;
        let mut tolerance: f32 = DEFAULT_TOLERANCE;
        let mut dump: usize = 0;

        let mut idx = 0;
        while idx < args.len() {
            match args[idx].as_str() {
                "--tensor" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --tensor".to_string())
                    })?;
                    tensor_name = Some(value.to_string());
                    tensor_index = None;
                }
                "--tensor-index" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --tensor-index".to_string())
                    })?;
                    tensor_index = Some(parse_usize(value, "--tensor-index")?);
                    tensor_name = None;
                }
                "--k" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --k".to_string())
                    })?;
                    k = parse_usize(value, "--k")?;
                }
                "--n" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --n".to_string())
                    })?;
                    n = parse_usize(value, "--n")?;
                }
                "--tolerance" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --tolerance".to_string())
                    })?;
                    tolerance = parse_f32(value, "--tolerance")?;
                }
                "--dump" => {
                    idx += 1;
                    let value = args.get(idx).ok_or_else(|| {
                        MlError::InvalidConfig("missing value for --dump".to_string())
                    })?;
                    dump = parse_usize(value, "--dump")?;
                }
                arg => {
                    if path.is_some() {
                        return Err(MlError::InvalidConfig(format!(
                            "unexpected argument: {arg}"
                        )));
                    }
                    path = Some(PathBuf::from(arg));
                }
            }
            idx += 1;
        }

        let path = path.ok_or_else(|| MlError::InvalidConfig("missing gguf path".to_string()))?;

        let config = GateConfig {
            path: path.clone(),
            tensor_name,
            tensor_index,
            k,
            n,
        };

        let outcome = run_q8_0_gate(&config)?;
        println!("gguf: {}", path.display());
        println!("tensor: {} ({})", outcome.tensor_name, outcome.tensor_type);
        println!("k: {} n: {}", outcome.k, outcome.n);
        println!("bytes_read: {}", outcome.bytes_read);
        println!("max_abs_diff: {}", outcome.max_abs);
        println!("mean_abs_diff: {}", outcome.mean_abs);
        println!("tolerance: {tolerance}");

        if dump > 0 {
            let count = dump.min(outcome.y_cpu.len());
            println!("sample:");
            for i in 0..count {
                let cpu = outcome.y_cpu[i];
                let gpu = outcome.y_gpu[i];
                let diff = (cpu - gpu).abs();
                println!("{i:4} cpu={cpu:.6} gpu={gpu:.6} diff={diff:.6}");
            }
        }

        if outcome.max_abs > tolerance {
            return Err(MlError::Model(format!(
                "max_abs_diff {} exceeds tolerance {}",
                outcome.max_abs, tolerance
            )));
        }

        Ok(())
    }

    fn parse_usize(value: &str, flag: &str) -> Result<usize> {
        value.parse().map_err(|_| {
            MlError::InvalidConfig(format!("invalid value for {flag}: {value}"))
        })
    }

    fn parse_f32(value: &str, flag: &str) -> Result<f32> {
        value.parse().map_err(|_| {
            MlError::InvalidConfig(format!("invalid value for {flag}: {value}"))
        })
    }
}

#[cfg(all(feature = "candle", feature = "wgpu"))]
fn main() -> ml::Result<()> {
    gate::run()
}

#[cfg(not(all(feature = "candle", feature = "wgpu")))]
fn main() {
    eprintln!("gguf_gate_c requires the candle and wgpu features");
    std::process::exit(1);
}
