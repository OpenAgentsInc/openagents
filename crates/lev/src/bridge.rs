//! The seam to Apple's runtime.
//!
//! A supervised child process speaks line-delimited JSON on its stdin and
//! stdout: one request object per line, one response object per line, with
//! diagnostics on stderr. Earlier bridges in this workspace hand-parsed HTTP
//! on a socket, and the audit of the first one called that naive and
//! truncation-prone. A line protocol removes the surface.
//!
//! Every decision builds its own session inside the helper, so no question
//! can read another question's text. That isolation is structural, and
//! `tests/isolation.rs` proves it with a planted secret rather than trusting
//! the claim.
//!
//! The helper is a child process, and a child process can hang, flood its
//! diagnostics, answer with the wrong line, or exit. A [`Bridge`] therefore
//! owns a reader thread for stdout and a drainer thread for stderr, caps the
//! size of one response, checks the response `id` against the request it
//! sent, and gives every exchange a deadline. A helper that misses any of
//! those is retired — killed and reaped — and the next call through its
//! [`Pool`] lane starts a fresh one. `tests/supervision.rs` drives each of
//! those faults through a fake helper.

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Refusal, RefusalCode, Result};
use crate::schema::Compiled;

/// Which sampling mode a call runs under.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Sampling {
    /// Deterministic highest-probability selection.
    Greedy,
    /// Seeded random sampling, which is what makes an ensemble reproducible.
    Random {
        /// The seed this sample ran under.
        seed: u64,
        /// An optional temperature.
        #[serde(skip_serializing_if = "Option::is_none")]
        temperature: Option<f64>,
    },
}

/// One call to the runtime.
#[derive(Clone, Debug, Serialize)]
pub struct Call {
    id: String,
    op: &'static str,
    instructions: String,
    prompt: String,
    options: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    band: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    use_case: Option<&'static str>,
    sampling: Sampling,
    #[serde(rename = "adapterPath", skip_serializing_if = "Option::is_none")]
    adapter_path: Option<String>,
}

impl Call {
    /// The correlation id the helper echoes back.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Builds a decision call from a compiled question.
    #[must_use]
    pub fn decide(compiled: &Compiled, sampling: Sampling) -> Self {
        Self {
            id: next_id(),
            op: "decide",
            instructions: compiled.instructions.clone(),
            prompt: compiled.prompt.clone(),
            options: compiled.options.clone(),
            band: None,
            use_case: Some("content_tagging"),
            sampling,
            adapter_path: None,
        }
    }

    /// Attaches a `.fmadapter` package for this call.
    ///
    /// The runtime takes an adapter or a use case, not both, so attaching one
    /// drops the content-tagging declaration.
    #[must_use]
    pub fn with_adapter(mut self, path: impl Into<String>) -> Self {
        self.adapter_path = Some(path.into());
        self.use_case = None;
        self
    }

    /// Asks for an ordered certainty band alongside the choice.
    #[must_use]
    pub fn with_band(mut self, band: Vec<String>) -> Self {
        self.band = Some(band);
        self
    }
}

/// What one call returned.
#[derive(Clone, Debug, Deserialize)]
pub struct Outcome {
    /// The admitted option the runtime selected.
    pub choice: Option<String>,
    /// The certainty band, when one was asked for.
    pub band: Option<String>,
    /// How long the call took inside the helper.
    pub latency_ms: Option<f64>,
}

/// How the model reports its own availability.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Availability {
    /// `available`, `unavailable`, or `unknown`.
    pub status: String,
    /// Why, when it is unavailable.
    pub reason: Option<String>,
}

impl Availability {
    /// Whether the runtime will answer at all.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.status == "available"
    }
}

#[derive(Deserialize)]
struct Wire {
    #[serde(default)]
    id: Option<String>,
    ok: bool,
    #[serde(default)]
    availability: Option<Availability>,
    #[serde(default)]
    choice: Option<String>,
    #[serde(default)]
    band: Option<String>,
    #[serde(default, rename = "latencyMs")]
    latency_ms: Option<f64>,
    #[serde(default, rename = "compatibleAdapters")]
    compatible_adapters: Option<Vec<String>>,
    #[serde(default, rename = "adapterMetadata")]
    adapter_metadata: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    error: Option<WireError>,
}

