//! The live receipt replayer: reruns a Wasm guest a run recorded and
//! compares the outcome.
//!
//! A `receipt` grader asks whether the guest invocation a run recorded
//! replays exactly on this host. The subject's program carries the guest
//! inline (its `module` step's `bytes_base64`, operation, input, and read
//! scope), and the run's trajectory records what the step returned. For
//! each recorded call of the operation, the replayer rebuilds the same
//! invocation — the same module bytes, invocation id (the step name),
//! operation, input, limits, and a snapshot of the run's workspace taken
//! the way Coder's runtime takes it — reruns it with `plugin`, and compares
//! the status and the digest of the canonical value with the recorded
//! ones. Coder's trajectory doesn't record the guest's fuel, so the replay
//! compares the outcome only. A call whose output can't be read is
//! `unverifiable`, never a pass.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::Value;

use crate::arms::Subject;
use crate::door::{ReplayVerdict, Replayer, RunKey};
use crate::record::Arm;
use crate::trajectory::Trajectory;

/// The fuel a `module` step gets when the operator sets no ceiling: the
/// ceiling Coder's runtime holds every module step to.
pub const MODULE_FUEL: u64 = 50_000_000;
/// The most workspace entries one snapshot holds, as Coder's runtime.
pub const SNAPSHOT_ENTRIES: usize = 1_024;
/// The most bytes one snapshot captures, as Coder's runtime.
pub const SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;
/// The snapshot's root entry and handle name, as Coder's runtime.
pub const SNAPSHOT_ROOT: &str = "workspace";

/// One `module` step of one of the subject's programs.
#[derive(Clone, Debug)]
pub struct ModuleStep {
    /// The step's name, which is also the invocation id.
    pub name: String,
    /// The guest's operation.
    pub operation: String,
    /// The guest's bytes.
    pub wasm: Vec<u8>,
    /// Whether the guest reads a snapshot.
    pub snapshot_read: bool,
    /// The operation input the program fixes.
    pub input: Value,
    /// The workspace-relative read scope.
    pub read: Vec<String>,
    /// The step's declared bounds.
    pub bounds: Value,
}

/// Replays the guests of one subject against finished runs.
#[derive(Default)]
pub struct ModuleReplayer {
    steps: Vec<ModuleStep>,
    runs: BTreeMap<(String, Arm, u32), (PathBuf, Option<Trajectory>)>,
}

impl ModuleReplayer {
    /// A replayer over every inline `module` step the subject's programs
    /// carry.
    #[must_use]
    pub fn new(subject: &Subject) -> Self {
        let mut steps = Vec::new();
        for program in &subject.programs {
            let Ok(value) = serde_json::from_slice::<Value>(&program.bytes) else {
                continue;
            };
            let Some(bound) = value.pointer("/binding/steps").and_then(Value::as_object) else {
                continue;
            };
            let declared = value
                .pointer("/definition/steps")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for (name, binding) in bound {
                let Some(module) = binding.get("module") else {
                    continue;
                };
                let Some(wasm) = module
                    .get("bytes_base64")
                    .and_then(Value::as_str)
                    .and_then(|text| plugin::decode_base64(text).ok())
                else {
                    continue;
                };
                let definition_bounds = declared
                    .iter()
                    .find(|step| step.get("name").and_then(Value::as_str) == Some(name))
                    .and_then(|step| step.get("bounds"))
                    .cloned()
                    .unwrap_or(Value::Null);
                let bounds = match binding.get("bounds") {
                    Some(bounds) if bounds.as_object().is_some_and(|b| !b.is_empty()) => {
                        bounds.clone()
                    }
                    _ => definition_bounds,
                };
                steps.push(ModuleStep {
                    name: name.clone(),
                    operation: module
                        .get("operation")
                        .and_then(Value::as_str)
                        .unwrap_or("echo")
                        .to_string(),
                    wasm,
                    snapshot_read: module.get("profile").and_then(Value::as_str)
                        == Some("snapshot-read"),
                    input: module.get("input").cloned().unwrap_or(Value::Null),
                    read: module
                        .get("read")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default(),
                    bounds,
                });
            }
        }
        Self {
            steps,
            runs: BTreeMap::new(),
        }
    }

    /// Records a finished run's workspace and trajectory.
    pub fn add_run(
        &mut self,
        key: (String, Arm, u32),
        workspace: PathBuf,
        trajectory: Option<Trajectory>,
    ) {
        self.runs.insert(key, (workspace, trajectory));
    }

