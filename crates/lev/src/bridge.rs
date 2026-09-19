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

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

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

/// A supervised helper process.
pub struct Bridge {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Bridge {
    /// Starts the helper at `path`, verifying its code signature first.
    ///
    /// The signature check is not decoration. The helper reaches the
    /// on-device model on the owner's behalf, and an earlier lane in this
    /// workspace only made the signature authoritative after the integration
    /// was already shipping.
    pub fn start(path: &Path) -> Result<Self> {
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
        Ok(Self { child, stdin, stdout })
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
        let wire = self.exchange(&Ask { id: next_id(), op: "availability" })?;
        wire.availability.ok_or_else(|| {
            Refusal::new(RefusalCode::BridgeError, "the helper reported no availability")
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
        let wire = self.exchange(&Ask { id: next_id(), op: "adapter_compat", adapter_name: name })?;
        wire.compatible_adapters.ok_or_else(|| {
            Refusal::new(RefusalCode::BridgeError, "the helper reported no adapter identifiers")
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
                Refusal::new(RefusalCode::BridgeError, "no identifier carried a signature prefix")
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
        let wire = self.exchange(&Ask {
            id: next_id(),
            op: "adapter_load",
            adapter_path: path.display().to_string(),
        })?;
        Ok(wire.adapter_metadata.unwrap_or_default())
    }

    /// Runs one decision call.
    pub fn decide(&mut self, call: &Call) -> Result<Outcome> {
        let wire = self.exchange(call)?;
        let choice = wire.choice.ok_or_else(|| {
            Refusal::new(RefusalCode::DecodingFailure, "the helper returned no choice")
        })?;
        Ok(Outcome { choice: Some(choice), band: wire.band, latency_ms: wire.latency_ms })
    }

    fn exchange<T: Serialize>(&mut self, request: &T) -> Result<Wire> {
        let line = serde_json::to_string(request).map_err(|error| {
            Refusal::new(RefusalCode::BridgeError, format!("request did not encode: {error}"))
        })?;
        writeln!(self.stdin, "{line}").and_then(|()| self.stdin.flush()).map_err(|error| {
            Refusal::new(RefusalCode::BridgeError, format!("the helper closed its input: {error}"))
        })?;

        let mut response = String::new();
        let read = self.stdout.read_line(&mut response).map_err(|error| {
            Refusal::new(RefusalCode::BridgeError, format!("the helper closed its output: {error}"))
        })?;
        if read == 0 {
            return Err(Refusal::new(RefusalCode::BridgeError, "the helper exited"));
        }
        let wire: Wire = serde_json::from_str(response.trim()).map_err(|error| {
            Refusal::new(RefusalCode::BridgeError, format!("the helper answered with {error}"))
        })?;
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
                format!("LEV_BRIDGE_BIN points at {}, which does not exist", path.display()),
            ))
        };
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| Refusal::new(RefusalCode::BridgeError, "the workspace root is not above this crate"))?;
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
            Refusal::new(RefusalCode::BridgeError, format!("codesign did not run: {error}"))
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
pub struct Pool {
    helpers: Vec<std::sync::Mutex<Bridge>>,
}

impl Pool {
    /// Starts `size` helpers from the discovered path.
    pub fn discover(size: usize) -> Result<Self> {
        let path = helper_path()?;
        let mut helpers = Vec::with_capacity(size.max(1));
        for _ in 0..size.max(1) {
            helpers.push(std::sync::Mutex::new(Bridge::start(&path)?));
        }
        Ok(Self { helpers })
    }

    /// How many helpers the pool holds.
    #[must_use]
    pub fn width(&self) -> usize {
        self.helpers.len()
    }

    /// Asks the first helper for the base model signature prefix the device
    /// runs.
    ///
    /// This is how a door with no adapter attached can still say what it is
    /// running, which is what a calibration record has to match before it may
    /// serve.
    pub fn base_signature_prefix(&self) -> Result<String> {
        let mut helper = self.helpers[0].lock().expect("a helper lock is not poisoned");
        helper.base_signature_prefix()
    }

    /// Asks the first helper whether the runtime will answer.
    pub fn availability(&self) -> Result<Availability> {
        let mut helper = self.helpers[0].lock().expect("a helper lock is not poisoned");
        helper.availability()
    }

    /// Runs `calls` across the pool, preserving input order.
    ///
    /// A call that the runtime refuses comes back as its refusal rather than
    /// failing the batch, so a caller can decide what a partial result means.
    pub fn decide_all(&self, calls: &[Call]) -> Vec<Result<Outcome>> {
        let mut results: Vec<Option<Result<Outcome>>> = (0..calls.len()).map(|_| None).collect();
        let width = self.helpers.len();
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for (lane, helper) in self.helpers.iter().enumerate() {
                let slice: Vec<(usize, &Call)> = calls
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| index % width == lane)
                    .collect();
                handles.push(scope.spawn(move || {
                    let mut helper = helper.lock().expect("a helper lock is not poisoned");
                    slice
                        .into_iter()
                        .map(|(index, call)| (index, helper.decide(call)))
                        .collect::<Vec<_>>()
                }));
            }
            for handle in handles {
                for (index, outcome) in handle.join().expect("a lane finished") {
                    results[index] = Some(outcome);
                }
            }
        });
        results
            .into_iter()
            .map(|slot| {
                slot.unwrap_or_else(|| {
                    Err(Refusal::new(RefusalCode::BridgeError, "a lane dropped a call"))
                })
            })
            .collect()
    }
}