#[derive(Deserialize)]
struct WireError {
    code: String,
    message: String,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> String {
    format!("c{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

/// The most bytes one response line may carry.
///
/// A response is one JSON object holding a choice, a band, and a latency, or
/// an availability report, or a short list of adapter identifiers. A megabyte
/// is orders of magnitude above any of them; a line that reaches it is a
/// helper that has stopped speaking the protocol.
pub const MAX_RESPONSE_BYTES: usize = 1 << 20;

/// How many bytes of the helper's most recent diagnostics are kept.
pub const STDERR_TAIL_BYTES: usize = 4 << 10;

/// The environment variable that sets the per-exchange deadline in
/// milliseconds.
pub const DEADLINE_VAR: &str = "LEV_BRIDGE_DEADLINE_MS";

/// The per-exchange deadline when [`DEADLINE_VAR`] is unset.
///
/// A decision call on the device runs in a few hundred milliseconds and an
/// adapter load in a few seconds; thirty seconds is a hung helper, not a slow
/// one.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(30);

/// The deadline one exchange runs under, from the environment or the default.
#[must_use]
pub fn deadline() -> Duration {
    std::env::var(DEADLINE_VAR)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map_or(DEFAULT_DEADLINE, Duration::from_millis)
}

/// The last [`STDERR_TAIL_BYTES`] the helper wrote to stderr.
#[derive(Default)]
struct Tail {
    bytes: VecDeque<u8>,
}

impl Tail {
    fn push(&mut self, chunk: &[u8]) {
        for byte in chunk {
            if self.bytes.len() == STDERR_TAIL_BYTES {
                self.bytes.pop_front();
            }
            self.bytes.push_back(*byte);
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes.iter().copied().collect::<Vec<_>>())
            .trim()
            .to_string()
    }
}

/// What the reader thread hands back for one line of stdout.
enum Frame {
    Line(String),
    /// The line passed [`MAX_RESPONSE_BYTES`] without ending.
    Oversized,
    /// Stdout closed or failed.
    Closed(Option<io::Error>),
}

/// Reads stdout one capped line at a time and forwards each to the bridge.
fn read_frames(mut stdout: impl BufRead, frames: &mpsc::Sender<Frame>) {
    loop {
        let mut line = Vec::new();
        let read = stdout
            .by_ref()
            .take(MAX_RESPONSE_BYTES as u64 + 1)
            .read_until(b'\n', &mut line);
        let frame = match read {
            Ok(0) => Frame::Closed(None),
            Ok(_) if line.last() != Some(&b'\n') && line.len() > MAX_RESPONSE_BYTES => {
                Frame::Oversized
            }
            Ok(_) if line.last() != Some(&b'\n') => Frame::Closed(None),
            Ok(_) => Frame::Line(String::from_utf8_lossy(&line).into_owned()),
            Err(error) => Frame::Closed(Some(error)),
        };
        let last = !matches!(frame, Frame::Line(_));
        if frames.send(frame).is_err() || last {
            return;
        }
    }
}

/// How long a retirement waits for the drainer to reach the end of stderr.
///
/// A killed helper's pipe closes at once; a grandchild it left behind can
/// hold the pipe open, and a retirement does not wait on that.
const STDERR_SETTLE: Duration = Duration::from_millis(250);

/// Drains stderr so the helper never blocks on a full pipe, keeping a tail,
/// and reports through `done` when the pipe closes.
fn drain_stderr(mut stderr: impl Read, tail: &Mutex<Tail>, done: &mpsc::Sender<()>) {
    let mut buffer = [0_u8; 4096];
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) | Err(_) => {
                let _ = done.send(());
                return;
            }
            Ok(read) => tail
                .lock()
                .expect("the stderr tail lock is not poisoned")
                .push(&buffer[..read]),
        }
    }
}

/// A supervised helper process.
pub struct Bridge {
    child: Child,
    stdin: ChildStdin,
    frames: Receiver<Frame>,
    stderr: Arc<Mutex<Tail>>,
    stderr_done: Receiver<()>,
    deadline: Duration,
    retired: bool,
}

impl Bridge {
    /// Starts the helper at `path`, verifying its code signature first.
    ///
    /// The signature check is not decoration. The helper reaches the
    /// on-device model on the owner's behalf, and an earlier lane in this
    /// workspace only made the signature authoritative after the integration
    /// was already shipping.
    ///
    /// Exchanges run under [`deadline`].
    pub fn start(path: &Path) -> Result<Self> {
        Self::start_with_deadline(path, deadline())
    }

