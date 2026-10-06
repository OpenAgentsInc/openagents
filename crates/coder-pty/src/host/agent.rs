//! A scoped producer receives typing authority without a terminal reader.
use super::{Host, Outcome};
use crate::{
    ext::{AgentInput, AgentTypist, Handoff},
    wire::{Reason, Refusal, TerminalRef, Value},
};
use std::sync::{Arc, Weak};

/// An explicitly handed-off producer. It exposes no output, history, or pane reads.
pub struct AgentProducer {
    host: Weak<Host>,
    terminal: TerminalRef,
    binding: AgentTypist,
}
impl AgentProducer {
    /// The exact terminal task identities admitted by the owner.
    pub fn binding(&self) -> &AgentTypist {
        &self.binding
    }
    /// Sends one attributed input. Exact request retries never type twice.
    pub fn input(&self, request: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Outcome {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| Refusal::new(Reason::NotTypist, "The terminal owner is unavailable."))?;
        host.agent_input(
            &self.binding.agent,
            &AgentInput::new(
                request,
                self.terminal.clone(),
                self.binding.lease.clone(),
                bytes,
            ),
        )
    }
}
impl Host {
    /// Creates a typing-only producer after the owner's explicit handoff succeeds.
    pub fn admit_agent(
        self: &Arc<Self>,
        owner: &str,
        request: &Handoff,
    ) -> Result<AgentProducer, Refusal> {
        let (_, Value::HandedOff { lease }) = self.hand_off(owner, request)? else {
            return Err(Refusal::new(
                Reason::Malformed,
                "The host did not acknowledge the handoff.",
            ));
        };
        self.agent_producer(&request.agent, &request.terminal, &lease)
    }
    /// Connects a host-local producer to an existing private handoff lease.
    pub fn agent_producer(
        self: &Arc<Self>,
        agent: &str,
        terminal: &TerminalRef,
        lease: &str,
    ) -> Result<AgentProducer, Refusal> {
        let owned = self.inner.running(terminal)?;
        let state = owned.state();
        let seat = state
            .agent
            .as_ref()
            .filter(|seat| seat.typist.agent == agent && seat.typist.lease == lease)
            .ok_or_else(|| {
                Refusal::new(
                    Reason::NotTypist,
                    "No current handoff admits this producer.",
                )
            })?;
        let attachment = state
            .attachments
            .get(&seat.by)
            .ok_or_else(|| Refusal::new(Reason::NotAdmitted, "The handing attachment ended."))?;
        self.inner
            .require(&attachment.principal, super::Right::Terminal)?;
        Ok(AgentProducer {
            host: Arc::downgrade(self),
            terminal: terminal.clone(),
            binding: seat.typist.clone(),
        })
    }
}

/// One generated input, pinned to the admitted terminal, thread, and run.
/// Its bytes are input only; this type contains no observation or execution request.
#[derive(Clone)]
pub struct GeneratedInput {
    pub request: String,
    pub terminal: TerminalRef,
    pub thread: String,
    pub run: String,
    pub data: Vec<u8>,
}

/// An explicit private evidence directory. It never selects the user's home.
pub struct PrivateEvidence {
    root: std::path::PathBuf,
}

#[derive(serde::Serialize)]
struct InputRecord<'a> {
    v: &'static str,
    terminal: &'a TerminalRef,
    binding: &'a AgentTypist,
    request: &'a str,
    outcome: &'a str,
    bytes: u64,
}

impl PrivateEvidence {
    /// Opens an existing private directory supplied by the task owner.
    /// Unix directories must exclude group and other access. Other platforms
    /// refuse until their private directory permissions can be validated.
    pub fn open(root: impl Into<std::path::PathBuf>) -> Result<Self, Refusal> {
        let root = root.into();
        private_directory(&root)?;
        Ok(Self { root })
    }
    fn begin(
        &self,
        terminal: &TerminalRef,
        binding: &AgentTypist,
        request: &str,
    ) -> Result<std::fs::File, Refusal> {
        // Recheck directories before each write, and never follow a record symlink.
        private_directory(&self.root)?;
        let thread = self.root.join(&binding.thread);
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&thread) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(evidence_error(error)),
        }
        private_directory(&thread)?;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(thread.join(format!("{request}.jsonl")))
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    Refusal::new(Reason::Stale, "This private input attempt already exists.")
                } else {
                    evidence_error(error)
                }
            })?;
        append(&mut file, terminal, binding, request, "unknown", 0)?;
        Ok(file)
    }
}

impl AgentProducer {
    /// Sends one bounded generated input and records its byte-free private evidence.
    /// An attempt is synced before input, so interruption preserves an unknown effect.
    /// Existing attempt IDs are never replayed, including after host restart.
    pub fn produce(&self, input: &GeneratedInput, evidence: &PrivateEvidence) -> Outcome {
        if input.terminal != self.terminal
            || input.thread != self.binding.thread
            || input.run != self.binding.run
        {
            return Err(Refusal::new(
                Reason::NotAdmitted,
                "Generated input must name the admitted terminal, thread, and run.",
            ));
        }
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| Refusal::new(Reason::NotTypist, "The terminal owner is unavailable."))?;
        AgentInput::new(
            &input.request,
            input.terminal.clone(),
            &self.binding.lease,
            input.data.clone(),
        )
        .check_with(host.features())?;
        let mut record = evidence.begin(&self.terminal, &self.binding, &input.request)?;
        let result = self.input(&input.request, input.data.clone());
        let (outcome, bytes) = match &result {
            Ok((_, Value::Written { bytes })) => ("written", *bytes),
            Err(refusal) if refusal.reason == Reason::Unavailable => ("unknown", 0),
            Err(_) => ("refused", 0),
            _ => ("unknown", 0),
        };
        append(
            &mut record,
            &self.terminal,
            &self.binding,
            &input.request,
            outcome,
            bytes,
        )?;
        result
    }
}

fn private_directory(path: &std::path::Path) -> Result<(), Refusal> {
    if !cfg!(unix) {
        return Err(Refusal::new(
            Reason::UnsupportedFeature,
            "Private evidence requires validated directory permissions on this platform.",
        ));
    }
    let metadata = std::fs::symlink_metadata(path).map_err(evidence_error)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Refusal::new(
            Reason::NotAdmitted,
            "Evidence requires an existing private directory, without symlinks.",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(Refusal::new(
                Reason::NotAdmitted,
                "Evidence directory permissions must exclude group and other access.",
            ));
        }
    }
    Ok(())
}

fn append(
    file: &mut std::fs::File,
    terminal: &TerminalRef,
    binding: &AgentTypist,
    request: &str,
    outcome: &str,
    bytes: u64,
) -> Result<(), Refusal> {
    use std::io::Write;
    serde_json::to_writer(
        &mut *file,
        &InputRecord {
            v: "openagents.terminal-agent-input-evidence.v1",
            terminal,
            binding,
            request,
            outcome,
            bytes,
        },
    )
    .map_err(evidence_error)?;
    file.write_all(b"\n").map_err(evidence_error)?;
    file.sync_all().map_err(evidence_error)
}

fn evidence_error(error: impl std::fmt::Display) -> Refusal {
    Refusal::new(
        Reason::Unavailable,
        format!("Private input evidence could not be retained: {error}"),
    )
}