    /// The steps the replayer knows.
    #[must_use]
    pub fn steps(&self) -> &[ModuleStep] {
        &self.steps
    }
}

/// The outputs a trajectory recorded for calls named `name`, in order.
#[must_use]
pub fn recorded_outputs(trajectory: &Trajectory, name: &str) -> Vec<String> {
    let mut outputs = Vec::new();
    for step in trajectory.steps() {
        if let Some(calls) = step.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let named = call.get("function_name").and_then(Value::as_str) == Some(name)
                    || call.pointer("/extra/step").and_then(Value::as_str) == Some(name);
                if !named {
                    continue;
                }
                let id = call.get("tool_call_id").and_then(Value::as_str);
                let content = step
                    .pointer("/observation/results")
                    .and_then(Value::as_array)
                    .and_then(|results| {
                        results.iter().find(|result| {
                            result.get("source_call_id").and_then(Value::as_str) == id
                        })
                    })
                    .and_then(|result| result.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                outputs.push(content.to_string());
            }
        }
        if let Some(call) = step.pointer("/extra/call")
            && call.get("function_name").and_then(Value::as_str) == Some(name)
        {
            outputs.push(
                call.get("content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            );
        }
    }
    outputs
}

impl ModuleStep {
    fn limits(&self) -> plugin::Limits {
        let ceiling = plugin::Limits {
            fuel: MODULE_FUEL,
            ..plugin::Limits::default()
        };
        let declared = |bound: &str, ceiling: usize| {
            self.bounds
                .get(bound)
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .map_or(ceiling, |count| count.min(ceiling))
        };
        plugin::Limits {
            fuel: self
                .bounds
                .get("fuel")
                .and_then(Value::as_u64)
                .map_or(ceiling.fuel, |fuel| fuel.min(ceiling.fuel)),
            memory_bytes: declared("memory_bytes", ceiling.memory_bytes),
            output_bytes: declared("output_bytes", ceiling.output_bytes),
            read_bytes: declared("read_bytes", ceiling.read_bytes),
            module_bytes: declared("module_bytes", ceiling.module_bytes),
        }
    }

    /// Reruns the guest on `workspace` and compares with `recorded`.
    fn replay(&self, workspace: &Path, recorded: &str) -> ReplayVerdict {
        let Ok(recorded) = serde_json::from_str::<Value>(recorded) else {
            return ReplayVerdict::Unverifiable {
                reason: "the trajectory recorded no guest output for the call".into(),
            };
        };
        let (Some(status), Some(value)) = (
            recorded.get("status").and_then(Value::as_str),
            recorded.get("value"),
        ) else {
            return ReplayVerdict::Unverifiable {
                reason: "the recorded output has no status and value".into(),
            };
        };
        let expected = format!(
            "{status} {}",
            plugin::digest(plugin::canonical(value).as_bytes())
        );
        let limits = self.limits();
        let (snapshot, handles) = if self.snapshot_read {
            match capture(workspace, &self.read, limits.read_bytes) {
                Ok(captured) => captured,
                Err(reason) => return ReplayVerdict::Unverifiable { reason },
            }
        } else {
            (plugin::Snapshot::default(), BTreeMap::new())
        };
        let result = plugin::invoke(plugin::Call {
            wasm: &self.wasm,
            profile: if self.snapshot_read {
                plugin::Profile::SnapshotRead
            } else {
                plugin::Profile::Pure
            },
            invocation: &self.name,
            operation: &self.operation,
            input: &self.input,
            snapshot: &snapshot,
            handles: &handles,
            limits,
            cancelled: Arc::new(AtomicBool::new(false)),
            required: true,
        });
        let actual = match result {
            Ok(value) => format!(
                "{} {}",
                value.status,
                plugin::digest(plugin::canonical(&value.value).as_bytes())
            ),
            Err(error) => format!("error {error}"),
        };
        if actual == expected {
            ReplayVerdict::Passed
        } else {
            ReplayVerdict::Failed {
                field: "outcome".into(),
                expected,
                actual,
            }
        }
    }
}

impl Replayer for ModuleReplayer {
    fn replay(&self, run: RunKey<'_>, operation: &str) -> Result<Vec<ReplayVerdict>, String> {
        let Some(step) = self
            .steps
            .iter()
            .find(|step| step.name == operation || step.operation == operation)
        else {
            return Err(format!(
                "the extension has no inline Wasm guest step named {operation}"
            ));
        };
        let Some((workspace, trajectory)) =
            self.runs.get(&(run.case.to_string(), run.arm, run.attempt))
        else {
            return Ok(Vec::new());
        };
        let Some(trajectory) = trajectory else {
            return Ok(Vec::new());
        };
        Ok(recorded_outputs(trajectory, &step.name)
            .iter()
            .map(|recorded| step.replay(workspace, recorded))
            .collect())
    }
}

/// The snapshot a `snapshot-read` step is granted, taken the way Coder's
/// runtime takes it: every file its scope names under the workspace, in
/// path order, each kept up to `read_bytes`, symlinks listed and never
/// followed.
///
/// # Errors
///
/// Returns why the scope can't be captured.
pub fn capture(
    workspace: &Path,
    scope: &[String],
    read_bytes: usize,
) -> Result<(plugin::Snapshot, BTreeMap<String, String>), String> {
    let root = workspace
        .canonicalize()
        .map_err(|error| format!("the workspace can't be read: {error}"))?;
    let mut capture = Capture {
        root: &root,
        read_bytes,
        captured: 0,
        entries: BTreeMap::new(),
    };
    for path in scope {
        let joined = if path == "." {
            root.clone()
        } else {
            root.join(path)
        };
        capture.walk(&joined)?;
    }
    let mut snapshot = plugin::Snapshot::default();
    let mut children = Vec::with_capacity(capture.entries.len());
    for (relative, entry) in capture.entries {
        let label = format!("{SNAPSHOT_ROOT}/{relative}");
        snapshot.insert(&label, entry).map_err(str::to_string)?;
        children.push(label);
    }
    snapshot
        .insert(SNAPSHOT_ROOT, plugin::Entry::Directory { children })
        .map_err(str::to_string)?;
    let handles = BTreeMap::from([(SNAPSHOT_ROOT.to_string(), "root".to_string())]);
    Ok((snapshot, handles))
}

struct Capture<'a> {
    root: &'a Path,
    read_bytes: usize,
    captured: usize,
    entries: BTreeMap<String, plugin::Entry>,
}