    /// Starts the helper at `path` with an explicit per-exchange deadline.
    pub fn start_with_deadline(path: &Path, deadline: Duration) -> Result<Self> {
        verify_signature(path)?;
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                Refusal::new(
                    RefusalCode::BridgeError,
                    format!("the helper at {} did not start: {error}", path.display()),
                )
            })?;
        let stdin = child.stdin.take().expect("stdin was piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout was piped"));
        let stderr_pipe = child.stderr.take().expect("stderr was piped");
        let (sender, frames) = mpsc::channel();
        std::thread::Builder::new()
            .name("lev-bridge-stdout".to_string())
            .spawn(move || read_frames(stdout, &sender))
            .map_err(|error| {
                Refusal::new(
                    RefusalCode::BridgeError,
                    format!("the helper's reader thread did not start: {error}"),
                )
            })?;
        let stderr = Arc::new(Mutex::new(Tail::default()));
        let tail = Arc::clone(&stderr);
        let (done, stderr_done) = mpsc::channel();
        std::thread::Builder::new()
            .name("lev-bridge-stderr".to_string())
            .spawn(move || drain_stderr(stderr_pipe, &tail, &done))
            .map_err(|error| {
                Refusal::new(
                    RefusalCode::BridgeError,
                    format!("the helper's stderr drainer did not start: {error}"),
                )
            })?;
        Ok(Self {
            child,
            stdin,
            frames,
            stderr,
            stderr_done,
            deadline,
            retired: false,
        })
    }

    /// Whether a fault retired this helper, so a caller should replace it.
    #[must_use]
    pub fn is_retired(&self) -> bool {
        self.retired
    }

    /// The most recent diagnostics the helper wrote to stderr.
    #[must_use]
    pub fn stderr_tail(&self) -> String {
        self.stderr
            .lock()
            .expect("the stderr tail lock is not poisoned")
            .text()
    }

    /// Kills and reaps the helper and marks it unusable, returning a refusal
    /// that carries `why` and the diagnostics tail.
    fn retire(&mut self, why: String) -> Refusal {
        self.retired = true;
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = self.stderr_done.recv_timeout(STDERR_SETTLE);
        let tail = self.stderr_tail();
        let message = if tail.is_empty() {
            why
        } else {
            format!("{why}; the helper's last diagnostics: {tail}")
        };
        Refusal::new(RefusalCode::BridgeError, message)
    }

    /// Starts the helper found by `helper_path`.
    pub fn discover() -> Result<Self> {
        Self::start(&helper_path()?)
    }

    /// Asks the runtime whether it will answer.
    pub fn availability(&mut self) -> Result<Availability> {
        #[derive(Serialize)]
        struct Ask {
            id: String,
            op: &'static str,
        }
        let id = next_id();
        let wire = self.exchange(
            &id,
            &Ask {
                id: id.clone(),
                op: "availability",
            },
        )?;
        wire.availability.ok_or_else(|| {
            Refusal::new(
                RefusalCode::BridgeError,
                "the helper reported no availability",
            )
        })
    }

    /// Asks the running base which adapter identifiers it accepts for a name.
    ///
    /// The identifiers come back as `fmadapter-<name>-<signature prefix>`, so
    /// this is how the live base model signature is read from outside. A
    /// package can then be checked against the device before it is attached.
    pub fn compatible_adapters(&mut self, name: &str) -> Result<Vec<String>> {
        #[derive(Serialize)]
        struct Ask<'a> {
            id: String,
            op: &'static str,
            #[serde(rename = "adapterName")]
            adapter_name: &'a str,
        }
        let id = next_id();
        let wire = self.exchange(
            &id,
            &Ask {
                id: id.clone(),
                op: "adapter_compat",
                adapter_name: name,
            },
        )?;
        wire.compatible_adapters.ok_or_else(|| {
            Refusal::new(
                RefusalCode::BridgeError,
                "the helper reported no adapter identifiers",
            )
        })
    }

    /// The base model signature prefix the device runs, read from the
    /// identifiers it will accept.
    pub fn base_signature_prefix(&mut self) -> Result<String> {
        let identifiers = self.compatible_adapters("lev")?;
        identifiers
            .first()
            .and_then(|id| id.rsplit('-').next())
            .map(str::to_string)
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::BridgeError,
                    "no identifier carried a signature prefix",
                )
            })
    }

    /// Asks the runtime to load a package, returning its producer metadata.
    pub fn load_adapter(
        &mut self,
        path: &Path,
    ) -> Result<std::collections::BTreeMap<String, String>> {
        #[derive(Serialize)]
        struct Ask {
            id: String,
            op: &'static str,
            #[serde(rename = "adapterPath")]
            adapter_path: String,
        }
        let id = next_id();
        let wire = self.exchange(
            &id,
            &Ask {
                id: id.clone(),
                op: "adapter_load",
                adapter_path: path.display().to_string(),
            },
        )?;
        Ok(wire.adapter_metadata.unwrap_or_default())
    }

    /// Runs one decision call.
    pub fn decide(&mut self, call: &Call) -> Result<Outcome> {
        let wire = self.exchange(&call.id, call)?;
        let choice = wire.choice.ok_or_else(|| {
            Refusal::new(
                RefusalCode::DecodingFailure,
                "the helper returned no choice",
            )
        })?;
        Ok(Outcome {
            choice: Some(choice),
            band: wire.band,
            latency_ms: wire.latency_ms,
        })
    }

    fn exchange<T: Serialize>(&mut self, id: &str, request: &T) -> Result<Wire> {
        if self.retired {
            return Err(Refusal::new(
                RefusalCode::BridgeError,
                "the helper was retired after an earlier fault",
            ));
        }
        let line = serde_json::to_string(request).map_err(|error| {
            Refusal::new(
                RefusalCode::BridgeError,
                format!("request did not encode: {error}"),
            )
        })?;
        if let Err(error) = writeln!(self.stdin, "{line}").and_then(|()| self.stdin.flush()) {
            return Err(self.retire(format!("the helper closed its input: {error}")));
        }

        let response = match self.frames.recv_timeout(self.deadline) {
            Ok(Frame::Line(line)) => line,
            Ok(Frame::Oversized) => {
                return Err(self.retire(format!(
                    "the helper answered with a line over {MAX_RESPONSE_BYTES} bytes"
                )));
            }
            Ok(Frame::Closed(Some(error))) => {
                return Err(self.retire(format!("the helper closed its output: {error}")));
            }
            Ok(Frame::Closed(None)) | Err(RecvTimeoutError::Disconnected) => {
                return Err(self.retire("the helper exited".to_string()));
            }
            Err(RecvTimeoutError::Timeout) => {
                return Err(self.retire(format!(
                    "the helper did not answer within {} ms",
                    self.deadline.as_millis()
                )));
            }
        };
        let wire: Wire = match serde_json::from_str(response.trim()) {
            Ok(wire) => wire,
            Err(error) => {
                return Err(self.retire(format!("the helper answered with {error}")));
            }
        };
        if wire.id.as_deref() != Some(id) {
            return Err(self.retire(format!(
                "the helper answered call {} when {id} was asked",
                wire.id.as_deref().unwrap_or("with no id")
            )));
        }
        if wire.ok {
            return Ok(wire);
        }
        let error = wire.error.unwrap_or(WireError {
            code: "unknown".to_string(),
            message: "the helper refused without a reason".to_string(),
        });
        Err(Refusal::from_bridge(&error.code, error.message))
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Where the helper binary is, by environment override or by build location.
pub fn helper_path() -> Result<PathBuf> {
    if let Some(explicit) = std::env::var_os("LEV_BRIDGE_BIN") {
        let path = PathBuf::from(explicit);
        return if path.exists() {
            Ok(path)
        } else {
            Err(Refusal::new(
                RefusalCode::BridgeError,
                format!(
                    "LEV_BRIDGE_BIN points at {}, which does not exist",
                    path.display()
                ),
            ))
        };
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::BridgeError,
                "the workspace root is not above this crate",
            )
        })?;
    let built = root.join("swift/lev-bridge/.build/release/lev-bridge");
    if built.exists() {
        return Ok(built);
    }
    Err(Refusal::new(
        RefusalCode::BridgeError,
        format!(
            "no helper at {}. Build it with ./scripts/build-lev-bridge.sh, or set LEV_BRIDGE_BIN",
            built.display()
        ),
    ))
}

