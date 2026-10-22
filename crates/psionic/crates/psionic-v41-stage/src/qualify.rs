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

mod calibration;
mod index;
mod isolated;
mod scope;

/// Collects isolated-stage output metrics on original oracle inputs.
///
/// # Errors
///
/// Refuses missing captures, a wrong bundle, and unsupported execution modes.
pub fn run_isolated(args: &[String]) -> Result<(), Refused> {
    isolated::run(args)
}

/// Rechecks hashed index observations against a complete oracle capture.
///
/// # Errors
///
/// Refuses missing evidence, changed inputs or files, and a failed verdict.
pub fn run_index_check(args: &[String]) -> Result<(), Refused> {
    index::run(args)
}

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
    /// Prospective confirmation corpus, required for release.
    pub corpus: Option<PathBuf>,
    pub max_context: usize,
    pub max_step_tokens: Option<usize>,
    pub prefill_chunk: Option<usize>,
    pub step_timeout: Duration,
    /// The bundle hash every stage must announce (#12410); `None` only with
    /// `development`.
    pub worker_build: Option<String>,
    /// A run without a build pin, labelled as one in the report.
    pub development: bool,
    /// Release qualification: refuse missing evidence (layers, steps, router
    /// and exact reports, an unevaluated rule) instead of reporting it.
    pub release: bool,
    /// The stage worker to start, when it isn't this executable: a pinned
    /// bundle's worker driven by a newer front.
    pub worker: Option<PathBuf>,
    /// G2 v2 (#12405): instead of the forced comparison, answer each case
    /// greedily from the oracle's prompt IDs to the end token or this cap.
    pub answers: Option<usize>,
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
        let mut release = false;
        let mut development = false;
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            if flag == "--observe" {
                observe = true;
            } else if flag == "--release" {
                release = true;
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
                "--corpus",
                "--max-context",
                "--max-step-tokens",
                "--prefill-chunk",
                "--step-secs",
                "--worker-build",
                "--answers",
                "--worker",
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
            corpus: values.remove("--corpus").map(PathBuf::from),
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
            worker: values.remove("--worker").map(PathBuf::from),
            release,
            answers: values
                .remove("--answers")
                .map(|v| {
                    v.parse()
                        .map_err(|_| refused("--answers takes a token cap"))
                })
                .transpose()?,
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
    run_hashes: BTreeMap<String, String>,
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
            run_hashes: serde_json::from_value(manifest["run_files_sha256"].clone())
                .unwrap_or_default(),
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
        if self
            .run_hashes
            .get(&format!("{case}/run.json"))
            .is_some_and(|sha| *sha != hex::encode(Sha256::digest(&bytes)))
        {
            return Err(refused(format!(
                "{case}: run metadata does not match its manifest hash"
            )));
        }
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
    execution_configs: Vec<Value>,
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
    /// The prompt mode the steps name (`thinking` or `chat`), when set.
    mode: Option<String>,
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
        let exe = match &opts.worker {
            Some(worker) => worker.clone(),
            None => std::env::current_exe().map_err(|e| refused(e.to_string()))?,
        };
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
                opts.backend.name().into(),
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
            execution_configs: vec![Value::Null; stages],
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
            mode: None,
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
                    group.execution_configs[i] = hello["execution_config_sha256"].clone();
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
                if let Some(mode) = &self.mode {
                    // The last stage's parser splits reasoning only in thinking mode.
                    message["mode"] = json!(mode);
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
#[derive(serde::Serialize, serde::Deserialize)]
struct Recorded {
    /// Label (`last`, `g1`, ...) to the logits file.
    logits: BTreeMap<String, PathBuf>,
    /// The replayed fixed sequence's last logits.
    replay_last: Option<PathBuf>,
    /// Per step, the files the stages wrote.
    records: Vec<Vec<String>>,
    #[serde(default)]
    replay_records: Vec<String>,
    #[serde(default)]
    files_sha256: BTreeMap<String, String>,
    #[serde(default)]
    source: Value,
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
    let replay_records = record_files(&ack);
    let replay_last = logits_file(record, &replay_records);
    let mut files_sha256 = BTreeMap::new();
    for file in records.iter().flatten().chain(&replay_records) {
        if !files_sha256.contains_key(file) {
            files_sha256.insert(file.clone(), checked_sha256(&record.join(file))?);
        }
    }
    let result = Recorded {
        files_sha256,
        source: json!({"case":case,"runtime":group.runtime,"profile":group.profile,
            "stage_bundle_sha256":group.build,"stage_executable_sha256":group.executable,"execution_config_sha256":group.execution_configs,
            "weights":group.weights,"limits":group.limits,
            "prompt_ids_sha256":hex::encode(Sha256::digest(serde_json::to_vec(prompt).map_err(|e| refused(e.to_string()))?)),
            "generated_ids_sha256":hex::encode(Sha256::digest(serde_json::to_vec(generated).map_err(|e| refused(e.to_string()))?))}),
        logits,
        replay_last,
        records,
        replay_records,
        seconds: started.elapsed().as_secs_f64(),
    };
    std::fs::write(
        record.join(format!("{request}.records.json")),
        serde_json::to_vec_pretty(&result).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(e.to_string()))?;
    Ok(result)
}

fn checked_sha256(path: &Path) -> Result<String, Refused> {
    let mut file = File::open(path).map_err(|e| refused(format!("{}: {e}", path.display())))?;
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let n = file.read(&mut bytes).map_err(|e| refused(e.to_string()))?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(hex::encode(hash.finalize()))
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
    weights: Option<&Path>,
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
    let mut scored_layers = 0usize;
    let mut candidate_layers = 0usize;
    let mut total_over_budget = 0usize;
    let mut changes = Vec::new();
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
        // The oracle's retained best scores per row (`--router-tops`), in the
        // selection function's units, sqrt(softplus(linear(x) / temp)) + bias.
        let tops_name = format!("{case}.decode.router_tops.layers.{layer}");
        let tops = if oracle.has(&format!("{tops_name}.scores")) {
            let scores = oracle.floats(&format!("{tops_name}.scores"))?;
            let ids = oracle.integers(&format!("{tops_name}.ids"))?;
            let width = scores.len() / row_step.len().max(1);
            if ids.len() != scores.len() || width <= top_k || width * row_step.len() != scores.len()
            {
                return Err(refused(format!(
                    "layer {layer}: malformed oracle router scores"
                )));
            }
            Some((scores, ids, width))
        } else {
            None
        };
        scored_layers += usize::from(tops.is_some());
        // The candidate's selection scores: a stage that reports them
        // (the CPU port's `RouterScores`), or an F64 evaluation of the
        // selection function on the stage's router input (`FfnIn`).
        let candidate_scores =
            candidate_router_scores(record, rec, layer, row_step.len(), weights)?;
        let experts = candidate_scores
            .as_ref()
            .map_or(EXPERTS as usize, |(_, scores)| {
                scores.len() / row_step.len()
            });
        if experts < top_k
            || tops
                .as_ref()
                .is_some_and(|(_, ids, _)| ids.iter().any(|id| *id < 0 || *id as usize >= experts))
        {
            return Err(refused(format!(
                "layer {layer}: oracle router IDs exceed the candidate score width {experts}"
            )));
        }
        let budget = policy
            .router_score_budget
            .as_ref()
            .and_then(|b| b.get(layer).copied());
        let rule = DecisionRule {
            epsilon: budget.or(rule.epsilon),
            ..rule
        };
        let mut errors = Vec::new();
        let (mut over_budget, mut local_changed, mut most_subs) = (0usize, 0usize, 0usize);
        let mut layer_counts = [0usize; 4];
        for (row, (step, r)) in row_step.iter().enumerate() {
            let o: Vec<u32> = reference[row * top_k..(row + 1) * top_k]
                .iter()
                .map(|x| *x as u32)
                .collect();
            let c = &candidate[row * top_k..(row + 1) * top_k];
            // With retained scores: the boundary gap is the k-th minus the
            // (k+1)-th score, and only the retained IDs have a known score.
            let row_scores: Option<BTreeMap<u32, f64>> = tops.as_ref().map(|(sc, ids, w)| {
                (0..*w)
                    .map(|j| (ids[row * w + j] as u32, f64::from(sc[row * w + j])))
                    .collect()
            });
            let tops_gap = tops.as_ref().map(|(sc, _, w)| {
                f64::from(sc[row * w + top_k - 1]) - f64::from(sc[row * w + top_k])
            });
            let gap = tops_gap
                .or_else(|| ties.get(&(*step, layer, *r)).copied())
                .or_else(|| {
                    // Unlisted: the gap is at least the oracle's tie window,
                    // unless its list for that forward was cut off.
                    let complete = tie_counts.get(&(*step, layer)).copied().unwrap_or_default()
                        < ORACLE_TIE_CAP;
                    let stable = rule.epsilon.is_some_and(|e| 2.0 * e < ORACLE_TIE_WINDOW);
                    (complete && stable).then_some(ORACLE_TIE_WINDOW)
                });
            let decision = rule.check(&Selection {
                oracle: &o,
                candidate: c,
                boundary_gap: gap,
                oracle_scores: row_scores.as_ref(),
                universe: experts as u32,
            });
            // The measured score error on the oracle's retained experts.
            let row_error = match (&candidate_scores, &row_scores) {
                (Some((_, cs)), Some(retained)) => {
                    let row_s = &cs[row * experts..(row + 1) * experts];
                    let e = retained
                        .iter()
                        .map(|(id, s)| (row_s[*id as usize] - s).abs())
                        .fold(0.0f64, f64::max);
                    if !same_set(&top_k_of(row_s, top_k), c) {
                        local_changed += 1;
                    }
                    errors.push(e);
                    Some(e)
                }
                _ => None,
            };
            if let (Some(e), Some(b)) = (row_error, budget)
                && e > b
            {
                over_budget += 1;
            }
            let removed: Vec<u32> = o.iter().filter(|x| !c.contains(x)).copied().collect();
            if !removed.is_empty() {
                let added: Vec<u32> = c.iter().filter(|x| !o.contains(x)).copied().collect();
                most_subs = most_subs.max(removed.len());
                let regret = row_scores.as_ref().and_then(|sc| {
                    let out: Option<f64> = removed.iter().map(|i| sc.get(i).copied()).sum();
                    let inn: Option<f64> = added.iter().map(|i| sc.get(i).copied()).sum();
                    Some(out? - inn?)
                });
                changes.push(
                    json!({"layer": layer, "row": row, "substitutions": removed.len(),
                    "oracle_gap": gap, "regret": regret, "score_error": row_error}),
                );
            }
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
        errors.sort_by(f64::total_cmp);
        let score_error = (!errors.is_empty()).then(|| {
            json!({"rows": errors.len(), "p50": errors[errors.len() / 2],
                "p99": errors[(errors.len() * 99 / 100).min(errors.len() - 1)],
                "max": errors[errors.len() - 1],
                "from": candidate_scores.as_ref().map(|(how, _)| *how)})
        });
        candidate_layers += usize::from(score_error.is_some());
        total_over_budget += over_budget;
        layers.push(
            json!({"layer": layer, "same": layer_counts[0], "substituted": layer_counts[1],
            "failed": layer_counts[2], "unresolved": layer_counts[3], "mismatch": mismatch,
            "limit": limit, "score_error": score_error, "score_budget": budget,
            "over_budget": over_budget, "local_changed": local_changed,
            "max_row_substitutions": most_subs}),
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
        "layers_compared": layers.len(),
        "layers_with_oracle_scores": scored_layers,
        "layers_with_candidate_scores": candidate_layers,
        "layers_with_stage_scores": layers.iter().filter(|l| l["score_error"]["from"] == "stage").count(),
        "score_selection_mismatches": layers.iter().filter_map(|l| l["local_changed"].as_u64()).sum::<u64>(),
        "over_budget": total_over_budget,
        // Two separate verdicts (review of 2026-10-22): the empirical
        // per-layer fraction limits, and the margin/regret rule, which needs
        // every changed row resolved.
        "fraction_verdict": if layer_limits { json!(failed == 0 && over.is_empty()) } else { Value::Null },
        "margin_verdict": failed == 0
            && unresolved == 0
            && total_over_budget == 0
            && layers.iter().all(|l| l["local_changed"] == 0)
            && fraction_ok != Some(false)
            && candidate_layers == layers.len()
            && scored_layers == layers.len(),
        "passed": total > 0
            && failed == 0
            && if layer_limits {
                over.is_empty()
            } else {
                unresolved == 0 && fraction_ok == Some(true)
            },
        "layers": layers,
        "examples": examples,
        // Every changed expert set, so a policy can judge it afterwards.
        "changes": changes,
    }))
}

fn top_k_of(scores: &[f64], k: usize) -> Vec<u32> {
    let mut idx: Vec<u32> = (0..scores.len() as u32).collect();
    idx.sort_by(|a, b| {
        scores[*b as usize]
            .total_cmp(&scores[*a as usize])
            .then(a.cmp(b))
    });
    idx.truncate(k);
    idx
}

fn same_set(a: &[u32], b: &[u32]) -> bool {
    a.len() == b.len() && a.iter().all(|x| b.contains(x))
}

/// The selection function sqrt(softplus(z)) + bias, with PyTorch's
/// softplus threshold of 20.
fn selection_score(z: f64, bias: f64) -> f64 {
    let soft = if z > 20.0 { z } else { z.exp().ln_1p() };
    soft.sqrt() + bias
}

/// One tensor of the checkpoint as F32, through its safetensors index.
fn checkpoint_tensor(weights: &Path, name: &str) -> Result<Vec<f32>, Refused> {
    let index: Value = serde_json::from_slice(
        &std::fs::read(weights.join("model.safetensors.index.json"))
            .map_err(|e| refused(format!("checkpoint index: {e}")))?,
    )
    .map_err(|e| refused(format!("checkpoint index: {e}")))?;
    let file = index["weight_map"][name]
        .as_str()
        .ok_or_else(|| refused(format!("{name} is not in the checkpoint")))?;
    let mut f = File::open(weights.join(file)).map_err(|e| refused(format!("{file}: {e}")))?;
    let mut len = [0u8; 8];
    f.read_exact(&mut len).map_err(|e| refused(e.to_string()))?;
    let n = u64::from_le_bytes(len) as usize;
    let mut header = vec![0u8; n];
    f.read_exact(&mut header)
        .map_err(|e| refused(e.to_string()))?;
    let header: Value = serde_json::from_slice(&header).map_err(|e| refused(e.to_string()))?;
    let meta = &header[name];
    let (a, b) = (
        meta["data_offsets"][0].as_u64().unwrap_or_default(),
        meta["data_offsets"][1].as_u64().unwrap_or_default(),
    );
    f.seek(SeekFrom::Start(8 + n as u64 + a))
        .map_err(|e| refused(e.to_string()))?;
    let mut raw = vec![0u8; (b - a) as usize];
    f.read_exact(&mut raw).map_err(|e| refused(e.to_string()))?;
    match meta["dtype"].as_str() {
        Some("BF16") => Ok(raw
            .chunks_exact(2)
            .map(|c| f32::from_bits(u32::from(u16::from_le_bytes([c[0], c[1]])) << 16))
            .collect()),
        Some("F32") => Ok(raw
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()),
        other => Err(refused(format!("{name}: dtype {other:?}"))),
    }
}

/// The candidate's selection scores for every row of one layer (384 per
/// original-weight row, or the synthetic model's smaller expert count):
/// the stage's own `RouterScores` when it reports them, else an F64
/// evaluation on its recorded router input (`FfnIn`) with the checkpoint's
/// gate. `None` when neither is recorded.
fn candidate_router_scores(
    record: &Path,
    rec: &Recorded,
    layer: usize,
    rows: usize,
    weights: Option<&Path>,
) -> Result<Option<(&'static str, Vec<f64>)>, Refused> {
    let gather = |site: &str| -> Result<Option<Vec<u32>>, Refused> {
        let mut out = Vec::new();
        for files in &rec.records {
            let matching: Vec<_> = files
                .iter()
                .filter(|f| f.ends_with(&format!(".L{layer}.{site}.u32")))
                .collect();
            if matching.is_empty() {
                return Ok(None);
            }
            for file in matching {
                out.extend(read_u32(&record.join(file))?);
            }
        }
        Ok(Some(out))
    };
    if let Some(words) = gather("RouterScores")? {
        if rows == 0 || words.is_empty() || !words.len().is_multiple_of(rows) {
            return Err(refused(format!(
                "layer {layer}: {} router scores for {rows} rows",
                words.len()
            )));
        }
        let scores: Vec<f64> = words
            .iter()
            .map(|w| f64::from(f32::from_bits(*w)))
            .collect();
        if scores.iter().any(|s| !s.is_finite()) {
            return Err(refused(format!(
                "layer {layer}: nonfinite stage router score"
            )));
        }
        return Ok(Some(("stage", scores)));
    }
    let (Some(weights), Some(words)) = (weights, gather("FfnIn")?) else {
        return Ok(None);
    };
    let gate = checkpoint_tensor(weights, &format!("layers.{layer}.ffn.gate.weight"))?;
    let bias = checkpoint_tensor(weights, &format!("layers.{layer}.ffn.gate.bias"))?;
    let experts = bias.len();
    if experts == 0 || !gate.len().is_multiple_of(experts) {
        return Err(refused(format!("layer {layer}: gate shape")));
    }
    let width = gate.len() / experts;
    if words.len() != rows * width {
        return Err(refused(format!(
            "layer {layer}: {} router input words for {rows} rows",
            words.len()
        )));
    }
    let gate: Vec<f64> = gate.iter().map(|v| f64::from(*v)).collect();
    let mut scores = vec![0.0f64; rows * experts];
    let threads = std::thread::available_parallelism().map_or(8, std::num::NonZero::get);
    let per = rows.div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (chunk, out) in scores.chunks_mut(per * experts).enumerate() {
            let (gate, bias, words) = (&gate, &bias, &words);
            scope.spawn(move || {
                for (i, row_out) in out.chunks_mut(experts).enumerate() {
                    let row = chunk * per + i;
                    let x: Vec<f64> = words[row * width..(row + 1) * width]
                        .iter()
                        .map(|w| f64::from(f32::from_bits(*w)))
                        .collect();
                    for (e, slot) in row_out.iter_mut().enumerate() {
                        let w = &gate[e * width..(e + 1) * width];
                        let z: f64 = w.iter().zip(&x).map(|(a, b)| a * b).sum();
                        *slot = selection_score(z, f64::from(bias[e]));
                    }
                }
            });
        }
    });
    Ok(Some(("f64", scores)))
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

