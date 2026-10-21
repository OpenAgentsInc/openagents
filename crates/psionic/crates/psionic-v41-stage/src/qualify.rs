//! G1 v2 qualification harness (#12405): the served runtime against the
//! B200 oracle on the oracle's own tokens.
//!
//! ```text
//! psionic-v41-stage qualify --weights DIR --manifest D --oracle DIR --out DIR
//!     [--backend cuda|cpu] [--arithmetic A] [--gpus 0,1,2,3] [--cases ID,...]
//!     [--observe] [--control NAME] [--policy FILE] [--max-context N]
//!     [--max-step-tokens N] [--prefill-chunk N]
//!     [--step-secs S] (--worker-build SHA256 | --development)
//! ```
//!
//! `--worker-build` is the qualified bundle's hash, the SHA-256 of its
//! `BUILD.json` (#12410). Every stage must announce that verified bundle,
//! or the run refuses. `--development` runs without a pin, and the report
//! says so.
//!
//! The harness is the group's front. It starts one `psionic-v41-stage`
//! worker per manifest stage, each on its own GPU and with `--record`, and
//! drives them over the front transport exactly as Pylon's front does: the
//! state between stages is W9's encoded message on each stage's listener.
//! For each oracle case it sends the complete prompt, optionally in bounded
//! serial chunks, then the oracle's generated tokens one step at a time. The runtime
//! and the oracle see the same prefix at every position. The last stage
//! writes its full-vocabulary logits after each output step; with `--observe`,
//! every stage writes its router and index selections.
//!
//! The runtime runs one graph per token for prompt and decode tokens alike,
//! so its replay of the fixed sequence and its forced decode compute the
//! same values. The harness checks that: it replays the fixed sequence as
//! a prompt under the same chunk schedule and requires its last logits to equal the forced decode's
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
    pub max_step_tokens: Option<usize>,
    pub prefill_chunk: Option<usize>,
    pub step_timeout: Duration,
    /// The bundle hash every stage must announce (#12410); `None` only with
    /// `development`.
    pub worker_build: Option<String>,
    /// A run without a build pin, labelled as one in the report.
    pub development: bool,
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
        let mut development = false;
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            if flag == "--observe" {
                observe = true;
            } else if flag == "--development" {
                development = true;
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
                "--max-step-tokens",
                "--prefill-chunk",
                "--step-secs",
                "--worker-build",
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
        let worker_build = values.remove("--worker-build");
        match (&worker_build, development) {
            (None, false) => {
                return Err(refused(
                    "pin the bundle with --worker-build SHA256 (its BUILD.json's SHA-256), or run with --development",
                ));
            }
            (Some(_), true) => {
                return Err(refused(
                    "--worker-build and --development don't go together",
                ));
            }
            (Some(pin), false)
                if pin.len() != 64 || !pin.bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                return Err(refused("--worker-build takes a SHA-256 in hex"));
            }
            _ => {}
        }
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
        let options = Self {
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
            max_step_tokens: values
                .remove("--max-step-tokens")
                .map(|v| {
                    v.parse()
                        .map_err(|_| refused("--max-step-tokens takes a number"))
                })
                .transpose()?,
            prefill_chunk: values
                .remove("--prefill-chunk")
                .map(|v| {
                    v.parse()
                        .map_err(|_| refused("--prefill-chunk takes a number"))
                })
                .transpose()?,
            step_timeout: Duration::from_secs(number(
                values.remove("--step-secs"),
                "--step-secs",
                3_600,
            )?),
            worker_build: worker_build.map(|p| p.to_ascii_lowercase()),
            development,
        };
        let step = options
            .max_step_tokens
            .or(options.prefill_chunk)
            .unwrap_or(options.max_context);
        if options.max_context < 2
            || step == 0
            || step > options.max_context
            || options.prefill_chunk.is_some_and(|n| n == 0 || n > step)
        {
            return Err(refused(
                "context must be at least two positions, and chunk and step capacities must be positive and fit the context",
            ));
        }
        Ok(options)
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
    wire_step: u64,
    limits: Vec<Value>,
    children: Vec<Child>,
    writers: Vec<TcpStream>,
    listens: Vec<Option<String>>,
    events: Receiver<(usize, Value)>,
    generation: u64,
    timeout: Duration,
    weights: String,
    runtime: String,
    /// The numerical profile, verified bundle hash, and executable SHA-256
    /// every stage announced (#12407, #12410), so the report names what
    /// ran, not what was asked for.
    profile: String,
    build: String,
    executable: String,
    /// Where the stages write their event logs.
    record: PathBuf,
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
            if let Some(n) = opts.max_step_tokens {
                args.extend(["--max-step-tokens".into(), n.to_string()]);
            }
            if let Some(n) = opts.prefill_chunk {
                args.extend(["--prefill-chunk".into(), n.to_string()]);
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
            // The stage narrows CUDA_VISIBLE_DEVICES to `--gpu` itself,
            // counting within any mask this process inherited (#12407).
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
            wire_step: 0,
            limits: vec![Value::Null; stages],
            children,
            writers: Vec::new(),
            listens: Vec::new(),
            events,
            generation,
            timeout: opts.step_timeout,
            weights: String::new(),
            runtime: String::new(),
            profile: String::new(),
            build: String::new(),
            executable: String::new(),
            record: record.to_owned(),
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
                        hello["profile"].as_str().unwrap_or_default().to_owned(),
                        hello["build"]["bundle_sha256"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                        hello["build"]["sha256"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                    ));
                    listens[i] = hello["listen"].as_str().map(str::to_owned);
                    group.limits[i] = hello["limits"].clone();
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
                "the stages announce different weights, runtimes, profiles, or builds: {identity:?}"
            )));
        }
        let (weights, runtime, profile, build, executable) =
            identity.into_iter().next().unwrap_or_default();
        if let Some(pin) = &opts.worker_build
            && build != *pin
        {
            return Err(refused(format!(
                "the stages run bundle {}, not the pinned bundle {pin}",
                if build.is_empty() {
                    "(none verified)"
                } else {
                    &build
                }
            )));
        }
        group.weights = weights;
        group.runtime = runtime;
        group.profile = profile;
        group.build = build;
        group.executable = executable;
        group.writers = writers.into_iter().flatten().collect();
        group.listens = listens;
        Ok(group)
    }

    /// Sends one step to every stage and waits for every stage's ack;
    /// returns the last stage's, with every stage's recorded files.
    fn step(
        &mut self,
        request: &str,
        step: u64,
        prompt_ids: Option<&[u32]>,
        token: Option<u32>,
    ) -> Result<Value, Refused> {
        if step == 0 {
            self.wire_step = 0;
        }
        let mut records = Vec::new();
        loop {
            let wire_step = self.wire_step;
            let mut phase = (false, false);
            let mut last = Value::Null;
            // Use the same serial schedule as the serving front: stage 0
            // chooses the chunk; downstream stages receive its phase.
            for i in 0..self.writers.len() {
                let mut message = json!({
                    "kind": "step", "generation": self.generation,
                    "request_id": request, "attempt": 0, "stage": i,
                    "step": wire_step, "output_step": step,
                    "prefill": phase.0, "prefill_pending": phase.1,
                    "sampling": Sampling::GREEDY, "final": false,
                });
                if let Some(next) = self.listens.get(i + 1).cloned().flatten() {
                    message["next"] = json!(next);
                }
                if i == 0 {
                    if wire_step == 0
                        && let Some(ids) = prompt_ids
                    {
                        message["prompt_ids"] = json!(ids);
                    }
                    if let Some(token) = token {
                        message["token"] = json!(token);
                    }
                }
                writeln!(self.writers[i], "{message}")
                    .map_err(|e| refused(format!("stage {i}: {e}")))?;
                let deadline = Instant::now() + self.timeout;
                loop {
                    if let Some(reason) = self.refusal(request, wire_step) {
                        return Err(refused(format!("step {wire_step} of {request}: {reason}")));
                    }
                    let left = deadline.saturating_duration_since(Instant::now());
                    match self.events.recv_timeout(
                        left.min(Duration::from_secs(5))
                            .max(Duration::from_millis(1)),
                    ) {
                        Ok((stage, v))
                            if stage == i
                                && v["kind"] == "ack"
                                && v["request_id"].as_str() == Some(request)
                                && v["step"].as_u64() == Some(wire_step) =>
                        {
                            phase = (
                                v["prefill"].as_bool().unwrap_or(false),
                                v["prefill_pending"].as_bool().unwrap_or(false),
                            );
                            records.extend(record_files(&v));
                            last = v;
                            break;
                        }
                        Ok(_) => {}
                        Err(RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                        Err(e) => {
                            return Err(refused(format!(
                                "step {wire_step} of {request}, stage {i}: {e}"
                            )));
                        }
                    }
                }
            }
            self.wire_step += 1;
            if !phase.1 {
                last["records"] = json!(records);
                return Ok(last);
            }
            if !phase.0 || last.get("token").is_some() {
                return Err(refused("an intermediate prefill chunk produced output"));
            }
        }
    }
}