/// Refuses a helper whose code signature does not check out.
fn verify_signature(path: &Path) -> Result<()> {
    if std::env::var_os("LEV_BRIDGE_ALLOW_UNSIGNED").is_some() {
        return Ok(());
    }
    let output = Command::new("/usr/bin/codesign")
        .arg("--verify")
        .arg("--strict")
        .arg(path)
        .output()
        .map_err(|error| {
            Refusal::new(
                RefusalCode::BridgeError,
                format!("codesign did not run: {error}"),
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    Err(Refusal::new(
        RefusalCode::BridgeError,
        format!(
            "the helper at {} failed signature verification: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    ))
}

/// A pool of helper processes.
///
/// One helper answers one call at a time, because the line protocol is a
/// request and a response on one pair of pipes. An `N`-sample ensemble
/// through a single helper therefore serializes, which is where Lev's
/// latency went: eight samples at about 260 ms each is a little over two
/// seconds.
///
/// Sessions are already independent — that is what gives question isolation
/// — so nothing about a sample depends on the helper that drew it. Spreading
/// the draws over `k` helpers divides the wall clock by `k`.
///
/// Each lane owns one helper. A helper that a fault retires is replaced the
/// next time its lane is used, so one hung or crashed process costs the
/// calls it was holding and nothing after them.
pub struct Pool {
    lanes: Vec<Lane>,
}

/// One helper's slot in the pool: where to start it from, and the running
/// process when there is one.
struct Lane {
    path: PathBuf,
    deadline: Duration,
    helper: Mutex<Option<Bridge>>,
}

impl Lane {
    /// Runs `f` on this lane's helper, starting or replacing it first when
    /// the lane is empty or the last call retired it.
    fn with<T>(&self, f: impl FnOnce(&mut Bridge) -> Result<T>) -> Result<T> {
        let mut slot = self.helper.lock().expect("a helper lock is not poisoned");
        if slot.as_ref().is_none_or(Bridge::is_retired) {
            *slot = Some(Bridge::start_with_deadline(&self.path, self.deadline)?);
        }
        let helper = slot.as_mut().expect("the slot was just filled");
        f(helper)
    }
}

impl Pool {
    /// A pool with no helpers.
    ///
    /// A door over one answers no question and says `model_unavailable`,
    /// which is what makes it useful: a check that must hold whether or not
    /// the device is reachable can be exercised against a door that certainly
    /// cannot reach it. A revocation is such a check — a withdrawn release
    /// has to refuse for the reason it was withdrawn, not for whatever the
    /// runtime happens to report.
    #[must_use]
    pub fn none() -> Self {
        Self { lanes: Vec::new() }
    }

    /// Starts `size` helpers from the discovered path.
    pub fn discover(size: usize) -> Result<Self> {
        Self::start(&helper_path()?, size, deadline())
    }

    /// Starts `size` helpers from `path`, each exchange under `deadline`.
    ///
    /// Every helper is started here rather than on first use, so a pool that
    /// cannot start is refused before a door is built over it.
    pub fn start(path: &Path, size: usize, deadline: Duration) -> Result<Self> {
        let mut lanes = Vec::with_capacity(size.max(1));
        for _ in 0..size.max(1) {
            let helper = Bridge::start_with_deadline(path, deadline)?;
            lanes.push(Lane {
                path: path.to_path_buf(),
                deadline,
                helper: Mutex::new(Some(helper)),
            });
        }
        Ok(Self { lanes })
    }

    /// How many helpers the pool holds.
    #[must_use]
    pub fn width(&self) -> usize {
        self.lanes.len()
    }

    /// Asks the first helper for the base model signature prefix the device
    /// runs.
    ///
    /// This is how a door with no adapter attached can still say what it is
    /// running, which is what a calibration record has to match before it may
    /// serve.
    pub fn base_signature_prefix(&self) -> Result<String> {
        self.first()?.with(Bridge::base_signature_prefix)
    }

    /// Asks the first helper whether the runtime will answer.
    pub fn availability(&self) -> Result<Availability> {
        self.first()?.with(Bridge::availability)
    }

    /// The first lane, or the refusal an empty pool answers with.
    fn first(&self) -> Result<&Lane> {
        self.lanes
            .first()
            .ok_or_else(|| Refusal::new(RefusalCode::ModelUnavailable, "this door holds no helper"))
    }

    /// Runs `calls` across the pool, preserving input order.
    ///
    /// A call that the runtime refuses comes back as its refusal rather than
    /// failing the batch, so a caller can decide what a partial result means.
    /// A call whose helper is retired mid-batch comes back as that fault; the
    /// lane's remaining calls run on a fresh helper.
    pub fn decide_all(&self, calls: &[Call]) -> Vec<Result<Outcome>> {
        let mut results: Vec<Option<Result<Outcome>>> = (0..calls.len()).map(|_| None).collect();
        let width = self.lanes.len();
        if width == 0 {
            return calls
                .iter()
                .map(|_| {
                    Err(Refusal::new(
                        RefusalCode::ModelUnavailable,
                        "this door holds no helper",
                    ))
                })
                .collect();
        }
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for (index, lane) in self.lanes.iter().enumerate() {
                let slice: Vec<(usize, &Call)> = calls
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| position % width == index)
                    .collect();
                handles.push(scope.spawn(move || {
                    slice
                        .into_iter()
                        .map(|(position, call)| (position, lane.with(|helper| helper.decide(call))))
                        .collect::<Vec<_>>()
                }));
            }
            for handle in handles {
                for (position, outcome) in handle.join().expect("a lane finished") {
                    results[position] = Some(outcome);
                }
            }
        });
        results
            .into_iter()
            .map(|slot| {
                slot.unwrap_or_else(|| {
                    Err(Refusal::new(
                        RefusalCode::BridgeError,
                        "a lane dropped a call",
                    ))
                })
            })
            .collect()
    }
}
