//! One-command setup for a decision pylon: `doctor`, `setup`, `start`,
//! `update`.
//!
//! `doctor` reads this computer (system, memory, GPU, disk, the pinned
//! model's id and file hash, the Psionic server, outbound reach to the
//! relay, the last self-check) and says whether it can serve Clef-Flash
//! decisions, what blocks it, and the one command that fixes each block.
//! A model is identified by its tensors ([`crate::model_id`]), so a copy
//! whose header differs from the download is still the pinned model.
//! `setup` fetches the pinned model and checks its digest. `start` runs
//! Psionic on loopback, self-checks it, and serves decisions; `update`
//! re-verifies the model against the pins this build carries and
//! re-fetches it when they changed.
//!
//! A pylon dials out to the relay: nothing here opens or asks for an
//! inbound port, a wallet seed, a registration fee, or a token. The shape
//! (read-only doctor with fixes, pinned digests, a self-check before
//! serving, updates that never silently change what is served) follows
//! REAX's miner kit (`reaxlabs/reax-miner-kit`, MIT); the code is ours.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The pinned decision model's file name.
pub const CLEF_FLASH_FILE: &str = "Clef-Flash-Q4_K_M.gguf";
/// Where it comes from: `ggml-org/Clef-Flash-GGUF` at a pinned revision
/// (Apache-2.0).
pub const CLEF_FLASH_URL: &str = "https://huggingface.co/ggml-org/Clef-Flash-GGUF/resolve/6b4fe4c59098479652d94f0fe11a40facbebd1ec/Clef-Flash-Q4_K_M.gguf";
/// The downloaded file's sha256: what `fetch` checks the download
/// against. Not the model's identity (see [`CLEF_FLASH_MODEL_ID`]).
pub const CLEF_FLASH_SHA256: &str =
    "3626116623ca8d89641952471fac9ce12af84fb3517520029a2ac6f5146868a2";
/// The model's identity: [`crate::model_id`]'s digest of its tensors,
/// the same for every file with these weights whatever its header says
/// (coderos-4080's copy hashes to `fd3e9060…` and has this id).
pub const CLEF_FLASH_MODEL_ID: &str =
    "gguf-tensors:sha256:2e417b2214d9a0b8736d850a1d80ffa4709be32056673b2f201f59cf8e6d07a3";
/// Its size in bytes.
pub const CLEF_FLASH_BYTES: u64 = 6_486_448_288;
/// The Psionic server program.
pub const SERVER: &str = "psionic-openai-server";
/// The loopback port `start` runs Psionic's decision lane on.
pub const DECIDE_PORT: u16 = 18096;
/// The command that builds the Psionic server from this repository.
pub const BUILD_SERVER: &str = "cargo build --release --manifest-path crates/psionic/Cargo.toml -p psionic-serve --bin psionic-openai-server && cp crates/psionic/target/release/psionic-openai-server ~/.openagents/bin/";

/// Where models live: `~/.openagents/models`.
#[must_use]
pub fn models_dir() -> PathBuf {
    user_home::home_os()
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents")
        .join("models")
}

/// The pinned model's path.
#[must_use]
pub fn model_path() -> PathBuf {
    models_dir().join(CLEF_FLASH_FILE)
}

/// The Psionic server, if it is on this computer: the folder in
/// `OPENAGENTS_PSIONIC_BIN`, then `PATH`, then `~/.openagents/bin`.
#[must_use]
pub fn find_server() -> Option<PathBuf> {
    let home = user_home::home_os().map(PathBuf::from);
    std::env::var_os("OPENAGENTS_PSIONIC_BIN")
        .map(PathBuf::from)
        .into_iter()
        .chain(
            std::env::var_os("PATH")
                .into_iter()
                .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>()),
        )
        .chain(home.map(|h| h.join(".openagents").join("bin")))
        .map(|dir| dir.join(SERVER))
        .find(|path| path.is_file())
}