impl Group {
    /// The reason a stage refused this step, from its event log.
    fn refusal(&self, request: &str, step: u64) -> Option<String> {
        (0..self.writers.len()).find_map(|i| {
            let path = self.record.join(format!("s{i}.events.jsonl"));
            let text = std::fs::read_to_string(path).ok()?;
            text.lines().find_map(|line| {
                let v: Value = serde_json::from_str(line).ok()?;
                (v["event"] == "refused"
                    && v["request"].as_str() == Some(request)
                    && v["step"].as_u64() == Some(step))
                .then(|| format!("stage {i} refused: {}", v["reason"].as_str().unwrap_or("")))
            })
        })
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
    let mut over = Vec::new();
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
            let matching: Vec<_> = files
                .iter()
                .filter(|f| f.ends_with(&format!(".L{layer}.ExpertIds.u32")))
                .collect();
            if matching.is_empty() {
                return Err(refused(format!("no router observation for layer {layer}")));
            }
            for file in matching {
                candidate.extend(read_u32(&record.join(file))?);
            }
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
        let rows = layer_counts.iter().sum::<usize>().max(1);
        let mismatch = (rows - layer_counts[0]) as f64 / rows as f64;
        let limit = policy
            .router_layer_mismatch
            .as_ref()
            .and_then(|l| l.get(layer).copied());
        if limit.is_some_and(|l| mismatch > l) {
            over.push(json!({"layer": layer, "mismatch": mismatch, "limit": limit}));
        }
        layers.push(
            json!({"layer": layer, "same": layer_counts[0], "substituted": layer_counts[1],
            "failed": layer_counts[2], "unresolved": layer_counts[3], "mismatch": mismatch,
            "limit": limit}),
        );
    }
    let total = same + substituted + failed + unresolved;
    // v2.1: under connected drift the per-row rule stays unresolved; the
    // per-layer mismatch limit decides, and a structural failure always fails.
    let layer_limits = policy.router_layer_mismatch.is_some();
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
        "layers_over_limit": over,
        "passed": total > 0
            && failed == 0
            && if layer_limits {
                over.is_empty()
            } else {
                unresolved == 0 && fraction_ok == Some(true)
            },
        "layers": layers,
        "examples": examples,
    }))
}

