//! One fresh guest instance per invocation.
//!
//! The host copies the packet in, calls `oa_handle`, copies the output out,
//! and frees both allocations. A trap discards the instance. No ambient
//! interface is linked.
//!
//! Setting the call's cancel flag stops a running guest: a watcher thread
//! bumps the engine's epoch, and the guest traps at its next loop header or
//! function entry. The host call path checks the same flag.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nostr::contracts::parse_strict;
use serde_json::{Map, Value, json};
use wasmtime::{Caller, Engine, Linker, Memory, Module, Store, StoreLimits, Trap};

use crate::memory::{check_range, overlaps};
use crate::snapshot::Snapshot;

/// How often the watcher thread checks the cancel flag while a guest runs.
const CANCEL_POLL: Duration = Duration::from_millis(5);

/// Which guest profile this invocation grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// The packet is the only input. Host calls are not linked.
    Pure,
    /// The guest may list and read the granted snapshot.
    SnapshotRead,
}

/// Ceilings for one invocation.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Guest instruction fuel, including the start function.
    pub fuel: u64,
    /// Maximum guest linear memory.
    pub memory_bytes: usize,
    /// Maximum response body.
    pub output_bytes: usize,
    /// Maximum snapshot bytes one invocation may read.
    pub read_bytes: usize,
    /// Maximum guest module size accepted for compilation.
    pub module_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel: 1_000_000,
            memory_bytes: 8 * 1024 * 1024,
            output_bytes: 65_536,
            read_bytes: 65_536,
            module_bytes: 2 * 1024 * 1024,
        }
    }
}

/// Why an invocation did not return a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// The module or the packet is not the ABI.
    Malformed(String),
    /// The profile denies the request.
    Denied(String),
    /// A ceiling was crossed.
    Limit(String),
    /// A handle or version is not this invocation's.
    Stale(String),
    /// The guest trapped or the host could not finish.
    Failed(String),
    /// The caller cancelled the invocation.
    Cancelled,
    /// A required guest refusal.
    Refused(String),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(detail)
            | Self::Denied(detail)
            | Self::Limit(detail)
            | Self::Stale(detail)
            | Self::Failed(detail)
            | Self::Refused(detail) => {
                write!(formatter, "{detail}")
            }
            Self::Cancelled => write!(formatter, "cancelled"),
        }
    }
}

impl std::error::Error for HostError {}

/// What the guest returned, with verification left unset.
#[derive(Debug, Clone, PartialEq)]
pub struct GuestValue {
    /// `ok` after a successful call, or `ok` for an optional fallback.
    pub status: String,
    /// The value, or the bounded fallback.
    pub value: Value,
    /// Always `not_run`. The guest does not verify its own output.
    pub verification: &'static str,
}

struct GuestState {
    snapshot: Snapshot,
    /// Opaque token to logical entry name.
    handles: BTreeMap<String, String>,
    memory: Option<Memory>,
    read_left: usize,
    output_left: usize,
    /// Child handles minted by `list` so far, for unique tokens.
    minted: usize,
    cancelled: Arc<AtomicBool>,
    store_limits: StoreLimits,
}

/// One guest invocation.
pub struct Call<'a> {
    /// Guest module bytes.
    pub wasm: &'a [u8],
    /// Pure or snapshot-read.
    pub profile: Profile,
    /// Host-generated invocation id.
    pub invocation: &'a str,
    /// Operation slug.
    pub operation: &'a str,
    /// Operation input.
    pub input: &'a Value,
    /// Snapshot the guest may read.
    pub snapshot: &'a Snapshot,
    /// Logical name to opaque handle token.
    pub handles: &'a BTreeMap<String, String>,
    /// Ceilings.
    pub limits: Limits,
    /// Set when the caller cancels the invocation. The host checks it
    /// before the guest starts, at each host call, and from a watcher
    /// thread that interrupts a running guest.
    pub cancelled: Arc<AtomicBool>,
    /// Whether a guest refusal fails the invocation.
    pub required: bool,
}

/// Run one guest.
///
/// # Errors
///
/// Returns a typed host error. The instance is not reused after a trap.
pub fn invoke(call: Call<'_>) -> Result<GuestValue, HostError> {
    if call.wasm.len() > call.limits.module_bytes {
        return Err(HostError::Limit("module bytes".into()));
    }
    if call.cancelled.load(Ordering::SeqCst) {
        return Err(HostError::Cancelled);
    }
    let mut config = wasmtime::Config::new();
    config.consume_fuel(true);
    config.epoch_interruption(true);
    let engine = Engine::new(&config).map_err(|error| HostError::Failed(error.to_string()))?;
    let cancelled = Arc::clone(&call.cancelled);
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let watcher = scope.spawn(|| interrupt_on_cancel(&engine, &cancelled, &done));
        let outcome = run_guest(&engine, call);
        done.store(true, Ordering::SeqCst);
        watcher.thread().unpark();
        outcome
    })
}