/// Bytes in a gigabyte as memory and disks are sold (GiB).
const GIB: f64 = 1_073_741_824.0;

/// What a computer has, as `doctor` sees it.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Host {
    pub os: String,
    pub arch: String,
    pub cpus: usize,
    pub memory_gb: Option<f64>,
    /// `apple` (Metal), `nvidia` (CUDA), or `none`.
    pub gpu: String,
    pub gpu_name: Option<String>,
    pub gpu_memory_gb: Option<f64>,
    pub driver: Option<String>,
    pub disk_free_gb: Option<f64>,
}

/// Read this computer. Never writes anything.
#[must_use]
pub fn host() -> Host {
    let mut host = Host {
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        cpus: std::thread::available_parallelism().map_or(1, usize::from),
        gpu: "none".into(),
        ..Host::default()
    };
    let out = |cmd: &str, args: &[&str]| -> Option<String> {
        let output = std::process::Command::new(cmd).args(args).output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    if host.os == "macos" {
        host.memory_gb = out("sysctl", &["-n", "hw.memsize"])
            .and_then(|s| s.parse::<f64>().ok())
            .map(|b| b / GIB);
        if host.arch == "aarch64" {
            host.gpu = "apple".into();
            host.gpu_name = out("sysctl", &["-n", "machdep.cpu.brand_string"]);
            host.gpu_memory_gb = host.memory_gb;
        }
    } else {
        host.memory_gb = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|text| parse_meminfo(&text));
    }
    if host.gpu == "none"
        && let Some(text) = out(
            "nvidia-smi",
            &[
                "--query-gpu=name,memory.total,driver_version",
                "--format=csv,noheader,nounits",
            ],
        )
        && let Some((name, mib, driver)) = parse_nvidia_smi(&text)
    {
        host.gpu = "nvidia".into();
        host.gpu_name = Some(name);
        host.gpu_memory_gb = Some(mib / 1024.0);
        host.driver = Some(driver);
    }
    let dir = models_dir();
    let probe = if dir.exists() {
        dir
    } else {
        user_home::home_os().map_or_else(|| PathBuf::from("."), PathBuf::from)
    };
    host.disk_free_gb = out("df", &["-k", &probe.to_string_lossy()]).and_then(|t| parse_df(&t));
    host
}

/// Total memory from `/proc/meminfo`, in GB.
#[must_use]
pub fn parse_meminfo(text: &str) -> Option<f64> {
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024.0 / GIB)
}

/// The first GPU from `nvidia-smi --query-gpu=name,memory.total,driver_version
/// --format=csv,noheader,nounits`: name, MiB, driver.
#[must_use]
pub fn parse_nvidia_smi(text: &str) -> Option<(String, f64, String)> {
    let line = text.lines().next()?;
    let mut parts = line.split(',').map(str::trim);
    let name = parts.next()?.to_string();
    let mib: f64 = parts.next()?.parse().ok()?;
    let driver = parts.next()?.to_string();
    Some((name, mib, driver))
}

/// Free space from `df -k PATH`, in GB.
#[must_use]
pub fn parse_df(text: &str) -> Option<f64> {
    let line = text.lines().nth(1)?;
    let kb: f64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(kb * 1024.0 / GIB)
}

/// Which decision device this computer would serve on, and why not when
/// it cannot.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Fit {
    /// `metal`, `cuda`, or `cpu`.
    pub device: String,
    /// Whether it can serve at a useful speed.
    pub useful: bool,
    pub note: String,
}

