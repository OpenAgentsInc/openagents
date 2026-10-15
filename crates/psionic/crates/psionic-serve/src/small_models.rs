//! Router-only serving in the existing Psionic server executable.

use std::{
    io::{self, Write},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use psionic_inference::{DeviceChoice, EngineKind, service::RouterService};

use crate::{clef, cors};

#[derive(Clone, Debug)]
pub struct RouterServerOptions {
    pub model_dir: PathBuf,
    pub engine: EngineKind,
    pub device: DeviceChoice,
    pub allow_cpu_fallback: bool,
    pub threads: usize,
    pub max_queue: usize,
    pub library: Option<PathBuf>,
    pub library_sha256: Option<String>,
    pub address: SocketAddr,
}

impl RouterServerOptions {
    pub fn parse(args: &[String]) -> Result<Option<Self>, String> {
        if !args
            .iter()
            .any(|a| a.starts_with("--router-") || a == "--ort-library" || a == "--ort-sha256")
        {
            return Ok(None);
        }
        let mut root = None;
        let mut engine = None;
        let mut threads = 4;
        let mut device = DeviceChoice::Cpu;
        let mut allow_cpu_fallback = false;
        let mut max_queue = 8;
        let mut library = None;
        let mut checksum = None;
        let mut host: IpAddr = "127.0.0.1".parse().map_err(|_| "Invalid default address")?;
        let mut port = 8080;
        let mut args = args.iter();
        while let Some(flag) = args.next() {
            if ![
                "--router-model-dir",
                "--router-engine",
                "--router-device",
                "--router-allow-cpu-fallback",
                "--router-threads",
                "--router-max-queue",
                "--ort-library",
                "--ort-sha256",
                "--host",
                "--port",
            ]
            .contains(&flag.as_str())
            {
                return Err(format!(
                    "Router-only mode does not take {flag}; start a separate server for GGUF models"
                ));
            }
            if flag == "--router-allow-cpu-fallback" {
                allow_cpu_fallback = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("Provide a value for {flag}"))?;
            match flag.as_str() {
                "--router-model-dir" => root = Some(PathBuf::from(value)),
                "--router-engine" => {
                    engine = Some(match value.as_str() {
                        "native" => EngineKind::Native,
                        "ort" => EngineKind::Ort,
                        _ => return Err("Choose native or ort for --router-engine".into()),
                    })
                }
                "--router-device" => {
                    device = match value.as_str() {
                        "cpu" => DeviceChoice::Cpu,
                        "cuda" => DeviceChoice::Cuda,
                        "metal" => DeviceChoice::Metal,
                        "coreml" => DeviceChoice::CoreMl,
                        _ => {
                            return Err(
                                "Choose cpu, cuda, metal, or coreml for --router-device".into()
                            );
                        }
                    }
                }
                "--router-threads" => {
                    threads = value
                        .parse::<usize>()
                        .ok()
                        .filter(|n| (1..=256).contains(n))
                        .ok_or("Choose between 1 and 256 router threads")?
                }
                "--router-max-queue" => {
                    max_queue = value
                        .parse::<usize>()
                        .ok()
                        .filter(|n| (1..=256).contains(n))
                        .ok_or("Choose a queue limit between 1 and 256")?
                }
                "--ort-library" => library = Some(PathBuf::from(value)),
                "--ort-sha256" => checksum = Some(value.clone()),
                "--host" => {
                    host = value
                        .parse()
                        .map_err(|_| "Provide an IP address for --host")?
                }
                "--port" => {
                    port = value
                        .parse::<u16>()
                        .map_err(|_| "Choose a port between 0 and 65535")?
                }
                _ => return Err(format!("Unknown option: {flag}")),
            }
        }
        let engine = engine.ok_or("Choose --router-engine native or ort explicitly")?;
        if (engine == EngineKind::Native && device == DeviceChoice::CoreMl)
            || (engine == EngineKind::Ort && device == DeviceChoice::Metal)
        {
            return Err(
                "Choose CPU, CUDA, or Metal for native; CPU, CUDA, or Core ML for ORT".into(),
            );
        }
        if allow_cpu_fallback && (engine != EngineKind::Ort || device == DeviceChoice::Cpu) {
            return Err("CPU fallback is an ORT accelerator option".into());
        }
        if engine == EngineKind::Ort && (library.is_none() || checksum.is_none()) {
            return Err("ORT requires --ort-library and --ort-sha256".into());
        }
        if engine == EngineKind::Native && (library.is_some() || checksum.is_some()) {
            return Err("ORT library options require --router-engine ort".into());
        }
        Ok(Some(Self {
            model_dir: root.ok_or("Provide --router-model-dir")?,
            engine,
            device,
            allow_cpu_fallback,
            threads,
            max_queue,
            library,
            library_sha256: checksum,
            address: SocketAddr::new(host, port),
        }))
    }

    pub async fn serve(self, origins: cors::AllowedOrigins) -> Result<(), String> {
        let model = RouterService::load_on(
            &self.model_dir,
            self.engine,
            self.threads,
            self.library.as_deref(),
            self.library_sha256.as_deref(),
            self.device,
            self.allow_cpu_fallback,
        )
        .map_err(|e| e.to_string())?;
        let router =
            psionic_inference::http::router(model, self.max_queue).map_err(|e| e.to_string())?;
        let listener = tokio::net::TcpListener::bind(self.address)
            .await
            .map_err(|e| e.to_string())?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let _ = writeln!(
            io::stdout(),
            "Psionic router listening on http://{address}, engine={:?}, device={:?}",
            self.engine,
            self.device
        );
        clef::serve(listener, cors::allow_origins(router, origins))
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(flags: &[&str]) -> Result<RouterServerOptions, String> {
        RouterServerOptions::parse(&flags.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            .map(|v| v.unwrap())
    }
    #[test]
    fn router_device_defaults_to_cpu_and_accepts_explicit_native_cuda() {
        let options = parse(&["--router-model-dir", "model", "--router-engine", "native"]).unwrap();
        assert_eq!(options.device, DeviceChoice::Cpu);
        let options = parse(&[
            "--router-model-dir",
            "model",
            "--router-engine",
            "native",
            "--router-device",
            "cuda",
        ])
        .unwrap();
        assert_eq!(options.device, DeviceChoice::Cuda);
        assert!(!options.allow_cpu_fallback);
    }
    #[test]
    fn router_rejects_incompatible_engine_devices_and_native_fallback() {
        for (engine, device) in [("native", "coreml"), ("ort", "metal")] {
            assert!(
                parse(&[
                    "--router-model-dir",
                    "model",
                    "--router-engine",
                    engine,
                    "--router-device",
                    device
                ])
                .is_err()
            );
        }
        assert!(
            parse(&[
                "--router-model-dir",
                "model",
                "--router-engine",
                "native",
                "--router-allow-cpu-fallback"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "--router-model-dir",
                "model",
                "--router-engine",
                "native",
                "--router-device",
                "gpu"
            ])
            .is_err()
        );
    }
    #[test]
    fn ort_gpu_fallback_is_explicit_and_does_not_consume_the_next_flag() {
        let options = parse(&[
            "--router-model-dir",
            "model",
            "--router-engine",
            "ort",
            "--router-device",
            "cuda",
            "--router-allow-cpu-fallback",
            "--port",
            "0",
            "--ort-library",
            "runtime",
            "--ort-sha256",
            "checksum",
        ])
        .unwrap();
        assert_eq!(options.device, DeviceChoice::Cuda);
        assert!(options.allow_cpu_fallback);
        assert_eq!(options.address.port(), 0);
        assert!(
            parse(&[
                "--router-model-dir",
                "model",
                "--router-engine",
                "ort",
                "--router-device",
                "cpu",
                "--router-allow-cpu-fallback",
                "--ort-library",
                "runtime",
                "--ort-sha256",
                "checksum"
            ])
            .is_err()
        );
    }
}
