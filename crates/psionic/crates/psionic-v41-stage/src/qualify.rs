//! G1 v2 qualification harness (#12405): the served runtime against the
//! B200 oracle on the oracle's own tokens.
//!
//! ```text
//! psionic-v41-stage qualify --weights DIR --manifest D --oracle DIR --out DIR
//!     [--backend cuda|cpu] [--arithmetic A] [--gpus 0,1,2,3] [--cases ID,...]
//!     [--observe] [--control NAME] [--policy FILE] [--max-context N]
//!     [--step-secs S]
//! ```
//!
//! The harness is the group's front. It starts one `psionic-v41-stage`
//! worker per manifest stage, each on its own GPU and with `--record`, and
//! drives them over the front transport exactly as Pylon's front does: the
//! state between stages is W9's encoded message on each stage's listener.
//! For each oracle case it sends the oracle's prompt token IDs as one step
//! and then the oracle's generated tokens one step at a time, so the runtime
//! and the oracle see the same prefix at every position. The last stage
//! writes its full-vocabulary logits after every step; with `--observe`,
//! every stage writes its router and index selections.
//!
//! The runtime runs one graph per token for prompt and decode tokens alike,
//! so its replay of the fixed sequence and its forced decode compute the
//! same values. The harness checks that: it replays the fixed sequence as
//! one prompt and requires its last logits to equal the forced decode's
//! bit for bit. The forced logits are then compared with the oracle's
//! decode logits (decode against decode) and its replay logits (replay
//! against replay) under the policy in [`psionic_models::deepseek_v41_qualify`].
//!
//! Exact checks: every oracle tensor's SHA-256 against the dump's manifest,
//! the workers' shard hashes against the checkpoint pin (each worker refuses
//! other weights), one weights digest and runtime across the group, and
//! router and index selections' shapes and ranges.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    time::{Duration, Instant},
};

use psionic_models::deepseek_v41_qualify::{
    self as q, Decision, DecisionRule, Policy, PositionResult, Selection,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{Backend, Control, Refused, Sampling, load_manifest};

const EXPERTS: u32 = 384;
/// Layers whose router gaps the oracle recorded per forward: all 40.
const LAYERS: usize = 40;
/// The oracle's near-tie window and its per-forward list cap (oracle_dump.py).
const ORACLE_TIE_WINDOW: f64 = 1e-4;
const ORACLE_TIE_CAP: usize = 64;

fn refused(message: impl std::fmt::Display) -> Refused {
    Refused(message.to_string())
}

/// The harness's command line.
#[derive(Debug, Clone)]
pub struct QualifyOptions {
    pub weights: PathBuf,
    pub manifest: String,
    pub oracle: PathBuf,
    pub out: PathBuf,
    pub backend: Backend,
    pub arithmetic: Option<String>,
    pub gpus: Option<Vec<u32>>,
    pub cases: Option<Vec<String>>,
    pub observe: bool,
    pub control: Option<Control>,
    pub policy: Option<PathBuf>,
    pub max_context: usize,
    pub step_timeout: Duration,
}

impl QualifyOptions {
    /// Parses `qualify`'s arguments.
    ///
    /// # Errors
    ///
    /// Refuses unknown or missing arguments.
    pub fn parse(args: &[String]) -> Result<Self, Refused> {
        let mut values = BTreeMap::new();
        let mut observe = false;
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            if flag == "--observe" {
                observe = true;
            } else if [
                "--weights",
                "--manifest",
                "--oracle",
                "--out",
                "--backend",
                "--arithmetic",
                "--gpus",
                "--cases",
                "--control",
                "--policy",
                "--max-context",
                "--step-secs",
            ]
            .contains(&flag)
            {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| refused(format!("{flag} needs a value")))?;
                values.insert(flag, value.clone());
                i += 1;
            } else {
                return Err(refused(format!("unknown argument {flag}")));
            }
            i += 1;
        }
        let mut take = |flag: &str| {
            values
                .remove(flag)
                .ok_or_else(|| refused(format!("{flag} is required")))
        };
        let weights = PathBuf::from(take("--weights")?);
        let manifest = take("--manifest")?;
        let oracle = PathBuf::from(take("--oracle")?);
        let out = PathBuf::from(take("--out")?);
        let list = |v: Option<String>| -> Option<Vec<String>> {
            v.map(|v| v.split(',').map(str::to_owned).collect())
        };
        let gpus = list(values.remove("--gpus"))
            .map(|g| {
                g.iter()
                    .map(|x| x.parse().map_err(|_| refused("--gpus takes numbers")))
                    .collect::<Result<Vec<u32>, _>>()
            })
            .transpose()?;
        let number = |v: Option<String>, flag: &str, default: u64| -> Result<u64, Refused> {
            v.map_or(Ok(default), |v| {
                v.parse()
                    .map_err(|_| refused(format!("{flag} takes a number")))
            })
        };
        Ok(Self {
            weights,
            manifest,
            oracle,
            out,
            backend: values
                .remove("--backend")
                .map(|b| Backend::parse(&b))
                .transpose()?
                .unwrap_or(Backend::Cuda),
            arithmetic: values.remove("--arithmetic"),
            gpus,
            cases: list(values.remove("--cases")),
            observe,
            control: values
                .remove("--control")
                .map(|c| Control::parse(&c))
                .transpose()?,
            policy: values.remove("--policy").map(PathBuf::from),
            max_context: number(values.remove("--max-context"), "--max-context", 32_768)? as usize,
            step_timeout: Duration::from_secs(number(
                values.remove("--step-secs"),
                "--step-secs",
                3_600,
            )?),
        })
    }
}