/// Clef-Flash on Metal keeps its weights resident as f16 (about 14 GB);
/// on CUDA it was measured on a 16 GB RTX 4080; on the CPU a decision
/// takes seconds.
#[must_use]
pub fn fit(host: &Host) -> Fit {
    match host.gpu.as_str() {
        "apple" => {
            let gb = host.memory_gb.unwrap_or(0.0);
            if gb >= 24.0 {
                Fit {
                    device: "metal".into(),
                    useful: true,
                    note: format!("Apple Silicon with {gb:.0} GB: Metal, about 0.3 s a decision"),
                }
            } else {
                Fit {
                    device: "cpu".into(),
                    useful: false,
                    note: format!(
                        "Apple Silicon with {gb:.0} GB: Metal needs 24 GB or more for Clef-Flash's resident weights; the CPU works but takes seconds a decision"
                    ),
                }
            }
        }
        "nvidia" => {
            let gb = host.gpu_memory_gb.unwrap_or(0.0);
            if gb >= 12.0 {
                Fit {
                    device: "cuda".into(),
                    useful: true,
                    note: format!(
                        "{} with {gb:.0} GB: CUDA",
                        host.gpu_name.as_deref().unwrap_or("NVIDIA GPU")
                    ),
                }
            } else {
                Fit {
                    device: "cpu".into(),
                    useful: false,
                    note: format!(
                        "{} has {gb:.0} GB; Clef-Flash on CUDA was measured on 16 GB, so this would run on the CPU",
                        host.gpu_name.as_deref().unwrap_or("this GPU")
                    ),
                }
            }
        }
        _ => Fit {
            device: "cpu".into(),
            useful: false,
            note: "no Apple or NVIDIA GPU: the CPU works but takes seconds a decision".into(),
        },
    }
}

/// The model file's state.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModelState {
    pub path: PathBuf,
    pub present: bool,
    /// `true` when its model id is the pinned one (checked once per size
    /// and modification time, then remembered beside the file).
    pub verified: bool,
    /// Its tensor digest ([`crate::model_id`]), when it is a GGUF.
    pub model_id: Option<String>,
    /// The file's sha256, for display and for matching a server's
    /// reported artifact digest to this file.
    pub sha256: Option<String>,
}

fn marker(path: &Path) -> PathBuf {
    path.with_extension("gguf.id")
}

fn stamp(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(format!("{} {}", meta.len(), modified))
}

/// The sha256 of a file.
///
/// # Errors
///
/// When it cannot be read.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Check the model at `path` against the pinned `model_id`. Its model id
/// and file sha256 are computed once per size and modification time and
/// remembered beside the file (`*.gguf.id`); `rehash` ignores that.
#[must_use]
pub fn model_state(path: &Path, model_id: &str, rehash: bool) -> ModelState {
    let present = path.is_file();
    let mut state = ModelState {
        path: path.to_path_buf(),
        present,
        verified: false,
        model_id: None,
        sha256: None,
    };
    if !present {
        return state;
    }
    let now = stamp(path);
    let remembered = (!rehash)
        .then(|| std::fs::read_to_string(marker(path)).ok())
        .flatten()
        .and_then(|text| {
            let mut words = text.split_whitespace();
            let (id, sha) = (words.next()?, words.next()?);
            let rest: Vec<&str> = words.collect();
            (now.as_deref() == Some(rest.join(" ").as_str()))
                .then(|| (id.to_string(), sha.to_string()))
        });
    let (id, sha) = match remembered {
        Some(both) => both,
        None => {
            state.sha256 = sha256_file(path).ok();
            let (Ok(id), Some(sha)) = (crate::model_id::tensor_digest(path), state.sha256.clone())
            else {
                return state;
            };
            if let Some(now) = &now {
                let _ = std::fs::write(marker(path), format!("{id} {sha} {now}\n"));
            }
            (id, sha)
        }
    };
    state.verified = id == model_id;
    state.model_id = Some(id);
    state.sha256 = Some(sha);
    state
}

