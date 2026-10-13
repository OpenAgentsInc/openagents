//! Plugin hooks (#11499, `docs/coder/design/coder-sdk.md`, "Plugin
//! hooks"): a plugin's Wasm guest as a hook.
//!
//! **The part.** A plugin's package record pins hook documents under
//! `hooks/` (`"hooks": [{"name", "digest"}]`). Each names an event, a
//! matcher, the guest operation, the module (`wasm/<name>.wasm`, pinned by
//! digest), the authority it asks for, what of the event it discloses to
//! the guest, and whether it may ask for a decision:
//!
//! ```json
//! {"id": "guard", "event": "PreToolUse", "matcher": "Bash", "operation": "guard",
//!  "module": {"name": "hook-guard", "digest": "sha256:..."},
//!  "authority": ["deny", "ask"], "discloses": ["tool", "input"]}
//! ```
//!
//! **Running.** The normalized event (only the disclosed fields) is the
//! packet's input; the guest's value is read as a [`HookOutcome`]; a
//! refusal is no opinion. It runs on the plugin host (`crates/plugin`) in
//! a fresh instance under its limits, with an empty snapshot, and every
//! firing keeps its invocation receipt and the exchanges with the host, so
//! it can be replayed ([`replay`]). Third-party code never runs in this
//! process outside that sandbox.
//!
//! **Authority** only tightens: deny, ask, context, and holding a stop
//! (`block_stop`). A plugin's allow never counts. A rewrite counts only
//! when the person turned it on for that hook ([`Grants`]); asked for and
//! not granted, the hook is registered without it and that is recorded.
//! `spend` is reserved for paid plugins and never granted yet. Nothing gets
//! past a floor: the SDK's composition rules hold.
//!
//! **Off until on.** A plugin's hooks register only while the person has
//! the plugin on, and only when the session's [`Admission`] covers what
//! each hook discloses. A guest may ask the host for a decision through
//! `oa_host.call` (`{"v": 1, "capability": "decide", ...}`), so a plugin
//! can ship its own decision hook without holding a key; that is a second
//! disclosure, to the Decisions API, checked the same way.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::admission::Admission;
use super::compose::{Budget, Runner};
use super::decide::Decider;
use super::events::{Call, Event};
use super::outcome::{HookOutcome, Permission, Stop};
use super::registry::{Disclosure, Hook, Registry, Source, Versions};
use super::scripted::{Replay as SessionReplay, ScriptedSession};

/// What a hook may do with its answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    Deny,
    Ask,
    Context,
    /// Keep the agent working at a stop.
    BlockStop,
    /// Change the tool's input; only with the person's grant.
    Rewrite,
    /// Paid plugins, later: parsed so a record can carry it, never granted.
    Spend,
}

impl Authority {
    fn words(self) -> &'static str {
        match self {
            Authority::Deny => "refuse",
            Authority::Ask => "ask you first",
            Authority::Context => "add a note for the agent",
            Authority::BlockStop => "keep the agent working",
            Authority::Rewrite => "change the command",
            Authority::Spend => "spend",
        }
    }
}

/// A module a hook runs, pinned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub name: String,
    /// `plugin::digest` of the module bytes.
    pub digest: String,
}

/// One hook document under a plugin's `hooks/`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookDoc {
    pub id: String,
    /// The event, by its Claude Code name.
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub operation: String,
    pub module: Module,
    pub authority: Vec<Authority>,
    /// What of the event the guest sees: `tool`, `input`. The event's
    /// name is always sent.
    #[serde(default)]
    pub discloses: Vec<String>,
    /// Whether the guest may ask the host for a decision.
    #[serde(default)]
    pub decide: bool,
    /// What it does, in plain words, for the plugin's listing.
    #[serde(default)]
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// One hook of a loaded plugin: its document and its module's bytes.
#[derive(Clone, Debug)]
pub struct PluginHook {
    pub doc: HookDoc,
    pub event: Event,
    pub wasm: Arc<Vec<u8>>,
}