// ---------------------------------------------------------------------------
// The oracle dump
// ---------------------------------------------------------------------------

/// The #12371 oracle dump: `manifest.json`, then per case `run.json` and
/// `tensors-rankN.safetensors`. Every read checks the tensor's SHA-256.
pub struct OracleDump {
    root: PathBuf,
    entries: BTreeMap<String, Value>,
    headers: BTreeMap<String, (u64, Value)>,
}

impl OracleDump {
    /// Opens a dump.
    ///
    /// # Errors
    ///
    /// A missing or malformed manifest.
    pub fn open(root: &Path) -> Result<Self, Refused> {
        let text = std::fs::read(root.join("manifest.json"))
            .map_err(|e| refused(format!("the oracle manifest: {e}")))?;
        let manifest: Value = serde_json::from_slice(&text)
            .map_err(|e| refused(format!("the oracle manifest: {e}")))?;
        let entries = manifest["tensors"]
            .as_array()
            .ok_or_else(|| refused("the oracle manifest lists no tensors"))?
            .iter()
            .filter_map(|e| Some((e["name"].as_str()?.to_owned(), e.clone())))
            .collect();
        Ok(Self {
            root: root.to_owned(),
            entries,
            headers: BTreeMap::new(),
        })
    }

    /// Whether the dump has a tensor.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// A case's `run.json`.
    ///
    /// # Errors
    ///
    /// A missing or malformed file.
    pub fn run(&self, case: &str) -> Result<Value, Refused> {
        let path = self.root.join(case).join("run.json");
        let bytes =
            std::fs::read(&path).map_err(|e| refused(format!("{}: {e}", path.display())))?;
        serde_json::from_slice(&bytes).map_err(|e| refused(format!("{}: {e}", path.display())))
    }