/// The model id of the artifact a decision server reports
/// (`sha256:<file hash>`): the pinned id when it is the pinned download,
/// else the id of whichever local GGUF has that file hash (`model`, when
/// given, then [`model_path`]). `None` when no file here matches.
#[must_use]
pub fn model_id_for(artifact_digest: &str, model: Option<&Path>) -> Option<String> {
    let file = artifact_digest
        .strip_prefix("sha256:")
        .unwrap_or(artifact_digest);
    if file == CLEF_FLASH_SHA256 {
        return Some(CLEF_FLASH_MODEL_ID.to_string());
    }
    let pinned = model_path();
    model
        .into_iter()
        .chain(std::iter::once(pinned.as_path()))
        .map(|path| model_state(path, CLEF_FLASH_MODEL_ID, false))
        .find(|state| state.sha256.as_deref() == Some(file))
        .and_then(|state| state.model_id)
}

/// Download `url` to `path`, checking `size` and `sha256` before the file
/// takes its name. `progress` gets bytes so far.
///
/// # Errors
///
/// The download fails or the file is not the pinned one (the partial file
/// is removed).
pub async fn fetch(
    url: &str,
    path: &Path,
    size: u64,
    sha256: &str,
    mut progress: impl FnMut(u64),
) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = path.with_extension("gguf.part");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("cannot download {url}: {e}"))?;
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut got = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("the download stopped: {e}"))?
    {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        hasher.update(&chunk);
        got += chunk.len() as u64;
        progress(got);
    }
    file.sync_all().map_err(|e| e.to_string())?;
    let digest = hex(&hasher.finalize());
    if got != size || digest != sha256 {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "the download is not the pinned model ({got} bytes, sha256 {digest}; wanted {size} bytes, {sha256})"
        ));
    }
    std::fs::rename(&part, path).map_err(|e| e.to_string())
}

/// Whether the relay's host answers a TCP connection: a pylon only dials
/// out, so this is the only network it needs.
pub async fn relay_reachable(relay: &str) -> bool {
    let rest = relay
        .strip_prefix("wss://")
        .or_else(|| relay.strip_prefix("ws://"))
        .unwrap_or(relay);
    let tls = relay.starts_with("wss://");
    let host = rest.split('/').next().unwrap_or(rest);
    let address = if host.contains(':') {
        host.to_string()
    } else {
        format!("{host}:{}", if tls { 443 } else { 80 })
    };
    matches!(
        tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::TcpStream::connect(address)
        )
        .await,
        Ok(Ok(_))
    )
}

/// One problem `doctor` found, and the command or step that fixes it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Block {
    pub what: String,
    pub fix: String,
}

/// What `doctor` reports.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub ready: bool,
    pub summary: String,
    pub host: Host,
    pub fit: Fit,
    pub model: ModelState,
    pub server: Option<PathBuf>,
    pub serving: Option<Value>,
    pub selfcheck: Option<crate::calibrate::SelfCheck>,
    pub relay: String,
    pub relay_reachable: bool,
    pub inbound_ports: Vec<u16>,
    pub blocks: Vec<Block>,
    pub warnings: Vec<String>,
    pub next: Vec<String>,
}

