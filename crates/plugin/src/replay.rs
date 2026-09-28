//! Invocation receipts and exact replay.
//!
//! A receipt records the digests of everything an invocation read and
//! returned: the module, operation, input, snapshot, handles, limits, and
//! engine, and then the outcome and the fuel it consumed. Another host
//! holding the same bytes replays the invocation and compares. A match is
//! `passed` with the verification class `exact_replay`.
//!
//! A replay proves that the same bytes on the same engine configuration
//! produced the same output. It is not remote attestation. It says nothing
//! about whether the output is correct. It reruns the guest on this host,
//! so it is only as independent as this host is from the one that wrote the
//! receipt.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::Ordering;

use nostr::contracts::parse_strict;
use serde_json::{Map, Value, json};

use crate::engine::{Call, GuestValue, HostError, Limits, Profile, digest, metered};
use crate::snapshot::{Entry, Snapshot};

/// The receipt's schema identifier.
pub const RECEIPT_SCHEMA: &str = "openagents.plugin-invocation-receipt.v1";

/// The engine and configuration [`crate::invoke`] runs guests on. A replay
/// on a different engine is `unverifiable`, not `failed`.
pub const ENGINE: &str = "wasmtime/48.0.2+consume_fuel+epoch_interruption";

/// The verification class a passing replay reports.
pub const EXACT_REPLAY: &str = "exact_replay";

/// What an invocation ended with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The guest returned a value.
    Value {
        /// The status [`GuestValue`] carried.
        status: String,
        /// Digest of the canonical JSON of the value.
        output: String,
    },
    /// The host returned a typed error.
    Error {
        /// `malformed`, `denied`, `limit`, `stale`, `failed`, `cancelled`,
        /// or `refused`.
        kind: String,
        /// The host-authored detail for `limit` and `refused`; empty
        /// otherwise, because other details can carry engine text.
        detail: String,
    },
}

impl Outcome {
    fn of(result: &Result<GuestValue, HostError>) -> Self {
        match result {
            Ok(value) => Self::Value {
                status: value.status.clone(),
                output: digest(canonical(&value.value).as_bytes()),
            },
            Err(error) => {
                let (kind, detail) = match error {
                    HostError::Malformed(_) => ("malformed", String::new()),
                    HostError::Denied(_) => ("denied", String::new()),
                    HostError::Limit(detail) => ("limit", detail.clone()),
                    HostError::Stale(_) => ("stale", String::new()),
                    HostError::Failed(_) => ("failed", String::new()),
                    HostError::Cancelled => ("cancelled", String::new()),
                    HostError::Refused(detail) => ("refused", detail.clone()),
                };
                Self::Error {
                    kind: kind.into(),
                    detail,
                }
            }
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Value { status, output } => format!("{status} {output}"),
            Self::Error { kind, detail } if detail.is_empty() => format!("error {kind}"),
            Self::Error { kind, detail } => format!("error {kind}: {detail}"),
        }
    }
}

/// The digested record of one invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationReceipt {
    /// Digest of the guest module bytes.
    pub module: String,
    /// Pure or snapshot-read.
    pub profile: Profile,
    /// Operation slug.
    pub operation: String,
    /// Host-generated invocation ID. It is part of the packet.
    pub invocation: String,
    /// Digest of the canonical JSON of the operation input.
    pub input: String,
    /// Digest of the granted snapshot and its handles.
    pub snapshot: String,
    /// Ceilings the invocation ran under.
    pub limits: Limits,
    /// Whether a guest refusal failed the invocation.
    pub required: bool,
    /// The engine identity, [`ENGINE`] on this host.
    pub engine: String,
    /// What the invocation ended with.
    pub outcome: Outcome,
    /// Guest fuel consumed, including the start function.
    pub fuel_consumed: u64,
}

/// A replay's verdict, in the shared verification vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replay {
    /// The rerun matched the receipt's outcome and fuel.
    Passed {
        /// Always [`EXACT_REPLAY`].
        class: &'static str,
    },
    /// The rerun ran on the receipt's inputs and diverged.
    Failed {
        /// `outcome` or `fuel_consumed`.
        field: &'static str,
        /// What the receipt recorded.
        expected: String,
        /// What the rerun produced.
        actual: String,
    },
    /// The replay could not run on the receipt's inputs, so it proves
    /// nothing either way.
    Unverifiable {
        /// Why, such as `input digest differs`.
        reason: String,
    },
}

impl Replay {
    /// `passed`, `failed`, or `unverifiable`.
    #[must_use]
    pub fn verification(&self) -> &'static str {
        match self {
            Self::Passed { .. } => "passed",
            Self::Failed { .. } => "failed",
            Self::Unverifiable { .. } => "unverifiable",
        }
    }
}