/// A plugin as its hooks see it.
#[derive(Clone, Debug)]
pub struct Plugin {
    /// `KEY:SLUG`.
    pub id: String,
    pub name: String,
    /// Whether the person has it on on this computer.
    pub enabled: bool,
    pub hooks: Vec<PluginHook>,
}

/// The digest a package record states for a component's text.
#[must_use]
pub fn text_digest(text: &str) -> String {
    let encoded = serde_json::to_string(text).unwrap_or_default();
    plugin::digest(encoded.as_bytes())
        .trim_start_matches("sha256:")
        .to_owned()
}

/// Reads the hooks `dir`'s package record pins, checking every digest.
///
/// # Errors
///
/// A hook the record pins that is missing, changed since it was pinned,
/// names an unknown event, or runs a module that is missing or changed.
pub fn load(id: &str, name: &str, enabled: bool, dir: &Path) -> Result<Plugin, String> {
    let record_path = dir.join("package.json");
    let record: Value = std::fs::read_to_string(&record_path)
        .map_err(|error| format!("{}: {error}", record_path.display()))
        .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))?;
    let mut hooks = Vec::new();
    for reference in record["hooks"].as_array().into_iter().flatten() {
        let hook = reference["name"].as_str().unwrap_or_default();
        let pinned = reference["digest"].as_str().unwrap_or_default();
        let path = dir.join("hooks").join(format!("{hook}.json"));
        let text = std::fs::read_to_string(&path).map_err(|_| {
            format!("{name} names the hook {hook}, but hooks/{hook}.json isn't there.")
        })?;
        if text_digest(&text) != pinned {
            return Err(format!("{name}'s hook {hook} changed since it was pinned."));
        }
        let doc: HookDoc = serde_json::from_str(&text)
            .map_err(|error| format!("{name}'s hook {hook}: {error}"))?;
        let event = Event::from_name(&doc.event).ok_or_else(|| {
            format!(
                "{name}'s hook {hook} names the unknown event {}.",
                doc.event
            )
        })?;
        let module = dir.join("wasm").join(format!("{}.wasm", doc.module.name));
        let wasm = std::fs::read(&module).map_err(|_| {
            format!(
                "{name}'s hook {hook} runs wasm/{}.wasm, which isn't there.",
                doc.module.name
            )
        })?;
        if plugin::digest(&wasm) != doc.module.digest {
            return Err(format!(
                "{name}'s module {} changed since it was pinned.",
                doc.module.name
            ));
        }
        hooks.push(PluginHook {
            doc,
            event,
            wasm: Arc::new(wasm),
        });
    }
    Ok(Plugin {
        id: id.to_owned(),
        name: name.to_owned(),
        enabled,
        hooks,
    })
}

/// The rewrites the person turned on, as `KEY:SLUG/hook`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grants {
    #[serde(default)]
    pub rewrite: BTreeSet<String>,
}

impl Grants {
    #[must_use]
    pub fn key(plugin: &str, hook: &str) -> String {
        format!("{plugin}/{hook}")
    }

    #[must_use]
    pub fn rewrites(&self, plugin: &str, hook: &str) -> bool {
        self.rewrite.contains(&Self::key(plugin, hook))
    }
}

/// What the guest sees, and who sees it.
#[must_use]
pub fn disclosure(plugin: &Plugin, hook: &PluginHook) -> Disclosure {
    let mut what = vec!["the event's name".to_owned()];
    what.extend(hook.doc.discloses.iter().map(|field| match field.as_str() {
        "tool" => "the tool's name".to_owned(),
        "input" => "the tool's input".to_owned(),
        other => format!("the event's {other}"),
    }));
    Disclosure {
        sends: what.join(", "),
        to: format!("{}'s Wasm guest", plugin.name),
    }
}