/// The runtime's words of one site for one token of a step.
fn site_row(
    record: &Path,
    files: &[String],
    layer: usize,
    site: &str,
    token: usize,
    tokens: usize,
) -> Result<Option<Vec<u32>>, Refused> {
    use std::io::{Read, Seek, SeekFrom};
    let files: Vec<_> = files
        .iter()
        .filter(|f| f.ends_with(&format!(".L{layer}.{site}.u32")))
        .map(|f| record.join(f))
        .collect();
    if files.is_empty() || token >= tokens {
        return Ok(None);
    }
    let lengths: Vec<_> = files
        .iter()
        .map(|f| std::fs::metadata(f).map(|m| m.len()))
        .collect::<Result<_, _>>()
        .map_err(|e| refused(e.to_string()))?;
    let total: u64 = lengths.iter().sum();
    if total == 0 || !total.is_multiple_of(tokens as u64 * 4) {
        return Err(refused(format!(
            "layer {layer} {site}: observation shape does not cover {tokens} tokens"
        )));
    }
    let row = total / tokens as u64;
    let mut offset = token as u64 * row;
    for (path, len) in files.iter().zip(lengths) {
        if !len.is_multiple_of(row) {
            return Err(refused("a chunk observation ends inside a token row"));
        }
        if offset < len {
            let mut file = std::fs::File::open(path).map_err(|e| refused(e.to_string()))?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| refused(e.to_string()))?;
            let mut bytes = vec![0; row as usize];
            file.read_exact(&mut bytes)
                .map_err(|e| refused(e.to_string()))?;
            return Ok(Some(
                bytes
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect(),
            ));
        }
        offset -= len;
    }
    Ok(None)
}