/// Run one guest, as [`crate::invoke`] does, and return its receipt.
///
/// The value's verification stays `not_run`: a receipt is not a check.
pub fn invoke_with_receipt(call: Call<'_>) -> (Result<GuestValue, HostError>, InvocationReceipt) {
    let inputs = Inputs::of(&call);
    let (result, fuel) = metered(call);
    let receipt = inputs.receipt(Outcome::of(&result), fuel);
    (result, receipt)
}

/// Rerun `call` and compare it with `receipt`.
///
/// The call must supply the same module, operation, invocation ID, input,
/// snapshot, handles, limits, and requirement the receipt digests, on the
/// same engine. Otherwise the verdict is `unverifiable` and the guest does
/// not run.
#[must_use]
pub fn replay(receipt: &InvocationReceipt, call: Call<'_>) -> Replay {
    if receipt.engine != ENGINE {
        return unverifiable(format!("engine {} is not {ENGINE}", receipt.engine));
    }
    if matches!(&receipt.outcome, Outcome::Error { kind, .. } if kind == "cancelled") {
        return unverifiable("the recorded invocation was cancelled".into());
    }
    let inputs = Inputs::of(&call);
    let checks = [
        ("module digest", receipt.module == inputs.module),
        ("profile", receipt.profile == inputs.profile),
        ("operation", receipt.operation == inputs.operation),
        ("invocation", receipt.invocation == inputs.invocation),
        ("input digest", receipt.input == inputs.input),
        ("snapshot digest", receipt.snapshot == inputs.snapshot),
        ("limits", receipt.limits == inputs.limits),
        ("required", receipt.required == inputs.required),
    ];
    if let Some((name, _)) = checks.iter().find(|(_, same)| !same) {
        return unverifiable(format!("{name} differs"));
    }
    let cancelled = std::sync::Arc::clone(&call.cancelled);
    let (result, fuel) = metered(call);
    if cancelled.load(Ordering::SeqCst) {
        return unverifiable("the replay was cancelled".into());
    }
    let outcome = Outcome::of(&result);
    if outcome != receipt.outcome {
        return Replay::Failed {
            field: "outcome",
            expected: receipt.outcome.describe(),
            actual: outcome.describe(),
        };
    }
    if fuel != receipt.fuel_consumed {
        return Replay::Failed {
            field: "fuel_consumed",
            expected: receipt.fuel_consumed.to_string(),
            actual: fuel.to_string(),
        };
    }
    Replay::Passed {
        class: EXACT_REPLAY,
    }
}

fn unverifiable(reason: String) -> Replay {
    Replay::Unverifiable { reason }
}

/// The digested inputs of one call.
struct Inputs {
    module: String,
    profile: Profile,
    operation: String,
    invocation: String,
    input: String,
    snapshot: String,
    limits: Limits,
    required: bool,
}

impl Inputs {
    fn of(call: &Call<'_>) -> Self {
        Self {
            module: digest(call.wasm),
            profile: call.profile,
            operation: call.operation.to_string(),
            invocation: call.invocation.to_string(),
            input: digest(canonical(call.input).as_bytes()),
            snapshot: snapshot_digest(call.snapshot, call.handles),
            limits: call.limits,
            required: call.required,
        }
    }

    fn receipt(self, outcome: Outcome, fuel_consumed: u64) -> InvocationReceipt {
        InvocationReceipt {
            module: self.module,
            profile: self.profile,
            operation: self.operation,
            invocation: self.invocation,
            input: self.input,
            snapshot: self.snapshot,
            limits: self.limits,
            required: self.required,
            engine: ENGINE.into(),
            outcome,
            fuel_consumed,
        }
    }
}

/// The digest of every entry in `snapshot`, in name order, and the handle
/// table.
fn snapshot_digest(snapshot: &Snapshot, handles: &BTreeMap<String, String>) -> String {
    let entries: Vec<Value> = snapshot
        .entries()
        .map(|(name, entry)| {
            let body = match entry {
                Entry::File {
                    bytes,
                    version,
                    complete,
                } => json!({
                    "type": "file",
                    "bytes": digest(bytes),
                    "version": version,
                    "complete": complete
                }),
                Entry::Directory { children } => json!({
                    "type": "directory",
                    "children": children
                }),
                Entry::Symlink { target } => json!({"type": "symlink", "target": target}),
            };
            json!([name, body])
        })
        .collect();
    let handles: Map<String, Value> = handles
        .iter()
        .map(|(name, token)| (name.clone(), Value::String(token.clone())))
        .collect();
    let value = json!({"entries": entries, "handles": handles});
    digest(canonical(&value).as_bytes())
}