/// G2 v2 (#12405): each case's complete greedy answer on the served runtime,
/// from the oracle's own prompt IDs to the end token or `cap` tokens, with
/// the #12372 parser's reasoning and final text from the last stage's acks.
/// Writes `answers.json`; the frozen suite's assertions are checked
/// separately on these texts and the oracle's.
///
/// # Errors
///
/// Any refusal of the oracle, the stages, or the files.
pub fn run_answers(opts: &QualifyOptions, cap: usize) -> Result<Value, Refused> {
    let record = opts.out.join("record");
    std::fs::create_dir_all(&record).map_err(|e| refused(e.to_string()))?;
    let oracle = OracleDump::open(&opts.oracle)?;
    let cases = opts
        .cases
        .clone()
        .ok_or_else(|| refused("--answers needs --cases"))?;
    let mut group = Group::start(opts, &record)?;
    let mut answers = Vec::new();
    for case in &cases {
        let run = oracle.run(case)?;
        let prompt = ids(&run["prompt_token_ids"]);
        if prompt.is_empty() {
            return Err(refused(format!("{case}: the oracle run has no prompt")));
        }
        let started = Instant::now();
        let request = format!("{case}-answer");
        // The oracle's mode, so the runtime's parser splits the answer the same way.
        group.mode = run["thinking_mode"].as_str().map(str::to_owned);
        let (mut tokens, mut reasoning, mut content) = (Vec::new(), String::new(), String::new());
        let mut finish = Value::Null;
        let mut ack = group.step(&request, 0, Some(&prompt), None)?;
        loop {
            let token = ack["token"]
                .as_u64()
                .ok_or_else(|| refused(format!("{case}: the last stage returned no token")))?
                as u32;
            tokens.push(token);
            reasoning.push_str(ack["reasoning"].as_str().unwrap_or_default());
            content.push_str(ack["content"].as_str().unwrap_or_default());
            if !ack["finish"].is_null() {
                finish = ack["finish"].clone();
            }
            if ack["end"] == json!(true) || tokens.len() >= cap {
                break;
            }
            ack = group.step(&request, tokens.len() as u64, None, Some(token))?;
        }
        let oracle_answer = &run["complete_answer"];
        answers.push(json!({
            "case": case,
            "prompt_tokens": prompt.len(),
            "tokens": tokens.len(),
            "ended": ack["end"] == json!(true),
            "finish": finish,
            "ids": tokens,
            "reasoning": reasoning,
            "content": content,
            "seconds": started.elapsed().as_secs_f64(),
            "oracle": {"tokens": oracle_answer["tokens"], "finished": oracle_answer["finished"],
                "ids": oracle_answer["ids"], "parsed": oracle_answer["parsed"]},
            "same_ids_as_oracle": oracle_answer["ids"] == json!(tokens),
        }));
        let _ = std::fs::write(
            opts.out.join("answers.partial.json"),
            serde_json::to_vec_pretty(&answers).unwrap_or_default(),
        );
    }
    let report = json!({
        "schema": "psionic.deepseek_v41.g2_v2_answers.v1",
        "issue": 12405,
        "runtime": group.runtime,
        "profile": group.profile,
        "stage_bundle_sha256": group.build,
        "stage_executable_sha256": group.executable,
        "execution_config_sha256": group.execution_configs,
        "expected_bundle_sha256": opts.worker_build,
        "launch": if opts.development { "development" } else { "pinned" },
        "manifest": opts.manifest,
        "oracle_manifest_sha256": sha256_file(&opts.oracle.join("manifest.json")),
        "cap": cap,
        "answers": answers,
    });
    std::fs::write(
        opts.out.join("answers.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(e.to_string()))?;
    Ok(json!({"passed": Value::Null, "answers": cases.len()}))
}

/// What a release qualification lacks (#12405 review): every case must have
/// its router report over all 40 layers with a resolved margin verdict, its
/// exact report with checks, every position, and an evaluated index rule.
#[must_use]
pub fn release_refusals(opts: &QualifyOptions, policy: &Policy, cases: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    if cases.is_empty() {
        out.push("release qualification needs at least one case".into());
    }
    if opts.control.is_some() {
        out.push("a deliberate control cannot qualify for release".into());
    }
    if opts.arithmetic.as_deref() == Some(crate::CUDA_FP8_REFERENCE) {
        out.push("FP8 arithmetic diagnostics cannot qualify for release".into());
    }
    if !opts.observe {
        out.push("release qualification needs --observe".into());
    }
    if opts.worker_build.is_none() {
        out.push("release qualification needs a pinned bundle".into());
    }
    for c in cases {
        let name = c["case"].as_str().unwrap_or("?");
        if c["refused"].is_string() {
            out.push(format!("{name}: refused"));
            continue;
        }
        let router = &c["router"];
        if router.is_null() {
            out.push(format!("{name}: no router report"));
        } else {
            if router["layers_compared"].as_u64() != Some(LAYERS as u64) {
                out.push(format!(
                    "{name}: router compared {} of {LAYERS} layers",
                    router["layers_compared"]
                ));
            }
            if router["margin_verdict"] != json!(true) {
                out.push(format!(
                    "{name}: router margin/regret verdict not established ({} unresolved, {} failed rows)",
                    router["unresolved"], router["failed"]
                ));
            }
        }
        let exact = &c["exact"];
        if exact.is_null() || exact["checked"].as_u64().unwrap_or(0) == 0 {
            out.push(format!("{name}: no exact checks"));
        }
        let modes: BTreeSet<_> = c["modes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["mode"].as_str())
            .collect();
        if modes != BTreeSet::from(["decode", "replay"]) {
            out.push(format!(
                "{name}: release needs both decode and replay modes"
            ));
        }
        for mode in c["modes"].as_array().into_iter().flatten() {
            if mode["missing"].as_array().is_some_and(|m| !m.is_empty()) {
                out.push(format!(
                    "{name} {}: missing positions {}",
                    mode["mode"], mode["missing"]
                ));
            }
            if mode["positions"].as_array().is_none_or(Vec::is_empty) {
                out.push(format!("{name} {}: no positions", mode["mode"]));
            }
        }
        if policy.index_out_of_scope.is_none()
            && (c["index"]["complete"] != true || c["index"]["passed"] != true)
        {
            out.push(format!("{name}: index rule not evaluated or failed"));
        }
        if policy.router_score_budget.is_some()
            && router["layers_with_candidate_scores"].as_u64() != Some(LAYERS as u64)
        {
            out.push(format!(
                "{name}: candidate router scores for {} of {LAYERS} layers",
                router["layers_with_candidate_scores"]
            ));
        }
        if policy.router_score_budget.is_some()
            && router["layers_with_stage_scores"].as_u64() != Some(LAYERS as u64)
        {
            out.push(format!("{name}: release qualification needs actual stage router scores for all {LAYERS} layers"));
        }
    }
    out
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
        router_score_budget: None,
        index_out_of_scope: None,
    }
}

/// Runs the harness and writes `report.json` under `--out`.
///
/// # Errors
///
/// Any refusal of the oracle, the stages, or the files.
pub fn run(opts: &QualifyOptions) -> Result<Value, Refused> {
    if let Some(cap) = opts.answers {
        return run_answers(opts, cap);
    }
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
    if opts.release {
        scope::preflight(opts)?;
    }
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
                    Some(&opts.weights),
                )?),
                Some(exact_case(&mut oracle, &run, case, &record, &rec)?),
            )
        } else {
            (None, None)
        };
        let index = if opts.observe {
            Some(
                index::case(&policy, &mut oracle, &run, case, &record, &rec).unwrap_or_else(
                    |error| json!({"complete":false,"passed":false,"error":error.to_string()}),
                ),
            )
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
            "index": index,
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
    let violations = judge_report(&policy, &json!({"cases": reports}));
    let release_refusals = if opts.release {
        let mut refusals = release_refusals(opts, &policy, &reports);
        refusals.extend(scope::violations(opts, &group));
        refusals
    } else {
        Vec::new()
    };
    let diagnostic_passed = passed;
    let policy_passed = diagnostic_passed && violations.is_empty();
    let release_passed = opts.release && policy_passed && release_refusals.is_empty();
    let exe = std::env::current_exe().map_err(|e| refused(e.to_string()))?;
    let report = json!({
        "schema": "psionic.deepseek_v41.g1_v2_report.v1",
        "release": opts.release,
        "release_refusals": release_refusals,
        "violations": violations,
        "release_passed": release_passed,
        "issue": 12405,
        "policy_version": policy.version,
        "policy_sha256": policy_sha,
        "corpus_sha256": opts.corpus.as_ref().map(|p|checked_sha256(p)).transpose()?,
        "runtime": group.runtime,
        "profile": group.profile,
        "stage_bundle_sha256": group.build,
        "stage_executable_sha256": group.executable,
        "execution_config_sha256": group.execution_configs,
        "expected_bundle_sha256": opts.worker_build,
        "launch": if opts.development { "development" } else { "pinned" },
        "weights": group.weights,
        "manifest": opts.manifest,
        "binary_sha256": sha256_file(&exe),
        "oracle_manifest_sha256": sha256_file(&opts.oracle.join("manifest.json")),
        "backend": opts.backend.name(),
        "arithmetic": opts.arithmetic,
        "schedule": {"kind": "serial", "max_context": opts.max_context,
            "max_step_tokens": opts.max_step_tokens.or(opts.prefill_chunk).unwrap_or(opts.max_context),
            "prefill_chunk": opts.prefill_chunk},
        "stage_limits": group.limits,
        "control": opts.control.map(Control::name),
        "observe": opts.observe,
        "arithmetic_schedule": "one graph per token; the head runs only after the last prompt chunk and each forced decode token",
        "cases": reports,
        "diagnostic_passed": diagnostic_passed,
        "passed": if opts.release { release_passed } else { policy_passed },
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
        router_score_budget: None,
        index_out_of_scope: None,
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

/// Every numerical criterion of `policy` that a report violates, judged at
/// the policy's own unit: per position, per case and mode, and per case and
/// router layer. A refused case (a control the stages refused) counts as one.
#[must_use]
pub fn judge_report(policy: &Policy, report: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for case in report["cases"].as_array().into_iter().flatten() {
        let name = case["case"].as_str().unwrap_or("?");
        if case["refused"].is_string() {
            out.push(format!("{name}: refused by the stages"));
            continue;
        }
        for mode in case["modes"].as_array().into_iter().flatten() {
            let label = mode["mode"].as_str().unwrap_or("?");
            let positions: Vec<PositionResult> = mode["positions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    let m = serde_json::from_value::<q::Distribution>(p["metrics"].clone()).ok()?;
                    Some(q::judge_position(
                        policy,
                        p["label"].as_str().unwrap_or("?"),
                        m,
                    ))
                })
                .collect();
            let judged = q::judge_case(policy, name, label, positions, Vec::new());
            out.extend(
                judged
                    .failures
                    .iter()
                    .map(|f| format!("{name} {label}: {f}")),
            );
        }
        if let Some(limits) = &policy.router_layer_mismatch {
            for layer in case["router"]["layers"].as_array().into_iter().flatten() {
                let l = layer["layer"].as_u64().unwrap_or_default() as usize;
                let m = layer["mismatch"].as_f64().unwrap_or_default();
                if let Some(limit) = limits.get(l).filter(|limit| m > **limit) {
                    out.push(format!(
                        "{name}: router layer {l} mismatch {m:.4} > {limit:.4}"
                    ));
                }
            }
        }
        if let Some(budget) = &policy.router_score_budget {
            out.extend(router_margin_violations(
                policy,
                budget,
                name,
                &case["router"],
            ));
        }
        if case["index"]["error"].is_string() {
            out.push(format!(
                "{name}: malformed index evidence: {}",
                case["index"]["error"]
            ));
        } else if policy.index_out_of_scope.is_none() && policy.index.epsilon.is_some() {
            out.extend(
                index::violations(policy.index, &case["index"])
                    .into_iter()
                    .map(|v| format!("{name}: {v}")),
            );
        }
        if case["exact"]["passed"] == json!(false) {
            out.push(format!("{name}: exact checks failed"));
        }
    }
    out
}

/// Policy 2.2's router margin rule, recomputed from a report: every layer
/// has measured candidate score errors within its budget, and every changed
/// expert set is resolved, within the substitution cap, at a boundary gap
/// of at most twice the budget, with regret at most twice the budget per
/// substitution. Missing score evidence is a violation, never a pass.
#[must_use]
pub fn router_margin_violations(
    policy: &Policy,
    budget: &[f64],
    name: &str,
    router: &Value,
) -> Vec<String> {
    let mut out = Vec::new();
    if router.is_null() {
        return vec![format!("{name}: no router evidence for the margin rule")];
    }
    for layer in router["layers"].as_array().into_iter().flatten() {
        if layer["local_changed"].as_u64().unwrap_or(0) > 0 {
            out.push(format!(
                "{name}: router layer {} scores do not reproduce {} actual selections",
                layer["layer"], layer["local_changed"]
            ));
        }
        let l = layer["layer"].as_u64().unwrap_or_default() as usize;
        let Some(eps) = budget.get(l).copied() else {
            out.push(format!("{name}: no router score budget for layer {l}"));
            continue;
        };
        match layer["score_error"]["max"].as_f64() {
            None => out.push(format!(
                "{name}: router layer {l} has no measured score error"
            )),
            Some(e) if e > eps => out.push(format!(
                "{name}: router layer {l} score error {e:.3e} > budget {eps:.3e}"
            )),
            Some(_) => {}
        }
    }
    let (mut unresolved, mut capped, mut stable, mut regret_over) =
        (0usize, 0usize, 0usize, 0usize);
    for c in router["changes"].as_array().into_iter().flatten() {
        let l = c["layer"].as_u64().unwrap_or_default() as usize;
        let Some(eps) = budget.get(l).copied() else {
            continue;
        };
        let subs = c["substitutions"].as_u64().unwrap_or(u64::MAX) as usize;
        if subs > policy.router.max_substitutions {
            capped += 1;
            continue;
        }
        let (Some(gap), Some(regret)) = (c["oracle_gap"].as_f64(), c["regret"].as_f64()) else {
            unresolved += 1;
            continue;
        };
        if gap > 2.0 * eps {
            stable += 1;
        } else if regret > 2.0 * eps * subs as f64 {
            regret_over += 1;
        }
    }
    for (count, what) in [
        (
            unresolved,
            "changed router rows without the oracle's scores (unresolved)",
        ),
        (capped, "changed router rows over the substitution cap"),
        (
            stable,
            "router changes at a stable boundary (gap over twice the budget)",
        ),
        (regret_over, "router changes with regret over the budget"),
    ] {
        if count > 0 {
            out.push(format!("{name}: {count} {what}"));
        }
    }
    out
}

/// Whether a report measured candidate router score errors anywhere.
fn has_router_score_evidence(report: &Value) -> bool {
    report["cases"].as_array().into_iter().flatten().any(|c| {
        c["router"]["layers"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|l| l["score_error"]["max"].is_f64())
    })
}

/// The positive and negative self-checks (#12405 review): the reference must
/// pass every criterion derived from it, and every control must fail.
///
/// # Errors
///
/// Names the reference's violations or the control that passes.
pub fn self_check(
    policy: &Policy,
    reference: &[Value],
    controls: &[Value],
) -> Result<Value, Refused> {
    // Each reference report must pass every criterion derived from it: one
    // without the oracle's router scores took no part in the router score
    // budget, so that rule alone isn't applied to it.
    // Exact checks are fixed equality rules, not derived from the reference
    // (and the CPU port records no Engram sites), so they don't apply here.
    let own: Vec<String> = reference
        .iter()
        .flat_map(|r| {
            let mut r = r.clone();
            for c in r["cases"].as_array_mut().into_iter().flatten() {
                c["exact"] = Value::Null;
            }
            if has_router_score_evidence(&r) {
                judge_report(policy, &r)
            } else {
                let mut without = policy.clone();
                without.router_score_budget = None;
                judge_report(&without, &r)
            }
        })
        .collect();
    if !own.is_empty() {
        return Err(refused(format!(
            "the reference fails the policy derived from it: {}",
            own.join("; ")
        )));
    }
    let mut detected = Vec::new();
    for c in controls {
        let failures = judge_report(policy, c);
        let name = c["control"].as_str().unwrap_or("unnamed");
        if failures.is_empty() {
            return Err(refused(format!(
                "the policy passes the {name} negative control"
            )));
        }
        // A control must fail on a numerical criterion, not only on missing
        // evidence.
        let numerical: Vec<&String> = failures
            .iter()
            .filter(|f| {
                ![
                    "no measured score error",
                    "no router evidence",
                    "missing",
                    "incomplete",
                    "malformed",
                    "pending",
                    "refused",
                    "exact checks",
                ]
                .iter()
                .any(|term| f.contains(term))
            })
            .collect();
        if numerical.is_empty() {
            return Err(refused(format!(
                "the {name} negative control fails only on missing evidence"
            )));
        }
        detected.push(json!({"control": name, "failures": failures.len(),
            "numerical_failures": numerical.len(), "first": numerical[0]}));
    }
    Ok(json!({"reference_passes": true, "controls": detected}))
}

/// Rule 2.2 (#12405 review): one aggregation unit, the case and mode, for
/// deriving and judging. Per-position limits are [`HEADROOM`] times the
/// reference's largest values; the case-mean limit [`HEADROOM`] times its
/// largest case mean; the tail threshold is the reference's pooled 90th
/// percentile total variation, with a fraction limit of [`HEADROOM`] times the
/// largest case-and-mode fraction above it (at least one position in 33, at
/// most half); the top-1 substitution fraction is [`HEADROOM`] times the
/// largest case-and-mode fraction (at least one in 33); router layers as in
/// 2.1. The derivation then runs [`self_check`].
///
/// # Errors
///
/// Missing reference positions, or a failed self-check.
pub fn derive_policy_v22(
    reference: &[Value],
    controls: &[Value],
) -> Result<(Policy, Value), Refused> {
    let (mut tv, mut js, mut lp, mut centered, mut case_mean) =
        (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut all_tv = Vec::new();
    let mut cases: Vec<(Vec<f64>, usize)> = Vec::new();
    for report in reference {
        for case in report["cases"].as_array().into_iter().flatten() {
            for mode in case["modes"].as_array().into_iter().flatten() {
                let ps = mode["positions"].as_array().cloned().unwrap_or_default();
                if ps.is_empty() {
                    continue;
                }
                let mut tvs = Vec::new();
                let mut subs = 0usize;
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
                    subs += usize::from(m["oracle_top1"] != m["candidate_top1"]);
                }
                case_mean = case_mean.max(tvs.iter().sum::<f64>() / tvs.len() as f64);
                all_tv.extend(tvs.iter().copied());
                cases.push((tvs, subs));
            }
        }
    }
    if cases.is_empty() || [tv, js, lp, centered].iter().any(|v| !v.is_finite()) {
        return Err(refused("the reference control has no complete positions"));
    }
    all_tv.sort_by(f64::total_cmp);
    let tail = all_tv[(all_tv.len() * 9 / 10).min(all_tv.len() - 1)];
    let floor = 1.0 / 33.0;
    // Router score budget: per layer, HEADROOM times the reference's largest
    // measured score error on the oracle's retained experts.
    let mut layer_error = vec![None::<f64>; LAYERS];
    let mut layer_rows = vec![0u64; LAYERS];
    let mut most_subs = 1usize;
    for report in reference {
        for case in report["cases"].as_array().into_iter().flatten() {
            for layer in case["router"]["layers"].as_array().into_iter().flatten() {
                let l = layer["layer"].as_u64().unwrap_or(u64::MAX) as usize;
                if let (Some(slot), Some(e)) =
                    (layer_error.get_mut(l), layer["score_error"]["max"].as_f64())
                {
                    *slot = Some(slot.map_or(e, |v: f64| v.max(e)));
                    layer_rows[l] += layer["score_error"]["rows"].as_u64().unwrap_or_default();
                }
                most_subs = most_subs
                    .max(layer["max_row_substitutions"].as_u64().unwrap_or_default() as usize);
            }
        }
    }
    let budget: Vec<f64> = layer_error
        .iter()
        .enumerate()
        .map(|(l, e)| {
            e.map(|e| HEADROOM * e)
                .ok_or_else(|| refused(format!("no reference router score error for layer {l}")))
        })
        .collect::<Result<_, _>>()?;
    let tail_fraction = cases
        .iter()
        .map(|(t, _)| t.iter().filter(|v| **v > tail).count() as f64 / t.len() as f64)
        .fold(0.0, f64::max);
    let sub_fraction = cases
        .iter()
        .map(|(t, s)| *s as f64 / t.len() as f64)
        .fold(0.0, f64::max);
    let policy = Policy {
        schema: q::POLICY_SCHEMA.into(),
        version: "2.2".into(),
        distribution: q::DistributionLimits {
            total_variation: Some(HEADROOM * tv),
            jensen_shannon: Some(HEADROOM * js),
            reference_logprob_error: Some(HEADROOM * lp),
            case_mean_total_variation: Some(HEADROOM * case_mean),
            tail_total_variation: Some(tail),
            tail_fraction: Some((HEADROOM * tail_fraction).clamp(floor, 0.5)),
        },
        top1: DecisionRule {
            epsilon: Some(centered),
            max_substitutions: 1,
            max_substituted_fraction: Some((HEADROOM * sub_fraction).max(floor)),
        },
        router: DecisionRule {
            epsilon: None,
            max_substitutions: most_subs,
            max_substituted_fraction: None,
        },
        index: DecisionRule {
            epsilon: None,
            max_substitutions: 2,
            max_substituted_fraction: None,
        },
        router_layer_mismatch: router_limits(reference)?,
        router_score_budget: Some(budget.clone()),
        index_out_of_scope: Some(
            "the pinned stage bundle records no index selections and the oracle dumps hold \
             none per row, so the index rule can't be evaluated; index effects reach this \
             policy only through the logit distribution checks"
                .into(),
        ),
    };
    let checks = self_check(&policy, reference, controls)?;
    let evidence = json!({
        "rule": "derive_policy_v22 in crates/psionic/crates/psionic-v41-stage/src/qualify.rs",
        "rationale": "Successor to 2.1 (owner choice B, 2026-10-22), frozen before its confirmation \
            set is captured. 2.1 rejected its own reference because it pooled positions for \
            derivation and judged per case; 2.2 uses one unit for both. Limits are HEADROOM times \
            what the CPU reference (cpu-bf16-tp8, a different implementation on different \
            hardware) shows against the B200 oracle: the portability envelope the launch \
            intends to support. The router rule is the review's margin and regret rule, with a \
            per-layer score budget measured on the reference in the selection function's units; \
            it is an empirical, connected budget, not a rigorous bound. h1-h6 are exposed and \
            count only as regression cases; the confirmation corpus is the generalization test.",
        "unit": "case and mode, for derivation and judging alike",
        "controls": "wrong-weights, wrong-routing, wrong-rope, stale-cache on p1 (pinned GPU \
            calibration); each must fail on a numerical criterion",
        "headroom": HEADROOM,
        "reference_positions": all_tv.len(),
        "reference_cases_and_modes": cases.len(),
        "reference_router_rows_with_scores_per_layer": layer_rows,
        "reference_router_score_error_max": layer_error,
        "reference_max_row_substitutions": most_subs,
        "reference_max": {"total_variation": tv, "jensen_shannon": js,
            "reference_logprob_error": lp, "centered_logit_error": centered,
            "case_mean_total_variation": case_mean, "tail_threshold_p90": tail,
            "tail_fraction": tail_fraction, "top1_substituted_fraction": sub_fraction},
        "self_check": checks,
    });
    Ok((policy, evidence))
}

/// Rule 2.3 (#12405, owner choice of 2026-10-22): rule 2.2's derivation
/// over a reference that isn't tuned to B200 arithmetic, the CPU port's
/// plain BF16 profile (`cpu-bf16`), with an oracle that keeps every router
/// score, so no changed expert set is unresolved for lack of evidence.
///
/// # Errors
///
/// A reference of another profile, or what [`derive_policy_v22`] refuses.
pub fn derive_policy_v23(
    reference: &[Value],
    controls: &[Value],
) -> Result<(Policy, Value), Refused> {
    for r in reference {
        if r["profile"] != json!("cpu-bf16") {
            return Err(refused(format!(
                "rule 2.3 takes the plain BF16 CPU reference, not {}",
                r["profile"]
            )));
        }
    }
    let (mut policy, mut evidence) = derive_policy_v22(reference, controls)?;
    policy.version = "2.3".into();
    evidence["rule"] =
        json!("derive_policy_v23 in crates/psionic/crates/psionic-v41-stage/src/qualify.rs");
    evidence["rationale"] = json!(
        "Successor to 2.1 and 2.2 (owner choice of 2026-10-22), frozen before any confirmation \
         data is read; the 2.1 and 2.2 failures stay on record. 2.2's reference, the bf16-tp8 \
         profile, reproduces B200 arithmetic (#12404) and so did not sample cross-hardware \
         variation, and with 14 retained router scores its own changed expert sets were \
         partly unresolved. 2.3's reference is the CPU port's plain BF16 profile (cpu-bf16, no \
         B200 emulation), a different implementation on different hardware from both the \
         oracle and the candidate. Its oracle keeps all 384 router scores. Limits are HEADROOM \
         times what that reference shows against the B200 oracle, judged per case and mode; \
         the router rule is the margin and regret rule with a per-layer score budget in the \
         selection function's units, an empirical connected budget rather than a rigorous \
         bound. h1-h6 are exposed and serve only as reference and regression cases; the \
         confirmation corpus is the generalization test."
    );
    evidence["reference_profiles"] = json!(["cpu-bf16"]);
    Ok((policy, evidence))
}

/// `qualify-judge --policy FILE --report R[,R...]`: every criterion each
/// report violates under the policy, as JSON lines.
///
/// # Errors
///
/// Unreadable files.
pub fn run_judge(args: &[String]) -> Result<(), Refused> {
    let flag = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let policy_path = flag("--policy").ok_or_else(|| refused("--policy is required"))?;
    let text = std::fs::read_to_string(&policy_path)
        .map_err(|e| refused(format!("{policy_path}: {e}")))?;
    let policy = Policy::parse(&text).map_err(|e| refused(e.to_string()))?;
    for path in flag("--report")
        .unwrap_or_default()
        .split(',')
        .filter(|p| !p.is_empty())
    {
        let bytes = std::fs::read(path).map_err(|e| refused(format!("{path}: {e}")))?;
        let report: Value =
            serde_json::from_slice(&bytes).map_err(|e| refused(format!("{path}: {e}")))?;
        let violations = judge_report(&policy, &report);
        println!("{}", json!({"report": path, "violations": violations}));
    }
    Ok(())
}

/// `qualify-derive --rule 2.1|2.2 --reference R[,R...] --control C[,C...] --out FILE`.
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
    let index_reference = read(flag("--index-reference"))?;
    let out = flag("--out").ok_or_else(|| refused("--out is required"))?;
    let (policy, evidence) = match flag("--rule").as_deref() {
        Some("2.1") => derive_policy(&reference, &controls)?,
        Some("2.2") => derive_policy_v22(&reference, &controls)?,
        Some("2.3") => derive_policy_v23(&reference, &controls)?,
        Some("2.4") => calibration::derive(&reference, &controls, &index_reference)?,
        _ => return Err(refused("--rule takes 2.1, 2.2, 2.3, or 2.4")),
    };
    let mut value = serde_json::to_value(&policy).map_err(|e| refused(e.to_string()))?;
    value["derivation"] = evidence;
    let mut input_files = BTreeMap::new();
    for key in ["--reference", "--control", "--index-reference"] {
        for path in flag(key)
            .unwrap_or_default()
            .split(',')
            .filter(|p| !p.is_empty())
        {
            input_files.insert(path.to_owned(), checked_sha256(Path::new(path))?);
        }
    }
    value["derivation"]["input_files_sha256"] = json!(input_files);
    value["launch_profile"] = json!({
        "prompt_tokens": 2872, "context": 8192, "output_cap": 4096,
        "deadline_seconds": 600, "step_seconds": 75, "prefill": "unchunked",
        "decided": "owner scope decision 91c68c4281, before any G1 v2 result",
    });
    // G2 v2's quality budget, frozen here before any v2 result.
    value["g2_quality"] = json!({
        "rule": "non-inferiority to the oracle: on the frozen task suite, the served runtime's \
            task pass count is at least the oracle's, and every individual failure is reported",
        "known_shared_failures": ["p2-code: fib(186) returns None although F(186) fits in u128; \
            it counts against both the oracle and the runtime and is listed in the report"],
        "decided": "decided by the owner's instruction to finish, 2026-10-21",
    });
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

    fn case_report(name: &str, positions: &[(f64, u32)], router: &[(usize, f64)]) -> Value {
        let ps: Vec<Value> = positions
            .iter()
            .map(|(tv, top)| {
                json!({"label": "p", "metrics": {"total_variation": tv, "jensen_shannon": tv / 10.0,
                "reference_logprob_error": tv, "centered_logit_error": 0.5,
                "oracle_top1": 5, "candidate_top1": top, "oracle_top1_gap": 0.1,
                "top1_regret": if *top == 5 { 0.0 } else { 0.05 }}})
            })
            .collect();
        let layers: Vec<Value> = (0..LAYERS)
            .map(|l| {
                let m = router
                    .iter()
                    .find(|(x, _)| *x == l)
                    .map_or(0.0, |(_, m)| *m);
                json!({"layer": l, "mismatch": m, "max_row_substitutions": 1,
                    "score_error": {"rows": 10, "max": 0.001 * (1.0 + l as f64)}})
            })
            .collect();
        json!({"case": name, "modes": [{"case": name, "mode": "decode", "positions": ps}],
            "router": {"layers": layers, "changes": [], "layers_with_candidate_scores": LAYERS}})
    }

    #[test]
    fn captured_router_scores_support_small_models_and_refuse_malformed_rows() {
        let root = tempfile::tempdir().unwrap();
        let name = "s.L0.RouterScores.u32".to_owned();
        let record = Recorded {
            replay_records: Vec::new(),
            files_sha256: BTreeMap::new(),
            source: Value::Null,
            logits: BTreeMap::new(),
            replay_last: None,
            records: vec![vec![name.clone()]],
            seconds: 0.0,
        };
        let write = |scores: &[f32]| {
            let bytes: Vec<u8> = scores
                .iter()
                .flat_map(|s| s.to_bits().to_le_bytes())
                .collect();
            std::fs::write(root.path().join(&name), bytes).unwrap();
        };
        write(&vec![0.5; 2 * 16]);
        let (source, scores) = candidate_router_scores(root.path(), &record, 0, 2, None)
            .unwrap()
            .unwrap();
        assert_eq!(source, "stage");
        assert_eq!(scores, vec![0.5; 32]);
        write(&vec![0.5; 31]);
        assert!(candidate_router_scores(root.path(), &record, 0, 2, None).is_err());
        write(&vec![f32::NAN; 32]);
        assert!(candidate_router_scores(root.path(), &record, 0, 2, None).is_err());
    }

    #[test]
    fn rule_2_1_rejects_its_own_reference_and_rule_2_2_does_not() {
        // A nine-position case with one substitution and a 33-position case
        // without: pooled, 1 in 42; per case, 1 in 9.
        let mut short = vec![(0.02, 5); 8];
        short.push((0.03, 6));
        let reference = json!({"cases": [
            case_report("short", &short, &[(1, 0.1)]),
            case_report("long", &vec![(0.01, 5); 33], &[(1, 0.05)]),
        ]});
        let control = json!({"control": "wrong-routing", "cases": [
            case_report("short", &vec![(0.02, 5); 9], &[(1, 0.9)])]});
        let (v21, _) = derive_policy(&[reference.clone()], &[control.clone()]).unwrap();
        let own = judge_report(&v21, &reference);
        assert!(
            own.iter().any(|f| f.contains("top-1 substitutions")),
            "{own:?}"
        );
        assert!(self_check(&v21, &[reference.clone()], &[control.clone()]).is_err());
        let (v22, evidence) = derive_policy_v22(&[reference.clone()], &[control.clone()]).unwrap();
        assert!(judge_report(&v22, &reference).is_empty());
        assert_eq!(evidence["self_check"]["reference_passes"], true);
        // A control the policy can't see is refused.
        let blind =
            json!({"control": "blind", "cases": [case_report("short", &short, &[(1, 0.1)])]});
        assert!(derive_policy_v22(&[reference], &[blind]).is_err());
    }

    #[test]
    fn the_router_margin_rule_judges_every_changed_row() {
        let reference = json!({"cases": [case_report("a", &vec![(0.01, 5); 33], &[])]});
        let control =
            json!({"control": "c", "cases": [case_report("a", &vec![(0.4, 5); 33], &[])]});
        let (policy, evidence) = derive_policy_v22(&[reference.clone()], &[control]).unwrap();
        let budget = policy.router_score_budget.clone().unwrap();
        assert!((budget[3] - HEADROOM * 0.004).abs() < 1e-12);
        assert_eq!(evidence["reference_max_row_substitutions"], 1);
        assert!(policy.index_out_of_scope.is_some());
        // A reference without the oracle's router scores joins the other
        // limits and is judged on them, not on the score budget.
        let mut unscored = case_report("b", &vec![(0.01, 5); 33], &[]);
        for l in unscored["router"]["layers"].as_array_mut().unwrap() {
            l["score_error"] = Value::Null;
        }
        let control =
            json!({"control": "c", "cases": [case_report("a", &vec![(0.4, 5); 33], &[])]});
        let both = [reference.clone(), json!({"cases": [unscored]})];
        assert_eq!(
            derive_policy_v22(&both, &[control])
                .unwrap()
                .0
                .router_score_budget,
            Some(budget.clone())
        );
        let eps = budget[0];
        let mut run = case_report("a", &vec![(0.01, 5); 33], &[]);
        run["router"]["changes"] = json!([
            {"layer": 0, "row": 1, "substitutions": 1, "oracle_gap": eps, "regret": eps},
            {"layer": 0, "row": 2, "substitutions": 1, "oracle_gap": 3.0 * eps, "regret": eps},
            {"layer": 0, "row": 3, "substitutions": 1, "oracle_gap": eps, "regret": 3.0 * eps},
            {"layer": 0, "row": 4, "substitutions": 1, "oracle_gap": eps, "regret": null},
            {"layer": 0, "row": 5, "substitutions": 2, "oracle_gap": eps, "regret": eps},
        ]);
        run["router"]["layers"][7]["score_error"]["max"] = json!(1.0);
        let v = judge_report(&policy, &json!({"cases": [run.clone()]}));
        for needle in [
            "stable boundary",
            "regret over",
            "unresolved",
            "substitution cap",
            "layer 7 score error",
        ] {
            assert!(v.iter().any(|f| f.contains(needle)), "{needle}: {v:?}");
        }
        // The first change alone is within the rule.
        run["router"]["changes"] = json!([{"layer": 0, "row": 1, "substitutions": 1,
            "oracle_gap": eps, "regret": eps}]);
        run["router"]["layers"][7]["score_error"]["max"] = json!(0.0);
        assert!(judge_report(&policy, &json!({"cases": [run.clone()]})).is_empty());
        run["router"]["layers"][7]["local_changed"] = json!(1);
        assert!(
            judge_report(&policy, &json!({"cases": [run.clone()]}))
                .iter()
                .any(|v| v.contains("do not reproduce 1 actual selections"))
        );
        run["router"]["layers"][7]["local_changed"] = json!(0);
        // Missing score evidence never passes.
        run["router"]["layers"][2]["score_error"] = Value::Null;
        assert!(!judge_report(&policy, &json!({"cases": [run]})).is_empty());
    }

    #[test]
    fn rule_2_3_takes_only_the_plain_bf16_reference() {
        let mut reference = json!({"profile": "cpu-bf16-tp8",
            "cases": [case_report("a", &vec![(0.01, 5); 33], &[])]});
        let control =
            json!({"control": "c", "cases": [case_report("a", &vec![(0.4, 5); 33], &[])]});
        assert!(derive_policy_v23(&[reference.clone()], &[control.clone()]).is_err());
        reference["profile"] = json!("cpu-bf16");
        let (policy, evidence) = derive_policy_v23(&[reference], &[control]).unwrap();
        assert_eq!(policy.version, "2.3");
        assert_eq!(evidence["reference_profiles"], json!(["cpu-bf16"]));
    }

    #[test]
    fn release_mode_refuses_missing_evidence() {
        let mut opts = QualifyOptions::parse(&[
            "--weights".into(),
            "w".into(),
            "--manifest".into(),
            "m".into(),
            "--oracle".into(),
            "o".into(),
            "--out".into(),
            "x".into(),
            "--development".into(),
            "--release".into(),
        ])
        .unwrap();
        let empty = json!({"case": "c", "modes": [{"mode": "decode", "positions": [], "missing": ["g1"]}],
            "router": null, "exact": null});
        assert!(
            release_refusals(&opts, &pending_policy(), &[])
                .iter()
                .any(|r| r.contains("at least one case"))
        );
        let refusals = release_refusals(&opts, &pending_policy(), &[empty]);
        for needle in [
            "--observe",
            "pinned bundle",
            "no router report",
            "no exact checks",
            "missing positions",
            "no positions",
            "index rule not evaluated",
        ] {
            assert!(
                refusals.iter().any(|r| r.contains(needle)),
                "{needle}: {refusals:?}"
            );
        }
        opts.observe = true;
        opts.control = Some(Control::WrongWeights);
        opts.arithmetic = Some(crate::CUDA_FP8_REFERENCE.into());
        let partial = json!({"case": "c", "modes": [], "router": {"layers_compared": 39,
            "margin_verdict": false, "unresolved": 3, "failed": 0}, "exact": {"checked": 2}});
        let refusals = release_refusals(&opts, &pending_policy(), &[partial]);
        assert!(refusals.iter().any(|r| r.contains("deliberate control")));
        assert!(refusals.iter().any(|r| r.contains("diagnostics cannot")));
        assert!(
            refusals
                .iter()
                .any(|r| r.contains("both decode and replay"))
        );
        assert!(refusals.iter().any(|r| r.contains("39 of 40")));
        assert!(refusals.iter().any(|r| r.contains("3 unresolved")));
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