impl Capture<'_> {
    fn relative(&self, path: &Path) -> Result<String, String> {
        let relative = path
            .strip_prefix(self.root)
            .map_err(|_| format!("{} is outside the workspace", path.display()))?;
        let mut parts = Vec::new();
        for part in relative.components() {
            match part {
                std::path::Component::Normal(part) => parts.push(
                    part.to_str()
                        .ok_or_else(|| format!("{} isn't a UTF-8 path", path.display()))?,
                ),
                _ => return Err(format!("{} isn't a plain path", path.display())),
            }
        }
        Ok(parts.join("/"))
    }

    fn inside(&self, path: &Path) -> Result<PathBuf, String> {
        let resolved = path
            .canonicalize()
            .map_err(|error| format!("{} can't be read: {error}", path.display()))?;
        if resolved.starts_with(self.root) {
            Ok(resolved)
        } else {
            Err(format!("{} resolves outside the workspace", path.display()))
        }
    }

    fn add(&mut self, relative: String, entry: plugin::Entry) -> Result<(), String> {
        if relative.is_empty() || self.entries.contains_key(&relative) {
            return Ok(());
        }
        if self.entries.len() >= SNAPSHOT_ENTRIES {
            return Err(format!(
                "the read scope names more than {SNAPSHOT_ENTRIES} entries"
            ));
        }
        self.entries.insert(relative, entry);
        Ok(())
    }

    fn walk(&mut self, path: &Path) -> Result<(), String> {
        self.inside(path)?;
        let relative = self.relative(path)?;
        let meta = std::fs::symlink_metadata(path)
            .map_err(|error| format!("{} can't be read: {error}", path.display()))?;
        if meta.file_type().is_symlink() {
            let target = self.inside(path)?;
            let target = self.relative(&target)?;
            return self.add(relative, plugin::Entry::Symlink { target });
        }
        if meta.is_dir() {
            let mut children = std::fs::read_dir(path)
                .and_then(|entries| {
                    entries
                        .map(|entry| entry.map(|entry| entry.path()))
                        .collect::<Result<Vec<_>, _>>()
                })
                .map_err(|error| format!("{} can't be listed: {error}", path.display()))?;
            children.sort();
            for child in children {
                self.walk(&child)?;
            }
            return Ok(());
        }
        if !meta.is_file() {
            return Ok(());
        }
        let size = usize::try_from(meta.len()).unwrap_or(usize::MAX);
        let keep = size.min(self.read_bytes);
        if self.captured.saturating_add(keep) > SNAPSHOT_BYTES {
            return Err(format!(
                "the read scope captures more than {SNAPSHOT_BYTES} bytes"
            ));
        }
        let mut bytes = Vec::with_capacity(keep);
        std::io::Read::read_to_end(
            &mut std::io::Read::take(
                std::fs::File::open(path)
                    .map_err(|error| format!("{} can't be read: {error}", path.display()))?,
                keep as u64,
            ),
            &mut bytes,
        )
        .map_err(|error| format!("{} can't be read: {error}", path.display()))?;
        self.captured += bytes.len();
        let complete = bytes.len() == size;
        let version = plugin::digest(&bytes);
        self.add(
            relative,
            plugin::Entry::File {
                bytes,
                version,
                complete,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::arms::Program;

    fn wasm() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plugin/fixtures/pure.wasm"
        ))
        .expect("the pure guest fixture")
    }

    fn subject() -> Subject {
        let program = json!({
            "definition": {"id": "a:b/echo", "steps": [{"name": "echo", "kind": "module", "bounds": {}}]},
            "binding": {"steps": {"echo": {"module": {
                "profile": "pure",
                "operation": "echo",
                "input": {"topic": "notes"},
                "bytes_base64": plugin::encode_base64(&wasm()),
            }}}},
        });
        Subject {
            slug: "echo".into(),
            definition: json!({}),
            package_lock: json!({}),
            programs: vec![Program {
                slug: "echo".into(),
                bytes: serde_json::to_vec(&program).unwrap(),
            }],
            skills: Vec::new(),
        }
    }

    fn trajectory(output: &str) -> Trajectory {
        let session = atif::Session::opening("s", "m", "d", "/w", "0");
        let mut step = atif::Step::said(atif::Source::System, "Ran the echo guest.");
        step.call = Some(atif::Call {
            id: "c1".into(),
            name: "echo".into(),
            arguments: json!({"operation": "echo"}),
            output: output.into(),
            outcome: atif::Outcome::Completed,
            milliseconds: 1,
            purpose: None,
            extra: serde_json::Map::new(),
        });
        let document = atif::document(&session, &[step]);
        Trajectory::from_bytes(&serde_json::to_vec(&document).unwrap()).unwrap()
    }

    fn replay(output: &str) -> Vec<ReplayVerdict> {
        let workspace = tempfile::tempdir().unwrap();
        let mut replayer = ModuleReplayer::new(&subject());
        replayer.add_run(
            ("case".into(), Arm::Subject, 1),
            workspace.path().to_path_buf(),
            Some(trajectory(output)),
        );
        replayer
            .replay(
                RunKey {
                    case: "case",
                    arm: Arm::Subject,
                    attempt: 1,
                },
                "echo",
            )
            .unwrap()
    }

    #[test]
    fn a_recorded_guest_call_replays_exactly_and_a_changed_one_fails() {
        let recorded = plugin::invoke(plugin::Call {
            wasm: &wasm(),
            profile: plugin::Profile::Pure,
            invocation: "echo",
            operation: "echo",
            input: &json!({"topic": "notes"}),
            snapshot: &plugin::Snapshot::default(),
            handles: &BTreeMap::new(),
            limits: plugin::Limits {
                fuel: MODULE_FUEL,
                ..plugin::Limits::default()
            },
            cancelled: Arc::new(AtomicBool::new(false)),
            required: true,
        })
        .unwrap();
        let output = json!({"status": recorded.status, "value": recorded.value}).to_string();
        assert_eq!(replay(&output), vec![ReplayVerdict::Passed]);
        let changed = json!({"status": recorded.status, "value": {"topic": "other"}}).to_string();
        assert!(matches!(
            replay(&changed).as_slice(),
            [ReplayVerdict::Failed { .. }]
        ));
        assert!(matches!(
            replay("not the guest's output").as_slice(),
            [ReplayVerdict::Unverifiable { .. }]
        ));
    }

    #[test]
    fn an_operation_the_extension_lacks_is_an_error_and_no_call_is_no_receipt() {
        let replayer = ModuleReplayer::new(&subject());
        let key = RunKey {
            case: "case",
            arm: Arm::Subject,
            attempt: 1,
        };
        assert!(replayer.replay(key, "search").is_err());
        assert_eq!(replayer.replay(key, "echo").unwrap(), Vec::new());
    }
}