/// Bumps the engine's epoch once the cancel flag is set, which traps a
/// running guest, and returns when the invocation is done.
fn interrupt_on_cancel(engine: &Engine, cancelled: &AtomicBool, done: &AtomicBool) {
    while !done.load(Ordering::SeqCst) {
        if cancelled.load(Ordering::SeqCst) {
            engine.increment_epoch();
            return;
        }
        std::thread::park_timeout(CANCEL_POLL);
    }
}

fn run_guest(engine: &Engine, call: Call<'_>) -> Result<GuestValue, HostError> {
    let Call {
        wasm,
        profile,
        invocation,
        operation,
        input,
        snapshot,
        handles,
        limits,
        cancelled,
        required,
    } = call;
    let module =
        Module::new(engine, wasm).map_err(|error| HostError::Malformed(error.to_string()))?;
    check_imports(&module, profile)?;
    let state = GuestState {
        snapshot: snapshot.clone(),
        handles: handles
            .iter()
            .map(|(name, token)| (token.clone(), name.clone()))
            .collect(),
        memory: None,
        read_left: limits.read_bytes,
        output_left: limits.output_bytes,
        minted: 0,
        cancelled: Arc::clone(&cancelled),
        store_limits: store_limits(limits.memory_bytes),
    };
    let mut store = Store::new(engine, state);
    store
        .set_fuel(limits.fuel)
        .map_err(|error| HostError::Failed(error.to_string()))?;
    // The epoch starts at zero and the watcher bumps it once, on cancel. A
    // bump that landed before this deadline was set would not trap, so the
    // flag is read again after it.
    store.set_epoch_deadline(1);
    if cancelled.load(Ordering::SeqCst) {
        return Err(HostError::Cancelled);
    }
    store.limiter(|guest| &mut guest.store_limits);
    let mut linker = Linker::new(engine);
    if profile == Profile::SnapshotRead {
        linker
            .func_wrap("oa_host", "call", host_call)
            .map_err(|error| HostError::Failed(error.to_string()))?;
    }
    let instance = linker.instantiate(&mut store, &module).map_err(map_trap)?;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| HostError::Malformed("guest exports no memory".into()))?;
    store.data_mut().memory = Some(memory);
    let alloc = instance
        .get_typed_func::<i32, i32>(&mut store, "oa_alloc")
        .map_err(|_| HostError::Malformed("guest exports no oa_alloc".into()))?;
    let free = instance
        .get_typed_func::<(i32, i32), ()>(&mut store, "oa_free")
        .map_err(|_| HostError::Malformed("guest exports no oa_free".into()))?;
    let handle = instance
        .get_typed_func::<(i32, i32), i64>(&mut store, "oa_handle")
        .map_err(|_| HostError::Malformed("guest exports no oa_handle".into()))?;

    let packet = json!({
        "v": "openagents.plugin-packet.v1",
        "requires": [],
        "invocation": invocation,
        "operation": operation,
        "input": input,
        "handles": handles_object(handles)
    });
    let bytes =
        serde_json::to_vec(&packet).map_err(|error| HostError::Failed(error.to_string()))?;
    let input_ptr = alloc
        .call(
            &mut store,
            i32::try_from(bytes.len()).map_err(|_| HostError::Limit("input".into()))?,
        )
        .map_err(map_trap)?;
    let input_ptr_u = u32::try_from(input_ptr).map_err(|_| HostError::Malformed("alloc".into()))?;
    let input_len = u32::try_from(bytes.len()).map_err(|_| HostError::Limit("input".into()))?;
    check_range(input_ptr_u, input_len, memory.data_size(&store))
        .map_err(|d| HostError::Malformed(d.into()))?;
    memory
        .write(&mut store, input_ptr_u as usize, &bytes)
        .map_err(|error| HostError::Failed(error.to_string()))?;
    let packed = handle
        .call(&mut store, (input_ptr, input_len as i32))
        .map_err(map_trap)?;
    let (out_ptr, out_len) = unpack(packed);
    let memory_len = memory.data_size(&store);
    check_range(out_ptr, out_len, memory_len)
        .map_err(|detail| HostError::Malformed(detail.into()))?;
    if overlaps(input_ptr_u, input_len, out_ptr, out_len) {
        return Err(HostError::Malformed("output overlaps input".into()));
    }
    if out_len as usize > limits.output_bytes {
        return Err(HostError::Limit("output bytes".into()));
    }
    let mut output = vec![0_u8; out_len as usize];
    memory
        .read(&store, out_ptr as usize, &mut output)
        .map_err(|error| HostError::Failed(error.to_string()))?;
    free.call(&mut store, (input_ptr, input_len as i32))
        .map_err(map_trap)?;
    free.call(&mut store, (out_ptr as i32, out_len as i32))
        .map_err(map_trap)?;
    let response = parse_response(&output, invocation)?;
    finish(response, required)
}