/// What a hook that may ask for a decision sends the Decisions API.
#[must_use]
pub fn decide_disclosure(plugin: &Plugin, hook: &PluginHook) -> Disclosure {
    Disclosure {
        sends: format!("what {}'s {} hook asks about", plugin.name, hook.doc.id),
        to: super::decide::disclosure().to,
    }
}

/// The admission turning a plugin on gives: what its listing shows each
/// hook disclosing, to its guest and, for a hook that decides, to the
/// Decisions API.
#[must_use]
pub fn admitted_by(plugin: &Plugin) -> Admission {
    let mut admission = Admission::none();
    for hook in &plugin.hooks {
        admission = admission.admitting(disclosure(plugin, hook));
        if hook.doc.decide {
            admission = admission.admitting(decide_disclosure(plugin, hook));
        }
    }
    admission
}

/// What registering a plugin's hooks did, to record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Note {
    Registered {
        hook: String,
        authority: Vec<Authority>,
    },
    /// The plugin is off: none of its hooks run.
    PluginOff { plugin: String },
    /// It asked to rewrite input and the person hasn't turned that on.
    RewriteWithheld { hook: String, message: String },
    /// It may not ask for decisions in this session.
    DecideWithheld { hook: String, message: String },
    /// It asked to spend; paid plugins aren't here yet.
    SpendWithheld { hook: String, message: String },
    /// The session doesn't admit what it discloses, so it is off.
    Refused { hook: String, message: String },
    /// It couldn't be registered.
    Invalid { hook: String, message: String },
}

/// One exchange between a guest and the host's capabilities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exchange {
    pub request: Value,
    /// The answer, or the error code.
    pub response: Result<Value, i32>,
}

/// One firing of a plugin hook: enough to replay it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Firing {
    pub plugin: String,
    pub hook: String,
    pub operation: String,
    pub invocation: String,
    /// The guest profile: `snapshot-read` with an empty snapshot, so the
    /// guest's one import reaches only the capabilities it was granted.
    pub profile: String,
    /// The packet input the guest got.
    pub input: Value,
    /// What the guest asked the host, in order.
    pub host_calls: Vec<Exchange>,
    /// The plugin host's invocation receipt.
    pub receipt: Value,
    /// The outcome after the hook's authority was applied.
    pub outcome: HookOutcome,
}

/// The firings of a session's plugin hooks, kept in memory and, when a
/// log is set, one JSON line each.
#[derive(Clone, Default)]
pub struct Firings {
    all: Arc<Mutex<Vec<Firing>>>,
    log: Option<PathBuf>,
}

impl Firings {
    #[must_use]
    pub fn logging(path: PathBuf) -> Self {
        Firings {
            all: Arc::default(),
            log: Some(path),
        }
    }

    #[must_use]
    pub fn all(&self) -> Vec<Firing> {
        self.all
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn push(&self, firing: Firing) {
        if let Some(path) = &self.log
            && let Ok(line) = serde_json::to_string(&firing)
        {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(file, "{line}");
            }
        }
        self.all
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(firing);
    }
}

/// A plugin hook's default time to answer.
pub const TIMEOUT: Duration = Duration::from_secs(2);

static NEXT_INVOCATION: AtomicU64 = AtomicU64::new(1);

