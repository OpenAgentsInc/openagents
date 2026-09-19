//! `kev-serve`: load the artifact bundle and serve `POST /v1/systemone`.
//!
//! ```text
//! kev-serve --adapter-dir ~/work/kev-artifacts/kev-0.5b \
//!           --base-dir   ~/work/kev-artifacts/qwen2.5-0.5b \
//!           --port 8009
//! ```

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use candle_core::{DType, Device};
use kev::decision::DecisionModel;
use kev::lora::LoraConfig;
use kev::serve::{ServeState, router};

struct Args {
    adapter_dir: PathBuf,
    base_dir: PathBuf,
    host: String,
    port: u16,
    model_id: String,
    device: String,
    dtype: String,
}

fn parse_args() -> Result<Args, String> {
    let mut adapter_dir = std::env::var("KEV_ARTIFACT_DIR").ok();
    let mut base_dir = std::env::var("KEV_BASE_DIR").ok();
    let mut host = "127.0.0.1".to_string();
    let mut port = 8009u16;
    let mut model_id = "kev-latest".to_string();
    let mut device = "cpu".to_string();
    let mut dtype = std::env::var("KEV_DTYPE").unwrap_or_else(|_| "fp32".to_string());
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut take = |name: &str| {
            args.next()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--adapter-dir" => adapter_dir = Some(take("--adapter-dir")?),
            "--base-dir" => base_dir = Some(take("--base-dir")?),
            "--host" => host = take("--host")?,
            "--port" => {
                port = take("--port")?
                    .parse()
                    .map_err(|_| "--port needs a number".to_string())?
            }
            "--model-id" => model_id = take("--model-id")?,
            "--device" => device = take("--device")?,
            "--dtype" => dtype = take("--dtype")?,
            "--help" | "-h" => {
                eprintln!(
                    "kev-serve --adapter-dir DIR --base-dir DIR [--host H] [--port P] [--model-id ID] [--device cpu|metal] [--dtype fp32|bf16]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let adapter_dir = adapter_dir.ok_or("--adapter-dir is required (or set KEV_ARTIFACT_DIR)")?;
    let base_dir = base_dir.ok_or("--base-dir is required (or set KEV_BASE_DIR)")?;
    Ok(Args {
        adapter_dir: PathBuf::from(adapter_dir),
        base_dir: PathBuf::from(base_dir),
        host,
        port,
        model_id,
        device,
        dtype,
    })
}

fn device(name: &str) -> Result<Device, String> {
    match name {
        "cpu" => Ok(Device::Cpu),
        "metal" => Device::new_metal(0).map_err(|e| format!("metal unavailable: {e}")),
        other => Err(format!("unknown device {other} (cpu or metal)")),
    }
}

fn dtype(name: &str) -> Result<DType, String> {
    match name {
        "fp32" | "f32" => Ok(DType::F32),
        "bf16" => Ok(DType::BF16),
        other => Err(format!("unknown dtype {other} (fp32 or bf16)")),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("kev-serve: {e}");
            return ExitCode::from(2);
        }
    };
    let device = match device(&args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("kev-serve: {e}");
            return ExitCode::from(2);
        }
    };
    let dtype = match dtype(&args.dtype) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("kev-serve: {e}");
            return ExitCode::from(2);
        }
    };
    let lora: LoraConfig = std::fs::read_to_string(args.adapter_dir.join("adapter_config.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(LoraConfig {
            r: 0,
            lora_alpha: 0.0,
            target_modules: Vec::new(),
        });
    let base = std::fs::read_to_string(args.adapter_dir.join("head_meta.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|meta| meta.get("base")?.as_str().map(str::to_string))
        .unwrap_or_else(|| args.base_dir.display().to_string());
    eprintln!(
        "kev-serve: loading {} + {} ({}, {})",
        args.base_dir.display(),
        args.adapter_dir.display(),
        args.device,
        args.dtype,
    );
    let model = match DecisionModel::load_with_dtype(
        &args.base_dir,
        &args.adapter_dir,
        device,
        dtype,
    ) {
        Ok(model) => model,
        Err(e) => {
            eprintln!("kev-serve: {e}");
            return ExitCode::from(1);
        }
    };
    let state = Arc::new(ServeState {
        model,
        model_id: args.model_id.clone(),
        aliases: vec!["jev-latest".to_string()],
        run: args.adapter_dir.display().to_string(),
        base,
        lora: lora.r,
        device: args.device,
    });
    let addr: SocketAddr = match format!("{}:{}", args.host, args.port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("kev-serve: bad listen address: {e}");
            return ExitCode::from(2);
        }
    };
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("kev-serve: bind {addr}: {e}");
            return ExitCode::from(1);
        }
    };
    eprintln!("kev-serve: {} listening on http://{addr}", args.model_id);
    match axum::serve(listener, router(state)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kev-serve: {e}");
            ExitCode::from(1)
        }
    }
}