    fn raw(&mut self, name: &str) -> Result<(Vec<u8>, String), Refused> {
        let entry = self
            .entries
            .get(name)
            .ok_or_else(|| refused(format!("the oracle has no tensor {name}")))?
            .clone();
        let file = entry["file"].as_str().unwrap_or_default().to_owned();
        let mut f =
            File::open(self.root.join(&file)).map_err(|e| refused(format!("{file}: {e}")))?;
        if !self.headers.contains_key(&file) {
            let mut size = [0; 8];
            f.read_exact(&mut size)
                .map_err(|e| refused(format!("{file}: {e}")))?;
            let size = u64::from_le_bytes(size);
            if size > 256 << 20 {
                return Err(refused(format!("{file}: header too large")));
            }
            let mut header = vec![0; size as usize];
            f.read_exact(&mut header)
                .map_err(|e| refused(format!("{file}: {e}")))?;
            let header: Value =
                serde_json::from_slice(&header).map_err(|e| refused(format!("{file}: {e}")))?;
            self.headers.insert(file.clone(), (size, header));
        }
        let (size, header) = &self.headers[&file];
        let offsets = &header[name]["data_offsets"];
        let (begin, end) = (
            offsets[0]
                .as_u64()
                .ok_or_else(|| refused(format!("{name}: offsets")))?,
            offsets[1]
                .as_u64()
                .ok_or_else(|| refused(format!("{name}: offsets")))?,
        );
        let mut bytes = vec![0; (end - begin) as usize];
        f.seek(SeekFrom::Start(8 + size + begin))
            .and_then(|_| f.read_exact(&mut bytes))
            .map_err(|e| refused(format!("{name}: {e}")))?;
        if hex::encode(Sha256::digest(&bytes)) != entry["sha256"].as_str().unwrap_or_default() {
            return Err(refused(format!(
                "{name} doesn't match its SHA-256 in the manifest"
            )));
        }
        Ok((
            bytes,
            entry["dtype"].as_str().unwrap_or_default().to_owned(),
        ))
    }

    /// A float tensor as `f32` (`float32` or `bfloat16`).
    ///
    /// # Errors
    ///
    /// A missing tensor, a hash mismatch, or another dtype.
    pub fn floats(&mut self, name: &str) -> Result<Vec<f32>, Refused> {
        let (bytes, dtype) = self.raw(name)?;
        match dtype.as_str() {
            "float32" => Ok(bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()),
            "bfloat16" => Ok(bytes
                .chunks_exact(2)
                .map(|c| f32::from_bits(u32::from(u16::from_le_bytes([c[0], c[1]])) << 16))
                .collect()),
            other => Err(refused(format!("{name} is {other}, not a float tensor"))),
        }
    }

    /// An integer tensor (`int32` or `int64`).
    ///
    /// # Errors
    ///
    /// A missing tensor, a hash mismatch, or another dtype.
    pub fn integers(&mut self, name: &str) -> Result<Vec<i64>, Refused> {
        let (bytes, dtype) = self.raw(name)?;
        match dtype.as_str() {
            "int32" => Ok(bytes
                .chunks_exact(4)
                .map(|c| i64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])))
                .collect()),
            "int64" => Ok(bytes
                .chunks_exact(8)
                .map(|c| i64::from_le_bytes(c.try_into().unwrap_or([0; 8])))
                .collect()),
            other => Err(refused(format!("{name} is {other}, not integers"))),
        }
    }
}

// ---------------------------------------------------------------------------
// The group
// ---------------------------------------------------------------------------

/// The workers and the front's connections to them.
struct Group {
    children: Vec<Child>,
    writers: Vec<TcpStream>,
    listens: Vec<Option<String>>,
    events: Receiver<(usize, Value)>,
    generation: u64,
    timeout: Duration,
    weights: String,
    runtime: String,
}