/// Registers `plugin`'s hooks into `registry` within `admission`, with
/// `grants` for rewrites and `decider` for decisions. Returns what was
/// done, to record.
pub fn register(
    registry: &mut Registry,
    plugin: &Plugin,
    grants: &Grants,
    admission: &Admission,
    decider: Option<Arc<dyn Decider>>,
    firings: &Firings,
) -> Vec<Note> {
    if !plugin.enabled {
        return vec![Note::PluginOff {
            plugin: plugin.id.clone(),
        }];
    }
    let mut notes = Vec::new();
    for hook in &plugin.hooks {
        let id = format!("{}/{}", plugin.id, hook.doc.id);
        let sends = disclosure(plugin, hook);
        if let Err(refused) = admission.check(&id, Some(&sends)) {
            notes.push(Note::Refused {
                hook: id,
                message: refused.message,
            });
            continue;
        }
        let mut authority: BTreeSet<Authority> = hook.doc.authority.iter().copied().collect();
        if authority.remove(&Authority::Spend) {
            notes.push(Note::SpendWithheld {
                hook: id.clone(),
                message: format!("{id} asks to spend, and paid plugins aren't available yet."),
            });
        }
        let granted_rewrite =
            authority.contains(&Authority::Rewrite) && grants.rewrites(&plugin.id, &hook.doc.id);
        if authority.contains(&Authority::Rewrite) && !granted_rewrite {
            authority.remove(&Authority::Rewrite);
            notes.push(Note::RewriteWithheld {
                hook: id.clone(),
                message: format!(
                    "{id} can change commands only when you turn that on; it runs without it."
                ),
            });
        }
        let decides = hook.doc.decide
            && decider.is_some()
            && admission
                .check(&id, Some(&decide_disclosure(plugin, hook)))
                .is_ok();
        if hook.doc.decide && !decides {
            notes.push(Note::DecideWithheld {
                hook: id.clone(),
                message: format!("{id} can't ask for decisions in this session."),
            });
        }
        let body = Body {
            plugin: plugin.id.clone(),
            hook: hook.doc.id.clone(),
            operation: hook.doc.operation.clone(),
            discloses: hook.doc.discloses.clone(),
            authority: authority.clone(),
            wasm: hook.wasm.clone(),
            decider: decides.then(|| decider.clone()).flatten(),
            firings: firings.clone(),
        };
        let mut built = Hook::new(&id, hook.event, Source::Plugin, move |call: &Call| {
            body.fire(call)
        })
        .timeout(hook.doc.timeout_ms.map_or(TIMEOUT, Duration::from_millis))
        .pinned(Versions {
            question_set: None,
            policy: Some(hook.doc.module.digest.clone()),
            model: None,
        });
        built.disclosure = Some(sends);
        built.matcher.clone_from(&hook.doc.matcher);
        if granted_rewrite {
            built = built.granted_rewrite();
        }
        match registry.register(built) {
            Ok(()) => notes.push(Note::Registered {
                hook: id,
                authority: authority.into_iter().collect(),
            }),
            Err(message) => notes.push(Note::Invalid { hook: id, message }),
        }
    }
    notes
}

/// One hook's firing: the guest, under its authority.
struct Body {
    plugin: String,
    hook: String,
    operation: String,
    discloses: Vec<String>,
    authority: BTreeSet<Authority>,
    wasm: Arc<Vec<u8>>,
    decider: Option<Arc<dyn Decider>>,
    firings: Firings,
}

impl Body {
    fn fire(&self, call: &Call) -> Result<HookOutcome, String> {
        let mut input = json!({"event": call.event.name()});
        if self.discloses.iter().any(|field| field == "tool") {
            input["tool"] = json!(call.tool);
        }
        if self.discloses.iter().any(|field| field == "input") {
            input["input"] = call.input.clone();
        }
        let invocation = format!(
            "hook-{}-{}",
            std::process::id(),
            NEXT_INVOCATION.fetch_add(1, Ordering::Relaxed)
        );
        let exchanges: Arc<Mutex<Vec<Exchange>>> = Arc::default();
        // `snapshot-read` links `oa_host.call`, which every PDK guest
        // imports; the snapshot is empty and no handle is granted, so the
        // only thing it reaches is a capability the hook was granted.
        let profile = plugin::Profile::SnapshotRead;
        let snapshot = plugin::Snapshot::default();
        let handles = BTreeMap::new();
        let guest = plugin::Call {
            wasm: &self.wasm,
            profile,
            invocation: &invocation,
            operation: &self.operation,
            input: &input,
            snapshot: &snapshot,
            handles: &handles,
            limits: plugin::Limits::default(),
            cancelled: Arc::new(AtomicBool::new(false)),
            required: false,
        };
        let (result, receipt) = match &self.decider {
            Some(decider) => {
                let host = decide_host(decider.clone(), self.id(), exchanges.clone());
                plugin::invoke_with_receipt_and_host(guest, host)
            }
            None => plugin::invoke_with_receipt(guest),
        };
        let outcome = match &result {
            Ok(value) if value.value.get("fallback") == Some(&Value::Bool(true)) => {
                HookOutcome::none()
            }
            Ok(value) => narrow(read_outcome(&value.value)?, &self.authority),
            Err(_) => HookOutcome::none(),
        };
        self.firings.push(Firing {
            plugin: self.plugin.clone(),
            hook: self.hook.clone(),
            operation: self.operation.clone(),
            invocation,
            profile: profile_name(profile).to_owned(),
            input,
            host_calls: exchanges
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            receipt: receipt.to_json(),
            outcome: outcome.clone(),
        });
        match result {
            Ok(_) => Ok(outcome),
            Err(error) => Err(format!("{} failed: {error}", self.id())),
        }
    }