fn store_limits(memory_bytes: usize) -> StoreLimits {
    wasmtime::StoreLimitsBuilder::new()
        .memory_size(memory_bytes)
        .instances(1)
        .memories(1)
        .tables(4)
        .table_elements(10_000)
        .build()
}

fn check_imports(module: &Module, profile: Profile) -> Result<(), HostError> {
    for import in module.imports() {
        let host_call = import.module() == "oa_host" && import.name() == "call";
        if profile == Profile::SnapshotRead && host_call {
            continue;
        }
        return Err(HostError::Denied(format!(
            "import {} {}",
            import.module(),
            import.name()
        )));
    }
    Ok(())
}

fn host_call(
    mut caller: Caller<'_, GuestState>,
    req: i32,
    req_len: i32,
    resp: i32,
    resp_cap: i32,
) -> Result<i32, wasmtime::Error> {
    if caller.data().cancelled.load(Ordering::Relaxed) {
        return Err(wasmtime::Error::msg("cancelled"));
    }
    let Some(memory) = caller.data().memory else {
        return Ok(-1);
    };
    let req_ptr = req as u32;
    let req_len_u = req_len as u32;
    if check_range(req_ptr, req_len_u, memory.data_size(&caller)).is_err() {
        return Ok(-1);
    }
    let mut request = vec![0_u8; req_len as usize];
    memory
        .read(&caller, req as usize, &mut request)
        .map_err(|error| wasmtime::Error::msg(error.to_string()))?;
    let value = match parse_strict(&request) {
        Ok(value) => value,
        Err(_) => return Ok(-1),
    };
    let response = match dispatch_import(caller.data_mut(), &value) {
        Ok(response) => response,
        Err(-5) => return Ok(-5),
        Err(-3) => return Ok(-3),
        Err(-2) => return Ok(-2),
        Err(_) => return Ok(-1),
    };
    let bytes =
        serde_json::to_vec(&response).map_err(|error| wasmtime::Error::msg(error.to_string()))?;
    if bytes.len() > resp_cap as usize || bytes.len() > caller.data().output_left {
        return Ok(-4);
    }
    let resp_ptr = resp as u32;
    let resp_len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    if check_range(resp_ptr, resp_len, memory.data_size(&caller)).is_err() {
        return Ok(-4);
    }
    if overlaps(req_ptr, req_len_u, resp_ptr, resp_len) {
        return Ok(-1);
    }
    caller.data_mut().output_left -= bytes.len();
    memory
        .write(&mut caller, resp as usize, &bytes)
        .map_err(|error| wasmtime::Error::msg(error.to_string()))?;
    i32::try_from(bytes.len()).map_err(|error| wasmtime::Error::msg(error.to_string()))
}