impl Drop for Group {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Group {
    fn start(opts: &QualifyOptions, record: &Path) -> Result<Self, Refused> {
        let manifest = load_manifest(&opts.manifest, &opts.weights)?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| refused(e.to_string()))?;
        let front = listener
            .local_addr()
            .map_err(|e| refused(e.to_string()))?
            .to_string();
        let exe = std::env::current_exe().map_err(|e| refused(e.to_string()))?;
        let token = hex::encode(Sha256::digest(format!("{front}{:?}", Instant::now())));
        let generation = 1;
        let mut children = Vec::new();
        for entry in &manifest.stages {
            let i = usize::from(entry.stage);
            let gpu = opts
                .gpus
                .as_ref()
                .map_or(entry.gpu, |g| g.get(i).copied().unwrap_or(entry.gpu));
            let mut args = vec![
                "--weights".into(),
                opts.weights.display().to_string(),
                "--layers".into(),
                format!("{}-{}", entry.first_layer, entry.end_layer),
                "--gpu".into(),
                gpu.to_string(),
                "--manifest".into(),
                opts.manifest.clone(),
                "--stage".into(),
                format!("s{i}"),
                "--generation".into(),
                generation.to_string(),
                "--front".into(),
                front.clone(),
                "--token".into(),
                token.clone(),
                "--backend".into(),
                match opts.backend {
                    Backend::Cpu => "cpu".into(),
                    Backend::Cuda => "cuda".into(),
                },
                "--max-context".into(),
                opts.max_context.to_string(),
                "--record".into(),
                record.display().to_string(),
                "--events".into(),
                record
                    .join(format!("s{i}.events.jsonl"))
                    .display()
                    .to_string(),
            ];
            if entry.embeddings {
                args.push("--embeddings".into());
            }
            if entry.head {
                args.push("--head".into());
            }
            if let Some(a) = &opts.arithmetic {
                args.extend(["--arithmetic".into(), a.clone()]);
            }
            if opts.observe {
                args.push("--observe".into());
            }
            if let Some(c) = opts.control {
                args.extend(["--control".into(), c.name().into()]);
            }
            let log = File::create(record.join(format!("s{i}.log")))
                .map_err(|e| refused(e.to_string()))?;
            let mut command = Command::new(&exe);
            command
                .args(&args)
                .stdout(Stdio::null())
                .stderr(Stdio::from(log));
            if opts.backend == Backend::Cuda {
                command.env("CUDA_VISIBLE_DEVICES", gpu.to_string());
            }
            children.push(
                command
                    .spawn()
                    .map_err(|e| refused(format!("stage {i}: {e}")))?,
            );
        }
        let stages = manifest.stages.len();
        let (send, events) = mpsc::channel::<(usize, Value)>();
        let mut writers: Vec<Option<TcpStream>> = (0..stages).map(|_| None).collect();
        let mut listens = vec![None; stages];
        let mut identity = BTreeSet::new();
        listener
            .set_nonblocking(true)
            .map_err(|e| refused(e.to_string()))?;
        // Loading the model can take most of an hour on cold disks.
        let deadline = Instant::now() + Duration::from_secs(7_200);
        let mut group = Self {
            children,
            writers: Vec::new(),
            listens: Vec::new(),
            events,
            generation,
            timeout: opts.step_timeout,
            weights: String::new(),
            runtime: String::new(),
        };
        while writers.iter().any(Option::is_none) {
            if Instant::now() > deadline {
                return Err(refused("the stages didn't all say hello within two hours"));
            }
            for (i, child) in group.children.iter_mut().enumerate() {
                if let Ok(Some(status)) = child.try_wait() {
                    return Err(refused(format!(
                        "stage {i} exited ({status}) before it was ready; see {}",
                        record.join(format!("s{i}.log")).display()
                    )));
                }
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    stream
                        .set_nonblocking(false)
                        .map_err(|e| refused(e.to_string()))?;
                    let mut reader =
                        BufReader::new(stream.try_clone().map_err(|e| refused(e.to_string()))?);
                    let mut line = String::new();
                    reader
                        .read_line(&mut line)
                        .map_err(|e| refused(e.to_string()))?;
                    let hello: Value = serde_json::from_str(&line)
                        .map_err(|e| refused(format!("a stage's hello: {e}")))?;
                    if hello["token"].as_str() != Some(&token) || hello["kind"] != "hello" {
                        return Err(refused("a connection that isn't one of the stages"));
                    }
                    let i: usize = hello["stage"]
                        .as_str()
                        .and_then(|s| s.strip_prefix('s'))
                        .and_then(|s| s.parse().ok())
                        .filter(|i| *i < stages)
                        .ok_or_else(|| refused("a hello from an unknown stage"))?;
                    identity.insert((
                        hello["weights"].as_str().unwrap_or_default().to_owned(),
                        hello["runtime"].as_str().unwrap_or_default().to_owned(),
                    ));
                    listens[i] = hello["listen"].as_str().map(str::to_owned);
                    let send = send.clone();
                    std::thread::spawn(move || {
                        for line in reader.lines() {
                            let Ok(line) = line else { break };
                            if let Ok(v) = serde_json::from_str::<Value>(&line)
                                && send.send((i, v)).is_err()
                            {
                                break;
                            }
                        }
                    });
                    writers[i] = Some(stream);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(200));
                }
                Err(e) => return Err(refused(e.to_string())),
            }
        }
        if identity.len() != 1 {
            return Err(refused(format!(
                "the stages announce different weights or runtimes: {identity:?}"
            )));
        }
        let (weights, runtime) = identity.into_iter().next().unwrap_or_default();
        group.weights = weights;
        group.runtime = runtime;
        group.writers = writers.into_iter().flatten().collect();
        group.listens = listens;
        Ok(group)
    }

    /// Sends one step to every stage and waits for every stage's ack;
    /// returns the last stage's.
    fn step(
        &mut self,
        request: &str,
        step: u64,
        prompt_ids: Option<&[u32]>,
        token: Option<u32>,
    ) -> Result<Value, Refused> {
        let stages = self.writers.len();
        for i in 0..stages {
            let mut message = json!({
                "kind": "step",
                "generation": self.generation,
                "request_id": request,
                "attempt": 0,
                "stage": i,
                "step": step,
                "sampling": Sampling::GREEDY,
                "final": false,
            });
            if let Some(next) = self.listens.get(i + 1).cloned().flatten() {
                message["next"] = json!(next);
            }
            if i == 0 {
                if let Some(ids) = prompt_ids {
                    message["prompt_ids"] = json!(ids);
                }
                if let Some(token) = token {
                    message["token"] = json!(token);
                }
            }
            writeln!(self.writers[i], "{message}")
                .map_err(|e| refused(format!("stage {i}: {e}")))?;
        }
        let mut acked = BTreeSet::new();
        let mut last = Value::Null;
        let deadline = Instant::now() + self.timeout;
        while acked.len() < stages {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left.max(Duration::from_millis(1))) {
                Ok((i, v)) => {
                    if v["kind"] == "ack"
                        && v["request_id"].as_str() == Some(request)
                        && v["step"].as_u64() == Some(step)
                    {
                        acked.insert(i);
                        if i + 1 == stages {
                            last = v;
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(refused(format!(
                        "step {step} of {request}: stages {:?} didn't answer in {:?}",
                        (0..stages)
                            .filter(|i| !acked.contains(i))
                            .collect::<Vec<_>>(),
                        self.timeout
                    )));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(refused("the stages closed their connections"));
                }
            }
        }
        Ok(last)
    }
}