    fn id(&self) -> String {
        format!("{}/{}", self.plugin, self.hook)
    }
}

fn profile_name(profile: plugin::Profile) -> &'static str {
    match profile {
        plugin::Profile::Pure => "pure",
        plugin::Profile::SnapshotRead => "snapshot-read",
    }
}

/// The `decide` capability: answers a guest's request through the
/// Decisions API, and keeps each exchange for the receipt.
fn decide_host(
    decider: Arc<dyn Decider>,
    asked_by: String,
    exchanges: Arc<Mutex<Vec<Exchange>>>,
) -> plugin::HostCalls {
    Arc::new(move |request: &Value| {
        let response = if request["capability"] == super::decide::TOOL {
            let decider = decider.clone();
            let args = request["args"].clone();
            let asked_by = asked_by.clone();
            // The hook runs on a thread of its own, outside any runtime.
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| -1)
                .and_then(|runtime| {
                    runtime
                        .block_on(super::decide::answer(decider.as_ref(), &args, &asked_by))
                        .map_err(|_| -2)
                })
        } else {
            Err(-2)
        };
        exchanges
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Exchange {
                request: request.clone(),
                response: response.clone(),
            });
        response
    })
}

/// The guest's value as an outcome: `permission` (`{"permission": "deny",
/// "reason"}`), `context`, `rewrite`, `stop`, each optional.
fn read_outcome(value: &Value) -> Result<HookOutcome, String> {
    let mut value = value.clone();
    if let Some(fields) = value.as_object_mut() {
        fields
            .entry("permission")
            .or_insert_with(|| json!({"permission": "no_opinion"}));
    }
    serde_json::from_value(value)
        .map_err(|error| format!("The hook's answer isn't a hook outcome: {error}"))
}

/// The outcome within `authority`: a plugin only tightens.
#[must_use]
pub fn narrow(mut outcome: HookOutcome, authority: &BTreeSet<Authority>) -> HookOutcome {
    outcome.permission = match outcome.permission {
        Permission::Deny { reason } if authority.contains(&Authority::Deny) => {
            Permission::Deny { reason }
        }
        Permission::Ask { step } if authority.contains(&Authority::Ask) => Permission::Ask { step },
        _ => Permission::NoOpinion,
    };
    if !authority.contains(&Authority::Context) {
        outcome.context = None;
    }
    if !authority.contains(&Authority::Rewrite) {
        outcome.rewrite = None;
    }
    outcome.stop = match outcome.stop {
        Stop::Block { reason } if authority.contains(&Authority::BlockStop) => {
            Stop::Block { reason }
        }
        _ => Stop::Continue,
    };
    outcome
}