fn dispatch_import(guest: &mut GuestState, value: &Value) -> Result<Value, i32> {
    let object = value.as_object().ok_or(-1)?;
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(-1);
    }
    let handle = object.get("handle").and_then(Value::as_str).ok_or(-1)?;
    let name = guest.handles.get(handle).ok_or(-5)?.clone();
    let operation = object.get("operation").and_then(Value::as_str).ok_or(-1)?;
    let args = object.get("args").and_then(Value::as_object);
    if guest.cancelled.load(Ordering::Relaxed) {
        return Err(-6);
    }
    match operation {
        "metadata" => guest.snapshot.metadata(&name).map_err(import_code),
        "list" => {
            let cursor = args
                .and_then(|args| args.get("cursor"))
                .and_then(Value::as_str);
            let max_entries = args
                .and_then(|args| args.get("max_entries"))
                .and_then(Value::as_u64)
                .unwrap_or(16) as usize;
            let mut value = guest
                .snapshot
                .list(&name, cursor, max_entries)
                .map_err(import_code)?;
            // A listed child gets a token scoped to this invocation, so
            // the guest can read what it listed and nothing it didn't.
            if let Some(entries) = value["entries"].as_array_mut() {
                for entry in entries {
                    let child = entry["name"].as_str().unwrap_or_default().to_string();
                    let token = guest.mint(child);
                    entry["handle"] = Value::String(token);
                }
            }
            Ok(value)
        }
        "read" => {
            let offset = args
                .and_then(|args| args.get("offset"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let max_bytes = args
                .and_then(|args| args.get("max_bytes"))
                .and_then(Value::as_u64)
                .unwrap_or(1024) as usize;
            if max_bytes > guest.read_left {
                return Err(-3);
            }
            let version = args
                .and_then(|args| args.get("version"))
                .and_then(Value::as_str);
            let value = guest
                .snapshot
                .read(&name, offset, max_bytes, version)
                .map_err(import_code)?;
            let size = value["bytes_base64"].as_str().map_or(0, str::len);
            guest.read_left = guest.read_left.saturating_sub(size.min(max_bytes));
            Ok(value)
        }
        _ => Err(-2),
    }
}

impl GuestState {
    /// A fresh invocation-scoped token for the entry `name`.
    fn mint(&mut self, name: String) -> String {
        loop {
            self.minted += 1;
            let token = format!("child-{}", self.minted);
            if !self.handles.contains_key(&token) {
                self.handles.insert(token.clone(), name);
                return token;
            }
        }
    }
}

fn import_code(detail: &str) -> i32 {
    match detail {
        "stale version" | "cursor" | "missing entry" => -5,
        "symlink" | "not a file" | "not a directory" => -2,
        _ => -1,
    }
}

fn parse_response(bytes: &[u8], invocation: &str) -> Result<Value, HostError> {
    let value = parse_strict(bytes).map_err(|error| HostError::Malformed(error.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| HostError::Malformed("response".into()))?;
    if object.get("v").and_then(Value::as_str) != Some("openagents.plugin-packet.v1") {
        return Err(HostError::Malformed("response version".into()));
    }
    if object
        .get("requires")
        .and_then(Value::as_array)
        .is_none_or(|items| !items.is_empty())
    {
        return Err(HostError::Malformed("response requires".into()));
    }
    if object.get("invocation").and_then(Value::as_str) != Some(invocation) {
        return Err(HostError::Malformed("response invocation".into()));
    }
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| HostError::Malformed("response status".into()))?;
    if !matches!(status, "ok" | "unsupported_input" | "refused") {
        return Err(HostError::Malformed("response status".into()));
    }
    if status != "ok" && object.get("value") != Some(&Value::Null) {
        return Err(HostError::Malformed("response value".into()));
    }
    Ok(value)
}

fn finish(response: Value, required: bool) -> Result<GuestValue, HostError> {
    let status = response["status"].as_str().unwrap_or("refused").to_string();
    if status == "ok" {
        return Ok(GuestValue {
            status,
            value: response["value"].clone(),
            verification: "not_run",
        });
    }
    if !required {
        return Ok(GuestValue {
            status: "ok".into(),
            value: json!({"fallback": true, "status": status}),
            verification: "not_run",
        });
    }
    Err(HostError::Refused(status))
}

fn handles_object(handles: &BTreeMap<String, String>) -> Map<String, Value> {
    handles
        .iter()
        .map(|(name, token)| (name.clone(), Value::String(token.clone())))
        .collect()
}

fn unpack(packed: i64) -> (u32, u32) {
    let bits = packed as u64;
    ((bits >> 32) as u32, bits as u32)
}

fn map_trap(error: wasmtime::Error) -> HostError {
    if error.to_string().contains("cancelled") {
        return HostError::Cancelled;
    }
    match error.downcast_ref::<Trap>() {
        Some(Trap::OutOfFuel) => return HostError::Limit("fuel".into()),
        Some(Trap::Interrupt) => return HostError::Cancelled,
        _ => {}
    }
    let text = error.to_string();
    if text.contains("fuel") {
        return HostError::Limit("fuel".into());
    }
    HostError::Failed(text)
}

/// The host rule owns the representation. Installation order does not.
#[must_use]
pub fn representation(rule: &str, installed: &[&str]) -> String {
    let _ = installed;
    rule.to_string()
}

/// Digests of the PDK source and the guest bytes a build produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildReceipt {
    /// Digest of the PDK source.
    pub pdk_digest: String,
    /// Digest of the guest module bytes.
    pub guest_digest: String,
    /// Profile the guest was built for.
    pub profile: String,
}

/// Bind a guest build to the PDK source it was compiled against.
#[must_use]
pub fn build_receipt(pdk: &[u8], guest: &[u8], profile: &str) -> BuildReceipt {
    BuildReceipt {
        pdk_digest: digest(pdk),
        guest_digest: digest(guest),
        profile: profile.to_string(),
    }
}

/// The `sha256:`-prefixed hex digest of `bytes`.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(bytes);
    format!("sha256:{}", hex(&hash))
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0xf) as usize] as char);
    }
    out
}