// ---------------------------------------------------------------------------
// Running a case
// ---------------------------------------------------------------------------

/// What the runtime produced for one case.
struct Recorded {
    /// Label (`last`, `g1`, ...) to the logits file.
    logits: BTreeMap<String, PathBuf>,
    /// The replayed fixed sequence's last logits.
    replay_last: Option<PathBuf>,
    /// Per step, the files the stages wrote.
    records: Vec<Vec<String>>,
    seconds: f64,
}

fn record_files(ack: &Value) -> Vec<String> {
    ack["records"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn logits_file(dir: &Path, files: &[String]) -> Option<PathBuf> {
    files
        .iter()
        .find(|f| f.ends_with(".logits.f32"))
        .map(|f| dir.join(f))
}

fn run_case(
    group: &mut Group,
    record: &Path,
    case: &str,
    prompt: &[u32],
    generated: &[u32],
    tag: &str,
) -> Result<Recorded, Refused> {
    let started = Instant::now();
    let request = format!("{case}{tag}");
    let mut logits = BTreeMap::new();
    let mut records = Vec::new();
    let ack = group.step(&request, 0, Some(prompt), None)?;
    let mut files = record_files(&ack);
    logits.insert(
        "last".to_owned(),
        logits_file(record, &files).ok_or_else(|| refused("no logits for the prompt"))?,
    );
    records.push(std::mem::take(&mut files));
    for (k, token) in generated.iter().enumerate() {
        let step = k as u64 + 1;
        let ack = group.step(&request, step, None, Some(*token))?;
        let files = record_files(&ack);
        logits.insert(
            format!("g{step}"),
            logits_file(record, &files).ok_or_else(|| refused(format!("no logits at g{step}")))?,
        );
        records.push(files);
    }
    let mut fixed = prompt.to_vec();
    fixed.extend_from_slice(generated);
    let ack = group.step(&format!("{request}-replay"), 0, Some(&fixed), None)?;
    let replay_last = logits_file(record, &record_files(&ack));
    Ok(Recorded {
        logits,
        replay_last,
        records,
        seconds: started.elapsed().as_secs_f64(),
    })
}

fn read_f32(path: &Path) -> Result<Vec<f32>, Refused> {
    let bytes = std::fs::read(path).map_err(|e| refused(format!("{}: {e}", path.display())))?;
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn read_u32(path: &Path) -> Result<Vec<u32>, Refused> {
    let bytes = std::fs::read(path).map_err(|e| refused(format!("{}: {e}", path.display())))?;
    Ok(bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn ids(value: &Value) -> Vec<u32> {
    value
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| t.as_u64().map(|t| t as u32))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Compares a case's recorded logits with the oracle in one mode.
fn distribution_case(
    policy: &Policy,
    oracle: &mut OracleDump,
    case: &str,
    mode: &str,
    rec: &Recorded,
    next_tokens: &BTreeMap<String, u32>,
) -> Result<q::CaseResult, Refused> {
    let mut positions: Vec<PositionResult> = Vec::new();
    let mut missing = Vec::new();
    for (label, token) in next_tokens {
        let name = format!("{case}.{mode}.{label}.logits");
        if !oracle.has(&name) {
            continue;
        }
        let Some(path) = rec.logits.get(label) else {
            missing.push(label.clone());
            continue;
        };
        let reference = oracle.floats(&name)?;
        let candidate = read_f32(path)?;
        match q::compare_logits(&reference, &candidate, *token) {
            Ok(metrics) => positions.push(q::judge_position(policy, label, metrics)),
            Err(error) => missing.push(format!("{label} ({error})")),
        }
    }
    Ok(q::judge_case(policy, case, mode, positions, missing))
}

/// Router selections against the oracle's per-token router IDs (decode mode),
/// with the oracle's recorded near-tie gaps.
fn router_case(
    policy: &Policy,
    oracle: &mut OracleDump,
    run: &Value,
    case: &str,
    record: &Path,
    rec: &Recorded,
) -> Result<Value, Refused> {
    let prompt = run["prompt_tokens"].as_u64().unwrap_or_default() as usize;
    // Oracle rows: the prompt's rows (step 0), then one row per decode step.
    let mut row_step = Vec::new();
    for r in 0..prompt {
        row_step.push((0_u64, r));
    }
    for k in 1..rec.records.len() {
        row_step.push((k as u64, 0));
    }
    let mut ties: BTreeMap<(u64, usize, usize), f64> = BTreeMap::new();
    let mut tie_counts: BTreeMap<(u64, usize), usize> = BTreeMap::new();
    for t in run["near_ties"].as_array().into_iter().flatten() {
        if t["mode"] == "decode" && t["kind"] == "gate" {
            let key = (
                t["step"].as_u64().unwrap_or_default(),
                t["layer"].as_u64().unwrap_or_default() as usize,
            );
            *tie_counts.entry(key).or_default() += 1;
            ties.insert(
                (key.0, key.1, t["row"].as_u64().unwrap_or_default() as usize),
                t["margin"].as_f64().unwrap_or_default(),
            );
        }
    }
    let rule = policy.router;
    let mut layers = Vec::new();
    let (mut same, mut substituted, mut failed, mut unresolved) = (0usize, 0, 0, 0);
    let mut examples = Vec::new();
    for layer in 0..LAYERS {
        let name = format!("{case}.decode.router_ids.layers.{layer}");
        if !oracle.has(&name) {
            continue;
        }
        let reference = oracle.integers(&name)?;
        let mut candidate = Vec::new();
        for files in &rec.records {
            let file = files
                .iter()
                .find(|f| f.ends_with(&format!(".L{layer}.ExpertIds.u32")))
                .ok_or_else(|| refused(format!("no router observation for layer {layer}")))?;
            candidate.extend(read_u32(&record.join(file))?);
        }
        let top_k = reference.len() / row_step.len().max(1);
        if reference.len() != candidate.len()
            || reference.len() != row_step.len() * top_k
            || top_k == 0
        {
            return Err(refused(format!(
                "layer {layer}: {} oracle and {} runtime router IDs for {} rows",
                reference.len(),
                candidate.len(),
                row_step.len()
            )));
        }
        let mut layer_counts = [0usize; 4];
        for (row, (step, r)) in row_step.iter().enumerate() {
            let o: Vec<u32> = reference[row * top_k..(row + 1) * top_k]
                .iter()
                .map(|x| *x as u32)
                .collect();
            let c = &candidate[row * top_k..(row + 1) * top_k];
            let gap = ties.get(&(*step, layer, *r)).copied().or_else(|| {
                // Unlisted: the gap is at least the oracle's tie window,
                // unless its list for that forward was cut off.
                let complete =
                    tie_counts.get(&(*step, layer)).copied().unwrap_or_default() < ORACLE_TIE_CAP;
                let stable = rule.epsilon.is_some_and(|e| 2.0 * e < ORACLE_TIE_WINDOW);
                (complete && stable).then_some(ORACLE_TIE_WINDOW)
            });
            let decision = rule.check(&Selection {
                oracle: &o,
                candidate: c,
                boundary_gap: gap,
                oracle_scores: None,
                universe: EXPERTS,
            });
            let slot = match &decision {
                Decision::Same => 0,
                Decision::Substituted { .. } => 1,
                Decision::Failed { .. } => 2,
                Decision::Unresolved { .. } => 3,
            };
            layer_counts[slot] += 1;
            if slot >= 2 && examples.len() < 32 {
                examples.push(
                    json!({"layer": layer, "row": row, "oracle": o, "runtime": c,
                    "gap": gap, "decision": decision}),
                );
            }
        }
        same += layer_counts[0];
        substituted += layer_counts[1];
        failed += layer_counts[2];
        unresolved += layer_counts[3];
        layers.push(
            json!({"layer": layer, "same": layer_counts[0], "substituted": layer_counts[1],
            "failed": layer_counts[2], "unresolved": layer_counts[3]}),
        );
    }
    let total = same + substituted + failed + unresolved;
    let fraction_ok = rule
        .max_substituted_fraction
        .map(|f| (substituted as f64) <= f * total.max(1) as f64);
    Ok(json!({
        "rows": total,
        "same": same,
        "substituted": substituted,
        "failed": failed,
        "unresolved": unresolved,
        "substituted_fraction_within_limit": fraction_ok,
        "passed": total > 0 && failed == 0 && unresolved == 0 && fraction_ok == Some(true),
        "layers": layers,
        "examples": examples,
    }))
}

fn sha256_file(path: &Path) -> String {
    std::fs::read(path)
        .map(|b| hex::encode(Sha256::digest(b)))
        .unwrap_or_default()
}

/// A policy with every limit pending, for runs before the policy is frozen.
#[must_use]
pub fn pending_policy() -> Policy {
    let rule = DecisionRule {
        epsilon: None,
        max_substitutions: 1,
        max_substituted_fraction: None,
    };
    Policy {
        schema: q::POLICY_SCHEMA.into(),
        version: "pending".into(),
        distribution: q::DistributionLimits::default(),
        top1: rule,
        router: rule,
        index: rule,
    }
}

/// Runs the harness and writes `report.json` under `--out`.
///
/// # Errors
///
/// Any refusal of the oracle, the stages, or the files.
pub fn run(opts: &QualifyOptions) -> Result<Value, Refused> {
    let record = opts.out.join("record");
    std::fs::create_dir_all(&record).map_err(|e| refused(e.to_string()))?;
    let (policy, policy_sha) = match &opts.policy {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| refused(e.to_string()))?;
            (
                Policy::parse(&text).map_err(|e| refused(e.to_string()))?,
                hex::encode(Sha256::digest(text.as_bytes())),
            )
        }
        None => (pending_policy(), String::new()),
    };
    let mut oracle = OracleDump::open(&opts.oracle)?;
    let all = [
        "p1-chat-thinking",
        "p2-code",
        "e-window-127",
        "e-window-128",
        "e-window-129",
        "e-engram",
        "p3-2k",
        "p4-20k",
    ];
    let cases: Vec<String> = opts
        .cases
        .clone()
        .unwrap_or_else(|| all.iter().map(|c| (*c).to_owned()).collect());
    let mut group = Group::start(opts, &record)?;
    let mut reports = Vec::new();
    for case in &cases {
        let run = oracle.run(case)?;
        let prompt = ids(&run["prompt_token_ids"]);
        let generated = ids(&run["generated_ids"]);
        if prompt.is_empty() || generated.is_empty() {
            return Err(refused(format!("{case}: the oracle run has no tokens")));
        }
        // The stale-cache control needs an earlier request whose cache leaks.
        if opts.control == Some(Control::StaleCache) {
            group.step(&format!("{case}-warm"), 0, Some(&prompt), None)?;
        }
        let rec = run_case(&mut group, &record, case, &prompt, &generated, "")?;
        // The token each recorded position continues with.
        let mut next_tokens = BTreeMap::new();
        next_tokens.insert("last".to_owned(), generated[0]);
        for k in 1..=generated.len() {
            let next = generated
                .get(k)
                .copied()
                .or_else(|| run["extra_step_argmax"].as_u64().map(|t| t as u32))
                .unwrap_or(generated[k - 1]);
            next_tokens.insert(format!("g{k}"), next);
        }
        let last_label = format!("g{}", generated.len());
        let replay_equal = match (&rec.replay_last, rec.logits.get(&last_label)) {
            (Some(a), Some(b)) => Some(std::fs::read(a).ok() == std::fs::read(b).ok()),
            _ => None,
        };
        let mut modes = Vec::new();
        for mode in ["decode", "replay"] {
            modes.push(distribution_case(
                &policy,
                &mut oracle,
                case,
                mode,
                &rec,
                &next_tokens,
            )?);
        }
        let router = if opts.observe {
            Some(router_case(
                &policy,
                &mut oracle,
                &run,
                case,
                &record,
                &rec,
            )?)
        } else {
            None
        };
        reports.push(json!({
            "case": case,
            "prompt_tokens": prompt.len(),
            "generated_tokens": generated.len(),
            "seconds": rec.seconds,
            "replay_equals_forced_decode": replay_equal,
            "modes": modes,
            "router": router,
        }));
        let partial = json!({"cases": reports});
        let _ = std::fs::write(
            opts.out.join("report.partial.json"),
            serde_json::to_vec_pretty(&partial).unwrap_or_default(),
        );
    }
    let passed = reports.iter().all(|r| {
        r["replay_equals_forced_decode"] == json!(true)
            && r["modes"]
                .as_array()
                .is_some_and(|m| m.iter().all(|c| c["passed"] == json!(true)))
            && (r["router"].is_null() || r["router"]["passed"] == json!(true))
    });
    let exe = std::env::current_exe().map_err(|e| refused(e.to_string()))?;
    let report = json!({
        "schema": "psionic.deepseek_v41.g1_v2_report.v1",
        "issue": 12405,
        "policy_version": policy.version,
        "policy_sha256": policy_sha,
        "runtime": group.runtime,
        "weights": group.weights,
        "manifest": opts.manifest,
        "binary_sha256": sha256_file(&exe),
        "oracle_manifest_sha256": sha256_file(&opts.oracle.join("manifest.json")),
        "backend": match opts.backend { Backend::Cpu => "cpu", Backend::Cuda => "cuda" },
        "arithmetic": opts.arithmetic,
        "control": opts.control.map(Control::name),
        "observe": opts.observe,
        "schedule": "one prompt step, then one step per oracle token; the runtime runs one graph per token",
        "cases": reports,
        "passed": passed,
    });
    std::fs::write(
        opts.out.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(e.to_string()))?;
    Ok(report)
}