/// Reruns a firing on the same module and compares it with its receipt;
/// the host answers from the exchanges the firing recorded.
#[must_use]
pub fn replay(firing: &Firing, wasm: &[u8]) -> plugin::Replay {
    let receipt = match plugin::InvocationReceipt::from_json(firing.receipt.to_string().as_bytes())
    {
        Ok(receipt) => receipt,
        Err(field) => {
            return plugin::Replay::Unverifiable {
                reason: format!("the receipt's {field} doesn't parse"),
            };
        }
    };
    let profile = if firing.profile == "snapshot-read" {
        plugin::Profile::SnapshotRead
    } else {
        plugin::Profile::Pure
    };
    let snapshot = plugin::Snapshot::default();
    let handles = BTreeMap::new();
    let call = plugin::Call {
        wasm,
        profile,
        invocation: &firing.invocation,
        operation: &firing.operation,
        input: &firing.input,
        snapshot: &snapshot,
        handles: &handles,
        limits: plugin::Limits::default(),
        cancelled: Arc::new(AtomicBool::new(false)),
        required: false,
    };
    let tape = Arc::new(Mutex::new(firing.host_calls.clone().into_iter()));
    let host: plugin::HostCalls = Arc::new(move |request: &Value| {
        match tape.lock().unwrap_or_else(PoisonError::into_inner).next() {
            Some(exchange) if &exchange.request == request => exchange.response,
            _ => Err(-2),
        }
    });
    plugin::replay_with_host(&receipt, call, host)
}

/// The plugin's listing lines for its hooks: what each does, when, what
/// it may do, what it sees, and a rewrite the person must turn on.
#[must_use]
pub fn listing(plugin: &Plugin, grants: &Grants) -> Vec<String> {
    plugin
        .hooks
        .iter()
        .map(|hook| {
            let when = match (hook.event, hook.doc.matcher.as_deref()) {
                (Event::PreToolUse, Some(tool)) => format!("before {tool}"),
                (Event::PreToolUse, None) => "before each tool".to_owned(),
                (Event::PostToolUse, Some(tool)) => format!("after {tool}"),
                (Event::PostToolUse, None) => "after each tool".to_owned(),
                (event, _) => format!("on {}", event.name()),
            };
            let may: Vec<&str> = hook
                .doc
                .authority
                .iter()
                .filter(|authority| **authority != Authority::Spend)
                .map(|authority| authority.words())
                .collect();
            let mut line = format!(
                "hook {}: {when}, may {}. Sees {}.",
                hook.doc.id,
                may.join(", "),
                disclosure(plugin, hook).sends
            );
            if !hook.doc.summary.is_empty() {
                line = format!("{line} {}", hook.doc.summary);
            }
            if hook.doc.decide {
                line.push_str(" Asks a decision model.");
            }
            if hook.doc.authority.contains(&Authority::Rewrite) {
                line.push_str(if grants.rewrites(&plugin.id, &hook.doc.id) {
                    " Changing commands: on."
                } else {
                    " Changing commands: off until you turn it on."
                });
            }
            line
        })
        .collect()
}

/// The extension evaluation path: replays one recorded session through
/// `base` alone and through `base` plus the plugin's hooks, so a plugin's
/// tests can show what its hooks change.
///
/// # Errors
///
/// A hook the scripted backend can't honor.
pub fn with_and_without(
    session: &ScriptedSession,
    base: impl Fn() -> Registry,
    plugin: &Plugin,
    grants: &Grants,
    decider: Option<Arc<dyn Decider>>,
) -> Result<(SessionReplay, SessionReplay, Vec<Note>, Firings), String> {
    let refused = |refused: Vec<super::compose::Refused>| {
        refused
            .into_iter()
            .map(|r| r.message)
            .collect::<Vec<_>>()
            .join(" ")
    };
    let without = session
        .replay(&mut Runner::new(base(), Budget::default()))
        .map_err(refused)?;
    let mut registry = base();
    let firings = Firings::default();
    let notes = register(
        &mut registry,
        plugin,
        grants,
        &admitted_by(plugin),
        decider,
        &firings,
    );
    let with = session
        .replay(&mut Runner::new(registry, Budget::default()))
        .map_err(refused)?;
    Ok((without, with, notes, firings))
}

#[cfg(test)]
#[path = "plugin_hooks_tests.rs"]
mod tests;