/// Put a report together from what was read. Pure, for tests.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn report(
    host: Host,
    model: ModelState,
    server: Option<PathBuf>,
    serving: Option<Value>,
    selfcheck: Option<crate::calibrate::SelfCheck>,
    relay: &str,
    relay_reachable: bool,
) -> Report {
    let fit = fit(&host);
    let mut blocks = Vec::new();
    let mut warnings = Vec::new();
    let mut next = Vec::new();
    if !relay_reachable {
        blocks.push(Block {
            what: format!("the relay {relay} cannot be reached"),
            fix: "check this computer's internet connection; a pylon only needs to dial out".into(),
        });
    }
    if !model.verified {
        let free = host.disk_free_gb.unwrap_or(f64::MAX);
        if free < CLEF_FLASH_BYTES as f64 / GIB + 2.0 {
            blocks.push(Block {
                what: format!(
                    "{free:.1} GB free; the model needs {:.1} GB",
                    CLEF_FLASH_BYTES as f64 / GIB
                ),
                fix: format!("free up space under {}", models_dir().display()),
            });
        }
        blocks.push(Block {
            what: if model.present {
                format!("{} is not the pinned model", model.path.display())
            } else {
                "the decision model is not downloaded".into()
            },
            fix: "openagents pylon setup".into(),
        });
    }
    if server.is_none() {
        blocks.push(Block {
            what: format!("{SERVER} is not installed"),
            fix: BUILD_SERVER.into(),
        });
    }
    if !fit.useful {
        warnings.push(fit.note.clone());
    }
    match &selfcheck {
        Some(check) if check.model_id == CLEF_FLASH_MODEL_ID => {
            if !check.pass {
                blocks.push(Block {
                    what: format!(
                        "the last self-check failed ({:.1}% of rounds pass)",
                        check.round_pass * 100.0
                    ),
                    fix: "openagents pylon selfcheck".into(),
                });
            }
        }
        _ => warnings.push("no self-check yet; `openagents pylon start` runs one".into()),
    }
    let ready = blocks.is_empty();
    if ready {
        if serving.is_none() {
            next.push("openagents pylon start --allow-any".into());
        }
    } else {
        for block in &blocks {
            if !next.contains(&block.fix) {
                next.push(block.fix.clone());
            }
        }
    }
    let summary = if ready {
        if serving.is_some() {
            format!(
                "ready, and the decision server is answering ({})",
                fit.device
            )
        } else {
            format!("ready to serve decisions on {}", fit.device)
        }
    } else {
        format!(
            "not ready: {}",
            blocks
                .iter()
                .map(|b| b.what.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    Report {
        ready,
        summary,
        host,
        fit,
        model,
        server,
        serving,
        selfcheck,
        relay: relay.into(),
        relay_reachable,
        inbound_ports: Vec::new(),
        blocks,
        warnings,
        next,
    }
}

/// The decision server answering at `url`, if one is: its model entry,
/// with the model id its artifact resolves to here ([`model_id_for`]).
pub async fn serving(url: &str) -> Option<Value> {
    let clef = crate::decide::Clef::new(url, None).ok()?;
    let found = clef.refresh().await.ok()?;
    let model_id = found
        .artifact_digest
        .as_deref()
        .and_then(|digest| model_id_for(digest, None));
    Some(
        json!({"url": url, "model": found.model, "model_id": model_id,
                "artifact_digest": found.artifact_digest}),
    )
}

/// Read everything `doctor` reports.
pub async fn doctor(relay: &str, decide: &str, rehash: bool) -> Report {
    let host = host();
    let model = model_state(&model_path(), CLEF_FLASH_MODEL_ID, rehash);
    report(
        host,
        model,
        find_server(),
        serving(decide).await,
        crate::calibrate::last(&crate::home()),
        relay,
        relay_reachable(relay).await,
    )
}

/// Render a report as text.
#[must_use]
pub fn render(report: &Report) -> String {
    let mut text = format!(
        "{}\n\ncomputer   {} {} · {} CPUs · {} memory\ndevice     {}\nmodel      {}\nserver     {}\nrelay      {} ({})\ninbound    none needed; the pylon dials out\n",
        report.summary,
        report.host.os,
        report.host.arch,
        report.host.cpus,
        report
            .host
            .memory_gb
            .map_or_else(|| "unknown".into(), |g| format!("{g:.0} GB")),
        report.fit.note,
        if report.model.verified {
            format!(
                "{} (the pinned model{})",
                report.model.path.display(),
                file_note(&report.model)
            )
        } else if report.model.present {
            format!(
                "{} (not the pinned model's weights{})",
                report.model.path.display(),
                file_note(&report.model)
            )
        } else {
            "not downloaded".into()
        },
        report
            .server
            .as_ref()
            .map_or_else(|| "not installed".into(), |p| p.display().to_string()),
        report.relay,
        if report.relay_reachable {
            "reachable"
        } else {
            "unreachable"
        },
    );
    if let Some(check) = &report.selfcheck {
        text.push_str(&format!(
            "self-check {} at {}: {:.2}% of rounds pass, worst item {:.3}\n",
            if check.pass { "passed" } else { "failed" },
            check.at,
            check.round_pass * 100.0,
            check.max
        ));
    }
    for warning in &report.warnings {
        text.push_str(&format!("note: {warning}\n"));
    }
    for step in &report.next {
        text.push_str(&format!("next: {step}\n"));
    }
    text.trim_end().to_string()
}

/// `, file sha256 1234abcd…`, or nothing when the hash is unknown.
fn file_note(model: &ModelState) -> String {
    model
        .sha256
        .as_deref()
        .map(|sha| format!(", file sha256 {}…", &sha[..sha.len().min(8)]))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac(gb: f64) -> Host {
        Host {
            os: "macos".into(),
            arch: "aarch64".into(),
            cpus: 12,
            memory_gb: Some(gb),
            gpu: "apple".into(),
            gpu_name: Some("Apple M5 Max".into()),
            gpu_memory_gb: Some(gb),
            driver: None,
            disk_free_gb: Some(100.0),
        }
    }

    fn verified() -> ModelState {
        ModelState {
            path: PathBuf::from("/m/Clef-Flash-Q4_K_M.gguf"),
            present: true,
            verified: true,
            model_id: Some(CLEF_FLASH_MODEL_ID.into()),
            sha256: Some(CLEF_FLASH_SHA256.into()),
        }
    }

    #[test]
    fn parsers_read_real_output() {
        assert_eq!(
            parse_nvidia_smi("NVIDIA GeForce RTX 4080, 16376, 580.82.07\n"),
            Some((
                "NVIDIA GeForce RTX 4080".into(),
                16376.0,
                "580.82.07".into()
            ))
        );
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted\n/dev/disk3s5 1951000000 1900000000 45000000 98% /System/Volumes/Data\n";
        assert!((parse_df(df).unwrap() - 42.915).abs() < 0.01);
        let meminfo = "MemTotal:       65536000 kB\nMemFree:  1 kB\n";
        assert!((parse_meminfo(meminfo).unwrap() - 62.5).abs() < 1e-6);
    }

    #[test]
    fn device_fit_follows_memory() {
        assert_eq!(fit(&mac(128.0)).device, "metal");
        assert!(!fit(&mac(16.0)).useful);
        let nvidia = Host {
            gpu: "nvidia".into(),
            gpu_name: Some("RTX 4080".into()),
            gpu_memory_gb: Some(16.0),
            ..Host::default()
        };
        assert_eq!(fit(&nvidia).device, "cuda");
        assert_eq!(fit(&Host::default()).device, "cpu");
    }

    #[test]
    fn a_ready_report_needs_no_inbound_port_and_names_the_next_step() {
        let r = report(
            mac(128.0),
            verified(),
            Some(PathBuf::from("/b/psionic-openai-server")),
            None,
            None,
            "wss://relay.openagents.com",
            true,
        );
        assert!(r.ready, "{:?}", r.blocks);
        assert!(r.inbound_ports.is_empty());
        assert_eq!(
            r.next,
            vec!["openagents pylon start --allow-any".to_string()]
        );
    }

    #[test]
    fn every_block_comes_with_its_fix() {
        let r = report(
            Host {
                disk_free_gb: Some(1.0),
                ..mac(128.0)
            },
            ModelState {
                path: PathBuf::from("/m/x.gguf"),
                present: false,
                verified: false,
                model_id: None,
                sha256: None,
            },
            None,
            None,
            None,
            "wss://relay.openagents.com",
            false,
        );
        assert!(!r.ready);
        assert_eq!(r.blocks.len(), 4, "{:?}", r.blocks);
        assert!(r.next.contains(&"openagents pylon setup".to_string()));
        assert!(r.next.iter().any(|n| n.contains("psionic-openai-server")));
        assert!(r.blocks.iter().all(|b| !b.fix.is_empty()));
    }

    fn weights(meta: &[(&str, &str)]) -> Vec<u8> {
        crate::model_id::tests::gguf(
            meta,
            &[("output.weight", vec![2, 2], vec![1.0, 2.0, 3.0, 4.0])],
        )
    }

    #[test]
    fn model_state_remembers_a_verified_model_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.gguf");
        std::fs::write(&path, weights(&[])).unwrap();
        let id = crate::model_id::tensor_digest(&path).unwrap();
        let state = model_state(&path, &id, false);
        assert!(state.verified);
        assert_eq!(state.sha256, Some(sha256_file(&path).unwrap()));
        assert!(marker(&path).is_file());
        assert!(model_state(&path, &id, false).verified);
        assert!(!model_state(&path, "gguf-tensors:sha256:00", true).verified);
        assert!(!model_state(&dir.path().join("none.gguf"), &id, false).present);
        // A file that is not a GGUF has no model id and is never verified.
        std::fs::write(&path, b"weights plus").unwrap();
        let state = model_state(&path, &id, false);
        assert!(!state.verified && state.model_id.is_none() && state.sha256.is_some());
    }

    #[test]
    fn files_differing_only_in_a_metadata_key_are_the_same_model() {
        let dir = tempfile::tempdir().unwrap();
        let pinned = dir.path().join("pinned.gguf");
        let tagged = dir.path().join("tagged.gguf");
        std::fs::write(&pinned, weights(&[("general.name", "Clef-Flash")])).unwrap();
        std::fs::write(
            &tagged,
            weights(&[("general.name", "Clef-Flash"), ("general.source", "mirror")]),
        )
        .unwrap();
        let a = model_state(&pinned, "", false);
        let b = model_state(&tagged, "", false);
        assert_ne!(a.sha256, b.sha256);
        assert!(a.model_id.is_some());
        assert_eq!(a.model_id, b.model_id);
        // A server reporting the tagged file's hash resolves to that id.
        let served = format!("sha256:{}", b.sha256.clone().unwrap());
        assert_eq!(model_id_for(&served, Some(&tagged)), b.model_id);
        assert_eq!(model_id_for("sha256:00", Some(&tagged)), None);
        assert_eq!(
            model_id_for(&format!("sha256:{CLEF_FLASH_SHA256}"), None).as_deref(),
            Some(CLEF_FLASH_MODEL_ID)
        );
    }

    #[test]
    fn doctor_names_the_pinned_model_and_its_file_hash() {
        let r = report(
            mac(128.0),
            verified(),
            None,
            None,
            None,
            "wss://relay.openagents.com",
            true,
        );
        let text = render(&r);
        assert!(
            text.contains("(the pinned model, file sha256 36261166…)"),
            "{text}"
        );
    }

    #[tokio::test]
    async fn fetch_refuses_a_file_that_is_not_the_pinned_one() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let _ = socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\ncontent-length: 7\r\nconnection: close\r\n\r\nweights",
                    )
                    .await;
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.gguf");
        let url = format!("http://127.0.0.1:{port}/m.gguf");
        let good = hex(&Sha256::digest(b"weights"));
        assert!(fetch(&url, &path, 7, "00", |_| {}).await.is_err());
        assert!(!path.exists() && !path.with_extension("gguf.part").exists());
        fetch(&url, &path, 7, &good, |_| {}).await.unwrap();
        assert_eq!(sha256_file(&path).unwrap(), good);
    }

    #[tokio::test]
    async fn an_unreachable_relay_is_reported() {
        assert!(!relay_reachable("ws://127.0.0.1:1").await);
    }
}