/// JSON with object keys sorted at every level and no insignificant
/// whitespace, so the digest doesn't depend on key order.
#[must_use]
pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(object) => {
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String((*key).clone()).to_string());
                out.push(':');
                write_canonical(&object[*key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        scalar => {
            let _ = write!(out, "{scalar}");
        }
    }
}

impl InvocationReceipt {
    /// The receipt as JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let outcome = match &self.outcome {
            Outcome::Value { status, output } => {
                json!({"type": "value", "status": status, "output": output})
            }
            Outcome::Error { kind, detail } => {
                json!({"type": "error", "kind": kind, "detail": detail})
            }
        };
        json!({
            "schema": RECEIPT_SCHEMA,
            "module": self.module,
            "profile": profile_name(self.profile),
            "operation": self.operation,
            "invocation": self.invocation,
            "input": self.input,
            "snapshot": self.snapshot,
            "limits": {
                "fuel": self.limits.fuel,
                "memory_bytes": self.limits.memory_bytes,
                "output_bytes": self.limits.output_bytes,
                "read_bytes": self.limits.read_bytes,
                "module_bytes": self.limits.module_bytes
            },
            "required": self.required,
            "engine": self.engine,
            "outcome": outcome,
            "fuel_consumed": self.fuel_consumed
        })
    }

    /// Parse a receipt. Duplicate keys, unknown fields, missing fields,
    /// and malformed digests are refused.
    ///
    /// # Errors
    ///
    /// Returns the first field that does not parse.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let value = parse_strict(bytes).map_err(|error| error.to_string())?;
        let object = fields(
            &value,
            "receipt",
            &[
                "schema",
                "module",
                "profile",
                "operation",
                "invocation",
                "input",
                "snapshot",
                "limits",
                "required",
                "engine",
                "outcome",
                "fuel_consumed",
            ],
        )?;
        if text(object, "schema")? != RECEIPT_SCHEMA {
            return Err("schema".into());
        }
        let profile = match text(object, "profile")? {
            "pure" => Profile::Pure,
            "snapshot-read" => Profile::SnapshotRead,
            _ => return Err("profile".into()),
        };
        let limits = fields(
            &object["limits"],
            "limits",
            &[
                "fuel",
                "memory_bytes",
                "output_bytes",
                "read_bytes",
                "module_bytes",
            ],
        )?;
        let size = |name: &str| -> Result<usize, String> {
            usize::try_from(number(limits, name)?).map_err(|_| name.to_string())
        };
        let limits = Limits {
            fuel: number(limits, "fuel")?,
            memory_bytes: size("memory_bytes")?,
            output_bytes: size("output_bytes")?,
            read_bytes: size("read_bytes")?,
            module_bytes: size("module_bytes")?,
        };
        let outcome = &object["outcome"];
        let outcome = match outcome.get("type").and_then(Value::as_str) {
            Some("value") => {
                let fields = fields(outcome, "outcome", &["type", "status", "output"])?;
                Outcome::Value {
                    status: text(fields, "status")?.into(),
                    output: sha256(fields, "output")?,
                }
            }
            Some("error") => {
                let fields = fields(outcome, "outcome", &["type", "kind", "detail"])?;
                Outcome::Error {
                    kind: text(fields, "kind")?.into(),
                    detail: text(fields, "detail")?.into(),
                }
            }
            _ => return Err("outcome".into()),
        };
        Ok(Self {
            module: sha256(object, "module")?,
            profile,
            operation: text(object, "operation")?.into(),
            invocation: text(object, "invocation")?.into(),
            input: sha256(object, "input")?,
            snapshot: sha256(object, "snapshot")?,
            limits,
            required: object["required"]
                .as_bool()
                .ok_or_else(|| "required".to_string())?,
            engine: text(object, "engine")?.into(),
            outcome,
            fuel_consumed: number(object, "fuel_consumed")?,
        })
    }
}

fn profile_name(profile: Profile) -> &'static str {
    match profile {
        Profile::Pure => "pure",
        Profile::SnapshotRead => "snapshot-read",
    }
}

/// `value` as an object with exactly the keys `expected`.
fn fields<'a>(
    value: &'a Value,
    name: &str,
    expected: &[&str],
) -> Result<&'a Map<String, Value>, String> {
    let object = value.as_object().ok_or_else(|| name.to_string())?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(format!("{name} fields"));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, String> {
    object[name].as_str().ok_or_else(|| name.to_string())
}

fn number(object: &Map<String, Value>, name: &str) -> Result<u64, String> {
    object[name].as_u64().ok_or_else(|| name.to_string())
}

fn sha256(object: &Map<String, Value>, name: &str) -> Result<String, String> {
    let digest = text(object, name)?;
    let hex = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| name.to_string())?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(name.to_string());
    }
    Ok(digest.to_string())
}