/// Exact checks at the oracle's captured positions: Engram hash IDs and
/// gathered rows on layers 1 and 14. Also reports, without a limit, how far
/// the router's input has drifted from the oracle's on each captured layer.
fn exact_case(
    oracle: &mut OracleDump,
    run: &Value,
    case: &str,
    record: &Path,
    rec: &Recorded,
) -> Result<Value, Refused> {
    let prompt = run["prompt_tokens"].as_u64().unwrap_or_default() as usize;
    let mut checked = 0usize;
    let mut failed = Vec::new();
    let mut drift = Vec::new();
    let labels = run["labels"].as_object().cloned().unwrap_or_default();
    for (label, position) in &labels {
        let Some(position) = position.as_u64().map(|p| p as usize) else {
            continue;
        };
        let (step, token, tokens) = if position < prompt {
            (0, position, prompt)
        } else {
            (position - prompt + 1, 0, 1)
        };
        let Some(files) = rec.records.get(step) else {
            continue;
        };
        for layer in [1usize, 14] {
            let ids_name = format!("{case}.decode.{label}.layers.{layer}.engram.hash_ids");
            if oracle.has(&ids_name) {
                let reference = oracle.integers(&ids_name)?;
                let got = site_row(record, files, layer, "EngramIds", token, tokens)?.map(|w| {
                    w.chunks_exact(2)
                        .map(|p| (u64::from(p[0]) | (u64::from(p[1]) << 32)) as i64)
                        .collect::<Vec<i64>>()
                });
                checked += 1;
                if got.as_ref() != Some(&reference) {
                    failed.push(format!("{label} layer {layer} Engram hash IDs"));
                }
            }
            let rows_name = format!("{case}.decode.{label}.layers.{layer}.engram.rows");
            if oracle.has(&rows_name) {
                let reference = oracle.floats(&rows_name)?;
                let got = site_row(record, files, layer, "EngramRows", token, tokens)?
                    .map(|w| w.into_iter().map(f32::from_bits).collect::<Vec<f32>>());
                checked += 1;
                if got.as_ref() != Some(&reference) {
                    failed.push(format!("{label} layer {layer} Engram rows"));
                }
            }
        }
        for layer in 0..LAYERS {
            let name = format!("{case}.decode.{label}.layers.{layer}.ffn.gate.in");
            if !oracle.has(&name) {
                continue;
            }
            let reference = oracle.floats(&name)?;
            if let Some(got) = site_row(record, files, layer, "FfnIn", token, tokens)? {
                let got: Vec<f32> = got.into_iter().map(f32::from_bits).collect();
                let max = reference
                    .iter()
                    .zip(&got)
                    .map(|(a, b)| f64::from((a - b).abs()))
                    .fold(0.0, f64::max);
                drift.push(json!({"label": label, "layer": layer, "max_abs": max,
                    "equal_values": reference.iter().zip(&got).filter(|(a, b)| a == b).count(),
                    "values": reference.len()}));
            }
        }
    }
    Ok(json!({
        "checked": checked,
        "failed": failed,
        // An oracle without captured positions (the synthetic tests) has
        // nothing to check; the real oracle's captured rows must be checked.
        "passed": failed.is_empty() && (checked > 0 || labels.is_empty()),
        "router_input_drift": drift,
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
        router_layer_mismatch: None,
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
        let rec = match run_case(&mut group, &record, case, &prompt, &generated, "") {
            Ok(rec) => rec,
            // A negative control that the stages refuse is detected, not an error.
            Err(refusal) if opts.control.is_some() => {
                reports.push(json!({"case": case, "refused": refusal.to_string()}));
                break;
            }
            Err(refusal) => return Err(refusal),
        };
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
        let (router, exact) = if opts.observe {
            (
                Some(router_case(
                    &policy,
                    &mut oracle,
                    &run,
                    case,
                    &record,
                    &rec,
                )?),
                Some(exact_case(&mut oracle, &run, case, &record, &rec)?),
            )
        } else {
            (None, None)
        };
        reports.push(json!({
            "case": case,
            "prompt_tokens": prompt.len(),
            "generated_tokens": generated.len(),
            "seconds": rec.seconds,
            "replay_equals_forced_decode": replay_equal,
            "modes": modes,
            "router": router,
            "exact": exact,
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
            && (r["exact"].is_null() || r["exact"]["passed"] == json!(true))
    });
    let exe = std::env::current_exe().map_err(|e| refused(e.to_string()))?;
    let report = json!({
        "schema": "psionic.deepseek_v41.g1_v2_report.v1",
        "issue": 12405,
        "policy_version": policy.version,
        "policy_sha256": policy_sha,
        "runtime": group.runtime,
        "profile": group.profile,
        "stage_bundle_sha256": group.build,
        "stage_executable_sha256": group.executable,
        "expected_bundle_sha256": opts.worker_build,
        "launch": if opts.development { "development" } else { "pinned" },
        "weights": group.weights,
        "manifest": opts.manifest,
        "binary_sha256": sha256_file(&exe),
        "oracle_manifest_sha256": sha256_file(&opts.oracle.join("manifest.json")),
        "backend": match opts.backend { Backend::Cpu => "cpu", Backend::Cuda => "cuda" },
        "arithmetic": opts.arithmetic,
        "schedule": {"kind": "serial", "max_context": opts.max_context,
            "max_step_tokens": opts.max_step_tokens.or(opts.prefill_chunk).unwrap_or(opts.max_context),
            "prefill_chunk": opts.prefill_chunk},
        "stage_limits": group.limits,
        "control": opts.control.map(Control::name),
        "observe": opts.observe,
        "arithmetic_schedule": "one graph per token; the head runs only after the last prompt chunk and each forced decode token",
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

// ---------------------------------------------------------------------------
// Deriving the policy
// ---------------------------------------------------------------------------

/// Headroom over the reference control's largest error.
const HEADROOM: f64 = 2.0;

/// Derives policy v2.1 from calibration reports (#12405). Version 2.0 of
/// this rule was committed before any calibration ran; on the calibration it
/// refused, because the wrong-routing control passed every distribution
/// limit. Version 2.1 adds the per-layer router limit, still taken from the
/// reference alone, before any holdout case has run:
///
/// - `reference` reports come from an independent implementation that differs
///   from the oracle only in arithmetic: the Psionic CPU port in the
///   `bf16-tp8` profile, run through this harness. Each per-position limit is
///   [`HEADROOM`] times the largest value it reached; the case-mean limit is
///   [`HEADROOM`] times its largest case mean; the tail threshold is half the
///   per-position limit, with a tail fraction of [`HEADROOM`] times the
///   largest observed fraction, at least one position in 33 and at most half.
/// - The top-1 budget `epsilon` is the reference's largest centered-logit
///   error: the largest logit change arithmetic alone produced. At most one
///   substitution per position, and a substituted fraction of [`HEADROOM`]
///   times the reference's, at least one position in 33.
/// - Per-row router and index budgets stay unset: under connected drift the
///   uncertainty of a row's scores is unmeasured. Instead, each layer's
///   router mismatch fraction may be at most [`HEADROOM`] times the
///   reference's largest, at least [`ROUTER_FLOOR`] (`router_limits`).
/// - `controls` are negative-control reports. Each must fail the derived
///   policy, or the derivation refuses.
///
/// The candidate's own results never enter.
///
/// # Errors
///
/// Missing reference positions, or a control the policy would pass.
pub fn derive_policy(reference: &[Value], controls: &[Value]) -> Result<(Policy, Value), Refused> {
    let mut tv = 0.0f64;
    let mut js = 0.0f64;
    let mut lp = 0.0f64;
    let mut centered = 0.0f64;
    let mut case_mean = 0.0f64;
    let mut positions = 0usize;
    let mut substituted = 0usize;
    let mut cases = Vec::new();
    for report in reference {
        for case in report["cases"].as_array().into_iter().flatten() {
            for mode in case["modes"].as_array().into_iter().flatten() {
                let ps = mode["positions"].as_array().cloned().unwrap_or_default();
                if ps.is_empty() {
                    continue;
                }
                let mut tvs = Vec::new();
                for p in &ps {
                    let m = &p["metrics"];
                    let t = m["total_variation"].as_f64().unwrap_or(f64::NAN);
                    tvs.push(t);
                    tv = tv.max(t);
                    js = js.max(m["jensen_shannon"].as_f64().unwrap_or(f64::NAN));
                    lp = lp.max(
                        m["reference_logprob_error"]
                            .as_f64()
                            .unwrap_or(f64::NAN)
                            .abs(),
                    );
                    centered = centered.max(m["centered_logit_error"].as_f64().unwrap_or(f64::NAN));
                    if m["oracle_top1"] != m["candidate_top1"] {
                        substituted += 1;
                    }
                }
                positions += ps.len();
                case_mean = case_mean.max(tvs.iter().sum::<f64>() / tvs.len() as f64);
                cases.push((tvs, mode["case"].clone(), mode["mode"].clone()));
            }
        }
    }
    if positions == 0 || [tv, js, lp, centered].iter().any(|v| !v.is_finite()) {
        return Err(refused("the reference control has no complete positions"));
    }
    let tv_limit = HEADROOM * tv;
    let tail = tv_limit / 2.0;
    let tail_fraction = cases
        .iter()
        .map(|(t, _, _)| t.iter().filter(|v| **v > tail).count() as f64 / t.len() as f64)
        .fold(0.0, f64::max);
    let floor = 1.0 / 33.0;
    let policy = Policy {
        schema: q::POLICY_SCHEMA.into(),
        version: "2.1".into(),
        distribution: q::DistributionLimits {
            total_variation: Some(tv_limit),
            jensen_shannon: Some(HEADROOM * js),
            reference_logprob_error: Some(HEADROOM * lp),
            case_mean_total_variation: Some(HEADROOM * case_mean),
            tail_total_variation: Some(tail),
            tail_fraction: Some((HEADROOM * tail_fraction).clamp(floor, 0.5)),
        },
        top1: DecisionRule {
            epsilon: Some(centered),
            max_substitutions: 1,
            max_substituted_fraction: Some(
                (HEADROOM * substituted as f64 / positions as f64).max(floor),
            ),
        },
        router: DecisionRule {
            epsilon: None,
            max_substitutions: 1,
            max_substituted_fraction: None,
        },
        index: DecisionRule {
            epsilon: None,
            max_substitutions: 2,
            max_substituted_fraction: None,
        },
        router_layer_mismatch: router_limits(reference)?,
    };
    let mut detected = Vec::new();
    for report in controls {
        let control = report["control"].as_str().unwrap_or("unnamed").to_owned();
        let mut failures = 0usize;
        for case in report["cases"].as_array().into_iter().flatten() {
            if case["refused"].is_string() {
                // The stages' exact checks refused the defect's state.
                failures += 1;
            }
            if let Some(limits) = &policy.router_layer_mismatch {
                for layer in case["router"]["layers"].as_array().into_iter().flatten() {
                    let l = layer["layer"].as_u64().unwrap_or_default() as usize;
                    let m = layer["mismatch"].as_f64().unwrap_or_default();
                    if limits.get(l).is_some_and(|limit| m > *limit) {
                        failures += 1;
                    }
                }
            }
            for mode in case["modes"].as_array().into_iter().flatten() {
                let ps: Vec<PositionResult> = mode["positions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| {
                        serde_json::from_value::<q::Distribution>(p["metrics"].clone()).ok()
                    })
                    .enumerate()
                    .map(|(i, m)| q::judge_position(&policy, &format!("p{i}"), m))
                    .collect();
                let judged = q::judge_case(
                    &policy,
                    mode["case"].as_str().unwrap_or_default(),
                    mode["mode"].as_str().unwrap_or_default(),
                    ps,
                    Vec::new(),
                );
                failures += judged.failures.len();
            }
        }
        if failures == 0 {
            return Err(refused(format!(
                "the derived policy would pass the {control} negative control"
            )));
        }
        detected.push(json!({"control": control, "failures": failures}));
    }
    let evidence = json!({
        "rule": "derive_policy in crates/psionic/crates/psionic-v41-stage/src/qualify.rs",
        "headroom": HEADROOM,
        "reference_positions": positions,
        "reference_max": {"total_variation": tv, "jensen_shannon": js,
            "reference_logprob_error": lp, "centered_logit_error": centered,
            "case_mean_total_variation": case_mean, "tail_fraction": tail_fraction,
            "top1_substitutions": substituted},
        "controls": detected,
    });
    Ok((policy, evidence))
}

/// The smallest per-layer router mismatch limit: the oracle's own near ties
/// make a few rows ambiguous even without drift.
const ROUTER_FLOOR: f64 = 0.05;

/// Per layer, [`HEADROOM`] times the reference's largest router mismatch
/// fraction over its cases, at least [`ROUTER_FLOOR`] and at most 1.
fn router_limits(reference: &[Value]) -> Result<Option<Vec<f64>>, Refused> {
    let mut worst = vec![None::<f64>; LAYERS];
    for report in reference {
        for case in report["cases"].as_array().into_iter().flatten() {
            for layer in case["router"]["layers"].as_array().into_iter().flatten() {
                let l = layer["layer"].as_u64().unwrap_or(u64::MAX) as usize;
                if let (Some(slot), Some(m)) = (worst.get_mut(l), layer["mismatch"].as_f64()) {
                    *slot = Some(slot.map_or(m, |w: f64| w.max(m)));
                }
            }
        }
    }
    if worst.iter().all(Option::is_none) {
        return Ok(None);
    }
    worst
        .into_iter()
        .enumerate()
        .map(|(l, w)| {
            w.map(|w| (HEADROOM * w).clamp(ROUTER_FLOOR, 1.0))
                .ok_or_else(|| refused(format!("the reference has no router rows for layer {l}")))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// `qualify-derive --reference R[,R...] --control C[,C...] --out FILE`.
///
/// # Errors
///
/// Unreadable reports, or a derivation refusal.
pub fn run_derive(args: &[String]) -> Result<(), Refused> {
    let flag = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let read = |list: Option<String>| -> Result<Vec<Value>, Refused> {
        list.unwrap_or_default()
            .split(',')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let bytes = std::fs::read(p).map_err(|e| refused(format!("{p}: {e}")))?;
                serde_json::from_slice(&bytes).map_err(|e| refused(format!("{p}: {e}")))
            })
            .collect()
    };
    let reference = read(flag("--reference"))?;
    let controls = read(flag("--control"))?;
    let out = flag("--out").ok_or_else(|| refused("--out is required"))?;
    let (policy, evidence) = derive_policy(&reference, &controls)?;
    let mut value = serde_json::to_value(&policy).map_err(|e| refused(e.to_string()))?;
    value["derivation"] = evidence;
    std::fs::write(
        &out,
        serde_json::to_vec_pretty(&value).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(format!("{out}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_rows_cover_all_chunks_in_absolute_prompt_order() {
        let root = tempfile::tempdir().unwrap();
        let files = vec![
            "r-0-0.stage0.L1.EngramIds.u32".to_owned(),
            "r-0-1.stage0.L1.EngramIds.u32".to_owned(),
        ];
        std::fs::write(
            root.path().join(&files[0]),
            (0u32..6).flat_map(u32::to_le_bytes).collect::<Vec<_>>(),
        )
        .unwrap();
        std::fs::write(
            root.path().join(&files[1]),
            (6u32..10).flat_map(u32::to_le_bytes).collect::<Vec<_>>(),
        )
        .unwrap();
        for token in 0..5 {
            assert_eq!(
                site_row(root.path(), &files, 1, "EngramIds", token, 5).unwrap(),
                Some(vec![token as u32 * 2, token as u32 * 2 + 1])
            );
        }
        assert!(
            site_row(root.path(), &files, 1, "EngramIds", 5, 5)
                .unwrap()
                .is_none()
        );
        assert!(site_row(root.path(), &files[..1], 1, "EngramIds", 4, 5).is_err());
    }

    fn report(tvs: &[(f64, u32)], control: Option<&str>) -> Value {
        let positions: Vec<Value> = tvs
            .iter()
            .map(|(tv, top)| {
                json!({"metrics": {"total_variation": tv, "jensen_shannon": tv / 10.0,
                "reference_logprob_error": -tv, "centered_logit_error": tv * 3.0,
                "oracle_top1": 5, "candidate_top1": top, "oracle_top1_gap": 1.0,
                "top1_regret": if *top == 5 { 0.0 } else { 0.5 }}})
            })
            .collect();
        json!({"control": control, "cases": [{"modes": [
            {"case": "c", "mode": "decode", "positions": positions}]}]})
    }

    #[test]
    fn the_policy_follows_the_reference_and_must_catch_every_control() {
        let reference = report(&[(0.01, 5), (0.03, 5), (0.02, 6)], None);
        let control = report(&[(0.5, 9), (0.4, 9)], Some("wrong-weights"));
        let (policy, evidence) = derive_policy(&[reference.clone()], &[control]).unwrap();
        let d = policy.distribution;
        assert!((d.total_variation.unwrap() - 0.06).abs() < 1e-12);
        assert!((d.jensen_shannon.unwrap() - 0.006).abs() < 1e-12);
        assert!((d.case_mean_total_variation.unwrap() - 0.04).abs() < 1e-12);
        assert!((policy.top1.epsilon.unwrap() - 0.09).abs() < 1e-12);
        assert!(policy.router.epsilon.is_none() && policy.index.epsilon.is_none());
        assert_eq!(evidence["controls"][0]["control"], "wrong-weights");
        // A control as small as the reference is not detected: refuse.
        let weak = report(&[(0.01, 5)], Some("wrong-rope"));
        assert!(derive_policy(&[reference], &[weak]).is_err());
        assert!(derive_policy(&[report(&[], None)], &[]).is_err());
    }
}
