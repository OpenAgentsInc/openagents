//! The workshop agent (`docs/verse/workshop-agent.md`).
//!
//! One named agent, such as `alice`, with a private record and an
//! append-only journal under the host's root:
//! `~/.openagents/host/agents/NAME/agent.json` (mode `0600`) and
//! `journal.jsonl`. Both survive a restart of the host and of Verse. The
//! record names the agent's own Nostr key, which the host keeps in its
//! secret store ([`super::agent_key`]: the keychain when the host runs
//! with `--keychain`, else `key` beside the record, mode `0600`), and the
//! owner's NIP-OA attestation of that key ([`Attestation`]); it also holds
//! her definition ([`Definition`]), her roles ([`Roles`]), and her state:
//! active, paused, stopped, retired, or moved ([`State`]). Rotation,
//! retirement, moves, and snapshots are in [`super::agent_lifecycle`].
//!
//! A terminal-mode request runs as a turn of Coder V1 in the agent's own
//! session (`super::agent_host`, `super::coder_v1`): each command Coder
//! proposes gets an effect class before it runs ([`effect`]), and anything
//! that is not read-only waits for the owner's CONFIRM or REJECT. The run
//! ends with a short plain-ASCII report ([`plain`]) and a headline drawn
//! from what ran, never from the model's words.
//!
//! The journal holds requests, commands, decisions, and reports, never
//! command output, and every entry passes [`screen`] first, so a
//! credential-shaped word never reaches it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub use microcoder_loop::models::NextAction;

use super::agent_key::{KeyStore, Slot};
pub use super::agent_preset::{PRESETS, Preset, Pronouns, Refer, capitalize, preset};

/// The agent record's schema.
pub const RECORD_SCHEMA: &str = "openagents.workshop-agent.v1";
/// One journal entry's schema.
pub const JOURNAL_SCHEMA: &str = "openagents.agent-journal-entry.v1";
/// The workshop agent: Alice, who sits at the last desk in the workshop.
pub const DEFAULT_NAME: &str = "alice";
/// The phase 1 demo's name for her, whose record [`Store::open`] moves to
/// [`DEFAULT_NAME`] when it finds one.
pub const LEGACY_NAME: &str = "ada";
/// The character look a new agent is drawn with: Alice's own character,
/// the Everglade pack's form `npc/alice`.
pub const DEFAULT_LOOK: &str = "alice";
/// The charter a new agent starts with: the owner's defaults until they
/// say otherwise (`docs/verse/workshop-agent.md`, "Open questions").
pub const DEFAULT_CHARTER: &str = "Terminal mode may run read-only commands anywhere in the \
     workspace without asking; any other command waits for the owner's CONFIRM or REJECT. Task \
     mode changes files only in her own worktree, and the owner merges at the Merge station, or \
     she does when the owner asks her to. Never push, publish, pay, or read credentials.";
/// The longest an attestation may last: a year.
pub const ATTESTATION_MAX: u64 = 366 * 24 * 60 * 60;
/// The most characters a report keeps.
pub const REPLY_MAX: usize = 600;
/// The most bytes a request may carry, as `studio.agent.ask` allows.
pub const TEXT_MAX: usize = 16 * 1024;
/// The most bytes one journal entry's text keeps.
const ENTRY_MAX: usize = 2048;

/// The [`Record::engine`] where Coder delegates her coding to Codex.
pub const ENGINE_CODEX: &str = "codex";

/// The [`Record::engine`] where Coder delegates her coding to the Devin
/// CLI (`devin`), or `devin:MODEL` for one of the models `devin models
/// list` names.
pub const ENGINE_DEVIN: &str = "devin";

/// The engine `word` names, as a record keeps it: `codex`, `devin` or
/// `devin:MODEL`, or empty for `coder`, Coder's own model.
///
/// # Errors
/// Any other word, or a `devin:` model with characters a Devin model name
/// never carries.
pub fn parse_engine(word: &str) -> Result<String, String> {
    match word {
        ENGINE_CODEX => Ok(ENGINE_CODEX.into()),
        ENGINE_DEVIN => Ok(ENGINE_DEVIN.into()),
        "coder" | "" => Ok(String::new()),
        other if other.starts_with("devin:") => {
            let model = &other["devin:".len()..];
            if !model.is_empty()
                && model
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            {
                Ok(other.into())
            } else {
                Err(format!(
                    "{other} isn't an engine; `devin:` takes a model name such as swe-2-high"
                ))
            }
        }
        other => Err(format!(
            "{other} isn't an engine; use coder (Coder's own model), codex, or devin[:MODEL]"
        )),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesModelScope {
    pub floor: String,
    pub actor: String,
}

/// The agent's standing record (`openagents.workshop-agent.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema: String,
    pub v: u32,
    /// Features a reader must know to read the record; none in v1.
    pub requires: Vec<String>,
    pub name: String,
    /// What it may do, in words. It only narrows the host's permit.
    pub charter: String,
    /// The directory its terminal opens in.
    pub workspace: String,
    /// The character look a view draws it with.
    pub look: String,
    /// When the record was made, Unix seconds.
    pub created_at: u64,
    /// Its own Nostr key's public half, 64 lowercase hex characters. The
    /// secret half stays in `key` beside the record and never leaves the
    /// host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,
    /// The owner's NIP-OA attestation of that key. It proves ownership and
    /// grants nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<Attestation>,
    /// Whether it takes new work.
    #[serde(default, skip_serializing_if = "State::is_active")]
    pub state: State,
    /// The route it plans with, such as `codex/loop:gpt-6-luna`; empty is
    /// the first provider with capacity.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub route: String,
    /// What does her coding: empty for Coder's own model, or
    /// [`ENGINE_CODEX`], where Coder delegates the coding to the Codex
    /// agent on the owner's ChatGPT login and checks its work.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub engine: String,
    /// Its desk in the workshop hall.
    #[serde(default = "default_desk")]
    pub desk: u32,
    /// Who she is to the people she answers: NIP-AP's fields, kept private
    /// on the host. A record from before phase 4 gains one when the host
    /// opens it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<Definition>,
    /// Who governs, runs, and keeps her key, in NIP-SOV's words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles: Option<Roles>,
    /// A private job description, distinct from SOV authority roles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_role: Option<coder_host::access::crew::JobRole>,
    /// Enforced sales scope; templates cannot widen host access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crew_charter: Option<coder_host::access::crew::Charter>,
    /// Stable expense scope carried across key rotation and host migration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sales_model_scope: Option<SalesModelScope>,
}

fn default_desk() -> u32 {
    3
}

/// Her definition: NIP-AP's persona fields, never published in v1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    /// The name people see, such as `Alice`.
    pub display_name: String,
    /// How she speaks, in words; empty is plain and brief.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub voice: String,
    /// The system prompt her own model calls start from; empty is the
    /// host's own, from her name and charter. Her charter always follows
    /// it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub system_prompt: String,
    /// The route her own calls use; empty is the record's route.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub route: String,
    /// Whom she answers. Only [`RESPOND_TO_OWNER`] in v1, which the host
    /// enforces.
    #[serde(default = "owner_only")]
    pub respond_to: String,
    /// How the host's sentences refer to the agent; none is its preset's,
    /// else by name ([`super::agent_preset`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pronouns: Option<Pronouns>,
}

/// NIP-AP's `respond_to` for an agent that answers only her owner.
pub const RESPOND_TO_OWNER: &str = "owner-only";

fn owner_only() -> String {
    RESPOND_TO_OWNER.into()
}

/// Who governs, runs, and keeps an agent, in NIP-SOV's words. Each is a
/// public key, 64 lowercase hex characters, or [`THIS_HOST`] for the host
/// whose root holds her record, so a later move to another custodian is a
/// record change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roles {
    /// Who admits her policy: the owner whose attestation she carries;
    /// empty until the owner attests her key.
    #[serde(default)]
    pub authority: String,
    /// Who runs her lifecycle.
    pub controller: String,
    /// Who keeps her key and signs for her.
    pub custodian: String,
}

/// A role the host whose root holds the record plays.
pub const THIS_HOST: &str = "host";

impl Definition {
    /// The definition a record without one gets: its preset's
    /// ([`super::agent_preset`]), else its name, capitalized, and the
    /// host's defaults for the rest.
    #[must_use]
    pub fn default_for(name: &str) -> Self {
        if let Some(preset) = preset(name) {
            return preset.definition();
        }
        Self {
            display_name: capitalize(name),
            voice: String::new(),
            system_prompt: String::new(),
            route: String::new(),
            respond_to: owner_only(),
            pronouns: None,
        }
    }
}

impl Record {
    /// Her definition, or the default one when the record has none.
    #[must_use]
    pub fn definition(&self) -> Definition {
        self.definition
            .clone()
            .unwrap_or_else(|| Definition::default_for(&self.name))
    }

    /// How the host's sentences refer to it: its definition's pronouns,
    /// else its preset's, else by name.
    #[must_use]
    pub fn refer(&self) -> Refer {
        let pronouns = self
            .definition
            .as_ref()
            .and_then(|d| d.pronouns)
            .or_else(|| preset(&self.name).map(|p| p.pronouns))
            .unwrap_or_default();
        Refer::new(pronouns, &self.display_name())
    }

    /// Its job on the crew, in a few words: its preset's role, else
    /// `workshop agent`.
    #[must_use]
    pub fn role(&self) -> &'static str {
        self.job_role.map_or_else(
            || {
                preset(&self.name)
                    .filter(|p| p.job_role.is_none())
                    .map_or("workshop agent", |p| p.role)
            },
            |role| role.name(),
        )
    }

    /// Whether Coder delegates its coding to Codex.
    #[must_use]
    pub fn codes_on_codex(&self) -> bool {
        self.engine == ENGINE_CODEX
    }

    /// Whether Coder delegates its coding to the Devin CLI.
    #[must_use]
    pub fn codes_on_devin(&self) -> bool {
        self.engine == ENGINE_DEVIN || self.devin_model().is_some()
    }

    /// The model `devin:MODEL` names, if the engine names one.
    #[must_use]
    pub fn devin_model(&self) -> Option<&str> {
        self.engine.strip_prefix("devin:")
    }

    /// The name people see.
    #[must_use]
    pub fn display_name(&self) -> String {
        let name = self.definition().display_name;
        if name.trim().is_empty() {
            self.name.clone()
        } else {
            name
        }
    }

    /// Fills a missing definition and roles with their defaults, and the
    /// authority from her attestation. Returns whether anything changed.
    pub fn fill_identity(&mut self) -> bool {
        let mut changed = false;
        if self.definition.is_none() {
            self.definition = Some(Definition::default_for(&self.name));
            changed = true;
        }
        let owner = self
            .attestation
            .as_ref()
            .map(|a| a.owner.clone())
            .unwrap_or_default();
        match &mut self.roles {
            None => {
                self.roles = Some(Roles {
                    authority: owner,
                    controller: THIS_HOST.into(),
                    custodian: THIS_HOST.into(),
                });
                changed = true;
            }
            Some(roles) if !owner.is_empty() && roles.authority != owner => {
                roles.authority = owner;
                changed = true;
            }
            Some(_) => {}
        }
        changed
    }
}

/// Whether an agent takes new work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// It takes requests and runs its standing jobs.
    #[default]
    Active,
    /// It keeps everything and starts nothing new.
    Paused,
    /// The owner stopped it: its jobs are off, its work cancelled, and it
    /// starts nothing until resumed.
    Stopped,
    /// Retired: its key is gone and its journal stays.
    Retired,
    /// Moved to another of the owner's computers, which runs her now:
    /// this one refuses her requests and runs none of her jobs.
    Moved,
}

impl State {
    #[must_use]
    pub fn is_active(&self) -> bool {
        *self == Self::Active
    }

    /// Whether this computer no longer runs her: retired or moved.
    #[must_use]
    pub fn is_gone(self) -> bool {
        matches!(self, Self::Retired | Self::Moved)
    }

    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
            Self::Retired => "retired",
            Self::Moved => "moved",
        }
    }
}

/// A NIP-OA `auth` tag the owner signed for the agent's key: `["auth",
/// owner, conditions, signature]`, where the signature is the owner's
/// Schnorr signature of `SHA-256("nostr:agent-auth:" || agent || ":" ||
/// conditions)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attestation {
    /// The owner's public key, 64 lowercase hex characters.
    pub owner: String,
    /// `created_at<EXPIRY`: it covers events made before its expiry.
    pub conditions: String,
    pub signature: String,
}

/// What a journal entry records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Created,
    /// The record moved from the phase 1 name.
    Migrated,
    /// Its key was made, or the owner attested it.
    Keyed,
    Request,
    Plan,
    Typed,
    Ran,
    Proposed,
    Confirmed,
    Rejected,
    Refused,
    Takeback,
    Report,
    Failed,
    /// One step of the stop sequence, or a pause, resume, or retirement.
    Control,
    /// A task-mode change: made, waiting at the Merge station, merged, or
    /// rejected.
    Task,
    /// A memory entry written, accepted, rejected, or forgotten.
    Memory,
    /// A standing job's occurrence, or its refusal.
    Job,
    /// A prompt she gave Coder, screened.
    Prompt,
    /// Her judgment of a Coder turn: Jev's answers, or the rule's when
    /// Jev isn't set up, and the move she chose.
    Judgment,
}

/// One journal line (`openagents.agent-journal-entry.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub schema: String,
    /// Unix seconds.
    pub at: u64,
    pub kind: Kind,
    /// Plain ASCII, screened, at most 2 KiB.
    pub text: String,
    /// A finished command's exit status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
    /// Who sent a request: a device key, `owner`, or `job:ID`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

impl Entry {
    #[must_use]
    pub fn new(at: u64, kind: Kind, text: &str) -> Self {
        Self {
            schema: JOURNAL_SCHEMA.into(),
            at,
            kind,
            text: bounded(&screen(text), ENTRY_MAX),
            status: None,
            from: None,
        }
    }
}

/// Whether `name` is an agent name: lowercase letters, digits, and
/// hyphens, at most 32 bytes, as a studio seat name.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `path` as a workspace for an agent: an absolute directory inside a Git
/// checkout, canonical. The error is a plain sentence for the owner.
///
/// # Errors
/// When `path` is relative, does not exist, is not a directory, or is in
/// no Git checkout.
pub fn checkout(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{} is not a full path.", path.display()));
    }
    let Ok(canonical) = path.canonicalize() else {
        return Err(format!("{} does not exist.", path.display()));
    };
    if !canonical.is_dir() {
        return Err(format!("{} is not a folder.", path.display()));
    }
    if !canonical.ancestors().any(|dir| dir.join(".git").exists()) {
        return Err(format!("{} is not in a Git repository.", path.display()));
    }
    Ok(canonical)
}

/// The host's root on this computer, `~/.openagents/host`.
#[must_use]
pub fn host_root() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/host"))
}

/// One agent's directory under a host root, and the store her key lives
/// in.
#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
    name: String,
    keys: Arc<dyn KeyStore>,
}

impl Store {
    /// The store of agent `name` under `host_root` (`agents/NAME`), with
    /// her key in the process's key store ([`super::agent_key::installed`]).
    ///
    /// # Errors
    /// When `name` is not an agent name.
    pub fn new(host_root: &Path, name: &str) -> Result<Self, String> {
        Self::with_keys(host_root, name, super::agent_key::installed())
    }

    /// The store of agent `name` under `host_root`, with her key in
    /// `keys`.
    ///
    /// # Errors
    /// When `name` is not an agent name.
    pub fn with_keys(
        host_root: &Path,
        name: &str,
        keys: Arc<dyn KeyStore>,
    ) -> Result<Self, String> {
        if !valid_name(name) {
            return Err(format!(
                "`{name}` is not an agent name: lowercase letters, digits, and hyphens"
            ));
        }
        Ok(Self {
            dir: host_root.join("agents").join(name),
            name: name.into(),
            keys,
        })
    }

    /// Where her key lives: `file` or `keychain`.
    #[must_use]
    pub fn custody_kind(&self) -> &'static str {
        self.keys.custody()
    }

    fn slot(&self) -> Slot<'_> {
        Slot {
            name: &self.name,
            dir: &self.dir,
        }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How the host's sentences refer to this agent: its record's words,
    /// or its preset's before it has a readable record.
    #[must_use]
    pub fn refer(&self) -> Refer {
        match self.load() {
            Ok(Some(record)) => record.refer(),
            _ => Refer::for_name(&self.name),
        }
    }

    fn record_path(&self) -> PathBuf {
        self.dir.join("agent.json")
    }

    fn journal_path(&self) -> PathBuf {
        self.dir.join("journal.jsonl")
    }

    /// The record, when the agent exists.
    ///
    /// # Errors
    /// When the record cannot be read or is not a v1 record.
    pub fn load(&self) -> Result<Option<Record>, String> {
        let Some(text) = super::sales::privacy::read_agent_text(self, &self.record_path())? else {
            return Ok(None);
        };
        let record: Record = serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} is not an agent record: {e}",
                self.record_path().display()
            )
        })?;
        if record.schema != RECORD_SCHEMA
            || record.v != 1
            || record
                .requires
                .iter()
                .any(|r| !matches!(r.as_str(), "crew-sales.v1" | "sales-model-budget.v1"))
        {
            return Err(format!(
                "{} is a record this host does not read",
                self.record_path().display()
            ));
        }
        record.validate_crew()?;
        if record.job_role.is_some() && !record.requires.iter().any(|r| r == "crew-sales.v1") {
            return Err("A sales record must declare its enforced crew-sales.v1 scope.".into());
        }
        Ok(Some(record))
    }

    /// Gives a record from before phase 4 its definition and roles, and
    /// journals that, once. Returns the record.
    ///
    /// # Errors
    /// When the record cannot be written.
    pub fn fill_identity(&self, mut record: Record, now: u64) -> Result<Record, String> {
        let had_definition = record.definition.is_some();
        if record.fill_identity() {
            self.save(&record)?;
            if !had_definition {
                self.append(&Entry::new(
                    now,
                    Kind::Migrated,
                    &format!(
                        "{} record gained a definition and roles",
                        record.refer().their()
                    ),
                ))?;
            }
        }
        Ok(record)
    }

    /// The record, made first when the agent does not exist yet, with its
    /// terminal opening in `workspace`.
    ///
    /// # Errors
    /// When the directory or the record cannot be written or read.
    pub fn open(&self, workspace: &Path, now: u64) -> Result<Record, String> {
        self.open_as(workspace, now, preset(&self.name))
    }

    /// [`Self::open`], with a new record made from `preset` when there is
    /// one: its charter, look, and definition, under this agent's name.
    /// An agent that exists already keeps its record.
    ///
    /// # Errors
    /// When the directory or the record cannot be written or read.
    pub fn open_as(
        &self,
        workspace: &Path,
        now: u64,
        preset: Option<&Preset>,
    ) -> Result<Record, String> {
        self.migrate(now)?;
        if let Some(record) = self.load()? {
            return self.fill_identity(record, now);
        }
        private_dir(&self.dir)?;
        let mut record = Record {
            schema: RECORD_SCHEMA.into(),
            v: 1,
            requires: if preset.and_then(|p| p.job_role).is_some() {
                vec!["crew-sales.v1".into()]
            } else {
                Vec::new()
            },
            name: self.name.clone(),
            charter: preset
                .map_or(super::agent_preset::CHARTER, |p| p.charter)
                .into(),
            workspace: workspace.display().to_string(),
            look: preset.map_or_else(|| self.name.clone(), |p| p.look.into()),
            created_at: now,
            pubkey: None,
            attestation: None,
            state: State::Active,
            route: String::new(),
            engine: String::new(),
            desk: default_desk(),
            definition: preset.map(|p| {
                let mut definition = p.definition();
                if p.name != self.name {
                    definition.display_name = capitalize(&self.name);
                }
                definition
            }),
            roles: None,
            job_role: preset.and_then(|p| p.job_role),
            sales_model_scope: None,
            crew_charter: preset
                .and_then(|p| p.job_role)
                .map(coder_host::access::crew::Charter::initial),
        };
        record.fill_identity();
        self.save(&record)?;
        self.append(&Entry::new(
            now,
            Kind::Created,
            &format!("{} was made, working in {}", record.name, record.workspace),
        ))?;
        Ok(record)
    }

    /// Writes `record` in place of the stored one, atomically.
    ///
    /// # Errors
    /// When the record cannot be written.
    pub fn save(&self, record: &Record) -> Result<(), String> {
        record.validate_crew()?;
        private_dir(&self.dir)?;
        let body = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
        let root = self
            .dir
            .parent()
            .and_then(Path::parent)
            .ok_or("agent host root is unavailable")?;
        super::sales::privacy::check_record_copy(
            root,
            record.job_role.is_some(),
            std::str::from_utf8(&body).map_err(|_| "agent definition is not UTF-8")?,
        )?;
        let temp = self.dir.join(".agent.json.tmp");
        write_private(&temp, &body)?;
        std::fs::rename(&temp, self.record_path())
            .map_err(|e| format!("cannot write {}: {e}", self.record_path().display()))
    }

    /// Moves the phase 1 agent's record to this one, once: when this is
    /// [`DEFAULT_NAME`], it has no record, and [`LEGACY_NAME`] has one. The
    /// journal moves with it and records the move; her look becomes
    /// Alice's. Returns whether it moved one.
    ///
    /// # Errors
    /// When the old directory cannot be moved or the record rewritten.
    pub fn migrate(&self, now: u64) -> Result<bool, String> {
        if self.name != DEFAULT_NAME || self.record_path().exists() {
            return Ok(false);
        }
        let Some(agents) = self.dir.parent() else {
            return Ok(false);
        };
        let old = agents.join(LEGACY_NAME);
        if !old.join("agent.json").is_file() {
            return Ok(false);
        }
        if self.dir.exists() {
            // An empty directory from an earlier attempt; anything else
            // stays as it is.
            std::fs::remove_dir(&self.dir).map_err(|e| {
                format!(
                    "cannot move {} over {}: {e}",
                    old.display(),
                    self.dir.display()
                )
            })?;
        }
        std::fs::rename(&old, &self.dir).map_err(|e| {
            format!(
                "cannot move {} to {}: {e}",
                old.display(),
                self.dir.display()
            )
        })?;
        let text = std::fs::read_to_string(self.record_path())
            .map_err(|e| format!("cannot read {}: {e}", self.record_path().display()))?;
        let mut record: Record = serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} is not an agent record: {e}",
                self.record_path().display()
            )
        })?;
        record.name = self.name.clone();
        if record.look == "workshop" {
            record.look = DEFAULT_LOOK.into();
        }
        if record
            .charter
            .starts_with("Terminal mode on this computer only.")
        {
            record.charter = DEFAULT_CHARTER.into();
        }
        self.save(&record)?;
        self.append(&Entry::new(
            now,
            Kind::Migrated,
            &format!(
                "{LEGACY_NAME} is now {}; {} record and journal moved here",
                self.name,
                record.refer().their()
            ),
        ))?;
        Ok(true)
    }

    /// The agent's own secret key, when it has one.
    ///
    /// # Errors
    /// When her key store can't be read or holds no key.
    pub fn key(&self) -> Result<Option<secp256k1::SecretKey>, String> {
        self.keys.load(self.slot())
    }

    /// Whether she may run: an agent whose record names a key runs only
    /// with that key in hand. The error says why not, for her journal.
    ///
    /// # Errors
    /// When her key store can't be read, holds no key although she had
    /// one, or holds another key than her record names.
    pub fn custody(&self, record: &Record) -> Result<(), String> {
        let Some(pubkey) = &record.pubkey else {
            return Ok(());
        };
        if record.state.is_gone() {
            return Ok(());
        }
        let p = record.refer();
        match self.key() {
            Err(why) => Err(format!(
                "{} key can't be read from the {}: {why}",
                p.their(),
                self.custody_kind()
            )),
            Ok(None) => Err(format!(
                "{} key {pubkey} is missing from the {}",
                p.their(),
                self.custody_kind()
            )),
            Ok(Some(key)) if public_hex(&key) != *pubkey => Err(format!(
                "the {} holds another key than {} record's {pubkey}",
                self.custody_kind(),
                p.their()
            )),
            Ok(Some(_)) => Ok(()),
        }
    }

    /// Makes the agent's own key when she has none and never had one,
    /// records its public half, and journals that. Returns the record. An
    /// agent whose record names a key never gets a new one here: a missing
    /// or different key is an error, so the owner restores it or rotates
    /// her on purpose. A retired agent may be keyed again.
    ///
    /// # Errors
    /// When her key can't be read, is missing or different although she
    /// had one, or the key or the record can't be written.
    pub fn ensure_key(&self, mut record: Record, now: u64) -> Result<Record, String> {
        let had = record.pubkey.clone();
        let retired = record.state == State::Retired;
        let p = record.refer();
        let key = match self.key()? {
            Some(key) => {
                let pubkey = public_hex(&key);
                match &had {
                    Some(had) if *had == pubkey => return Ok(record),
                    Some(had) if !retired => {
                        return Err(format!(
                            "the {} holds another key than {} record's {had}; the host won't \
                             replace it",
                            self.custody_kind(),
                            p.their()
                        ));
                    }
                    _ => key,
                }
            }
            None if had.is_some() && !retired => {
                return Err(format!(
                    "{} key is missing from the {}; the host won't make {} a new one",
                    p.their(),
                    self.custody_kind(),
                    p.them()
                ));
            }
            None => {
                let key = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
                self.keys.store(self.slot(), &key)?;
                key
            }
        };
        let pubkey = public_hex(&key);
        if record.pubkey.as_deref() != Some(pubkey.as_str()) {
            // A new key needs a new attestation.
            record.attestation = None;
        }
        record.pubkey = Some(pubkey.clone());
        self.save(&record)?;
        self.append(&Entry::new(
            now,
            Kind::Keyed,
            &format!(
                "{} key is {pubkey}, kept in the {}",
                p.their(),
                self.custody_kind()
            ),
        ))?;
        Ok(record)
    }

    /// Records the owner's attestation of the agent's key, signed with
    /// `owner`, valid until `expires_at`, at most [`ATTESTATION_MAX`] away.
    ///
    /// # Errors
    /// When the agent has no key, the expiry is out of range, or the
    /// record cannot be written.
    pub fn attest(
        &self,
        mut record: Record,
        owner: &secp256k1::SecretKey,
        expires_at: u64,
        now: u64,
    ) -> Result<Record, String> {
        let agent = record
            .pubkey
            .clone()
            .ok_or("the agent has no key to attest; make one first")?;
        if expires_at <= now || expires_at - now > ATTESTATION_MAX {
            return Err("an attestation expires within a year".into());
        }
        let attestation = sign_attestation(owner, &agent, &format!("created_at<{expires_at}"))?;
        verify_attestation(&agent, &attestation, now)?;
        record.attestation = Some(attestation.clone());
        record.fill_identity();
        self.save(&record)?;
        self.append(&Entry::new(
            now,
            Kind::Keyed,
            &format!(
                "the owner {} attested {} key until {expires_at}",
                attestation.owner,
                record.refer().their()
            ),
        ))?;
        // Her profile carries the new attestation. Without her key here,
        // the host signs it when it next opens her.
        let _ = super::agent_profile::refresh(self, &record, now);
        Ok(record)
    }

    /// Keeps `key` as her next key during a rotation, in a slot of its
    /// own (`agent:NAME.next`, or `agents/NAME/next/key`), and reads it
    /// back.
    ///
    /// # Errors
    /// When her key store can't be written or doesn't keep the key.
    pub(crate) fn store_next_key(&self, key: &secp256k1::SecretKey) -> Result<(), String> {
        let (name, dir) = self.next_slot();
        let slot = Slot {
            name: &name,
            dir: &dir,
        };
        self.keys.store(slot, key)?;
        match self.keys.load(slot)? {
            Some(back) if back == *key => Ok(()),
            _ => Err(format!(
                "the {} didn't keep {} next key",
                self.custody_kind(),
                self.refer().their()
            )),
        }
    }

    /// Deletes her next key and its directory, after a rotation or one
    /// that failed.
    pub(crate) fn delete_next_key(&self) {
        let (name, dir) = self.next_slot();
        let _ = self.keys.delete(Slot {
            name: &name,
            dir: &dir,
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn next_slot(&self) -> (String, PathBuf) {
        (format!("{}.next", self.name), self.dir.join("next"))
    }

    /// Keeps `key` as her key in place of the one there, and reads it
    /// back. The key it replaces is gone.
    ///
    /// # Errors
    /// When her key store can't be written or doesn't keep the key.
    pub(crate) fn replace_key(&self, key: &secp256k1::SecretKey) -> Result<(), String> {
        self.keys.store(self.slot(), key)?;
        match self.keys.load(self.slot())? {
            Some(back) if back == *key => Ok(()),
            _ => Err(format!(
                "the {} didn't keep {} new key",
                self.custody_kind(),
                self.refer().their()
            )),
        }
    }

    /// Deletes the agent's key, keeping its record and journal.
    ///
    /// # Errors
    /// When her key store can't be written.
    pub fn delete_key(&self) -> Result<bool, String> {
        self.keys.delete(self.slot())
    }

    /// Every agent under `host_root`, by name.
    #[must_use]
    pub fn all(host_root: &Path) -> Vec<Self> {
        let Ok(entries) = std::fs::read_dir(host_root.join("agents")) else {
            return Vec::new();
        };
        let mut stores: Vec<Self> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter_map(|name| Self::new(host_root, &name).ok())
            .filter(|store| store.record_path().is_file())
            .collect();
        stores.sort_by(|a, b| a.name.cmp(&b.name));
        stores
    }

    /// Appends `entry` to the journal. The journal is never rewritten.
    ///
    /// # Errors
    /// When the journal cannot be written.
    pub fn append(&self, entry: &Entry) -> Result<(), String> {
        super::sales::privacy::check_agent_copy(
            self,
            &serde_json::to_string(entry).map_err(|_| "agent journal serialization failed")?,
        )?;
        private_dir(&self.dir)?;
        let mut line = serde_json::to_vec(entry).map_err(|e| e.to_string())?;
        line.push(b'\n');
        super::sales::privacy::append_agent_directory_text(
            &self.dir,
            &self.journal_path(),
            std::str::from_utf8(&line).map_err(|_| "agent journal is not UTF-8")?,
        )
    }

    /// The newest `last` journal entries, oldest first. A line that does
    /// not read is skipped.
    ///
    /// # Errors
    /// When the journal exists and cannot be read.
    pub fn journal(&self, last: usize) -> Result<Vec<Entry>, String> {
        let Some(text) = super::sales::privacy::read_agent_text(self, &self.journal_path())? else {
            return Ok(Vec::new());
        };
        let entries: Vec<Entry> = text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        let skip = entries.len().saturating_sub(last);
        Ok(entries.into_iter().skip(skip).collect())
    }

    /// Every journal entry with its position, the 1-based line number a
    /// `journal:POS` reference names, oldest first. A line that does not
    /// read is skipped and keeps its position.
    ///
    /// # Errors
    /// When the journal exists and cannot be read.
    pub fn journal_rows(&self) -> Result<Vec<(usize, Entry)>, String> {
        let Some(text) = super::sales::privacy::read_agent_text(self, &self.journal_path())? else {
            return Ok(Vec::new());
        };
        Ok(text
            .lines()
            .enumerate()
            .filter_map(|(i, line)| serde_json::from_str(line).ok().map(|e| (i + 1, e)))
            .collect())
    }
}

pub(crate) fn private_dir(dir: &Path) -> Result<(), String> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(dir)
        .map_err(|e| format!("cannot create {}: {e}", dir.display()))
}

pub(crate) fn write_private(path: &Path, body: &[u8]) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    file.write_all(body)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

pub(crate) fn decode_hex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// The x-only public key of `key`, as 64 lowercase hex characters.
#[must_use]
pub fn public_hex(key: &secp256k1::SecretKey) -> String {
    let keypair = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::signing_only(), key);
    keypair.x_only_public_key().0.to_string()
}

/// A secret key read from `text`: 64 hex characters or an `nsec1` string.
///
/// # Errors
/// When `text` holds neither.
pub fn parse_secret(text: &str) -> Result<secp256k1::SecretKey, String> {
    let text = text.trim();
    let bytes = if text.starts_with("nsec1") {
        nostr::nip19::decode_nsec(text).map_err(|_| "not an nsec key".to_string())?
    } else {
        decode_hex32(text).ok_or("a key is 64 hex characters or an nsec1 string")?
    };
    secp256k1::SecretKey::from_byte_array(bytes).map_err(|_| "not a secret key".to_string())
}

/// The owner's NIP-OA attestation of `agent` under `conditions`, minted by
/// [`nostr::domain::mint_owner_attestation`].
///
/// # Errors
/// When `agent` is not a lowercase hex key, is the owner's own key, or
/// `conditions` breaks the NIP-OA grammar.
pub fn sign_attestation(
    owner: &secp256k1::SecretKey,
    agent: &str,
    conditions: &str,
) -> Result<Attestation, String> {
    let minted = nostr::domain::mint_owner_attestation(owner, agent, conditions)?;
    Ok(Attestation {
        owner: minted.owner_pubkey,
        conditions: minted.conditions,
        signature: minted.signature,
    })
}

/// Checks `attestation` of `agent` at `now`: the owner's signature, a
/// different owner, and every `created_at` clause. Returns the expiry.
///
/// # Errors
/// Says what does not hold.
pub fn verify_attestation(agent: &str, attestation: &Attestation, now: u64) -> Result<u64, String> {
    use sha2::Digest;
    if attestation.owner == agent {
        return Err("an agent cannot attest its own key".into());
    }
    let owner = decode_hex32(&attestation.owner)
        .and_then(|b| secp256k1::XOnlyPublicKey::from_byte_array(b).ok())
        .ok_or("the attestation's owner is not a key")?;
    let mut signature = [0u8; 64];
    let text = &attestation.signature;
    if text.len() != 128 {
        return Err("the attestation's signature is malformed".into());
    }
    for (i, byte) in signature.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
            .map_err(|_| "the attestation's signature is malformed")?;
    }
    let digest: [u8; 32] = sha2::Sha256::digest(
        format!("nostr:agent-auth:{agent}:{}", attestation.conditions).as_bytes(),
    )
    .into();
    secp256k1::Secp256k1::verification_only()
        .verify_schnorr(
            &secp256k1::schnorr::Signature::from_byte_array(signature),
            &digest,
            &owner,
        )
        .map_err(|_| "the attestation's signature does not verify".to_string())?;
    let mut expires = u64::MAX;
    for clause in attestation.conditions.split('&').filter(|c| !c.is_empty()) {
        if let Some(value) = clause.strip_prefix("created_at<") {
            let bound: u64 = value.parse().map_err(|_| "a condition is malformed")?;
            if now >= bound {
                return Err("the attestation expired".into());
            }
            expires = expires.min(bound);
        } else if let Some(value) = clause.strip_prefix("created_at>") {
            let bound: u64 = value.parse().map_err(|_| "a condition is malformed")?;
            if now <= bound {
                return Err("the attestation is not valid yet".into());
            }
        } else if !clause.starts_with("kind=") {
            return Err("the attestation has a condition this host does not read".into());
        }
    }
    Ok(expires)
}

/// `text` as printable ASCII on one line per line, with every word shaped
/// like a credential replaced by `[redacted]`.
#[must_use]
pub fn screen(text: &str) -> String {
    const PREFIXES: &[&str] = &[
        "sk-",
        "sk_",
        "pk_",
        "rk_",
        "ghp_",
        "gho_",
        "ghs_",
        "ghu_",
        "github_pat_",
        "xox",
        "AKIA",
        "AIza",
        "ya29.",
        "eyJ",
        "nsec1",
        "oak_",
        "sess_",
        "glpat-",
        "npm_",
    ];
    let ascii = ascii(&secret_screen::redact(text));
    let mut out = String::with_capacity(ascii.len());
    for (i, line) in ascii.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let words: Vec<String> = line
            .split(' ')
            .map(|word| {
                let bare = word.trim_matches(|c: char| "\"'`()[]{}<>,;=:".contains(c));
                let after_equals = word.rsplit(['=', ':']).next().unwrap_or(word);
                let keyed = PREFIXES
                    .iter()
                    .any(|p| bare.starts_with(p) || after_equals.trim_matches('"').starts_with(p));
                let long_secret = bare.len() >= 32
                    && bare
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "+/_-".contains(c))
                    && bare.chars().any(|c| c.is_ascii_digit())
                    && bare.chars().any(|c| c.is_ascii_alphabetic())
                    && !bare.contains('/');
                if (keyed && bare.len() >= 8) || long_secret || word.contains("-----BEGIN") {
                    "[redacted]".to_string()
                } else {
                    word.to_string()
                }
            })
            .collect();
        out.push_str(&words.join(" "));
    }
    out
}

/// `text` in printable ASCII: tabs become spaces, other characters outside
/// ASCII become `?`, and control characters other than a newline go.
#[must_use]
pub fn ascii(text: &str) -> String {
    text.chars()
        .filter_map(|c| match c {
            '\n' => Some('\n'),
            '\t' => Some(' '),
            '\u{2018}' | '\u{2019}' => Some('\''),
            '\u{201c}' | '\u{201d}' => Some('"'),
            '\u{2013}' | '\u{2014}' => Some('-'),
            '\u{2026}' => Some('.'),
            c if c.is_ascii_control() => None,
            c if c.is_ascii() => Some(c),
            _ => Some('?'),
        })
        .collect()
}

/// `text` cut to at most `max` bytes at a character boundary.
fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// What in `text`, a line the agent says to the owner, would tell them to
/// do a machine step: a key to press, a program to quit or run, a flag, a
/// path, or a session to follow. The owner asks; she handles the rest.
#[must_use]
pub fn instructs(text: &str) -> Option<String> {
    const STEPS: [&str; 16] = [
        "`",
        "Ctrl",
        "CTRL",
        "--",
        "~/",
        "press ",
        "Press ",
        "quit ",
        "Quit ",
        "ESC",
        "ENTER",
        "follow along",
        "openagents ",
        "agent log",
        "scripts/",
        "ask again",
    ];
    if let Some(step) = STEPS.iter().find(|step| text.contains(**step)) {
        return Some((*step).to_string());
    }
    text.split_whitespace()
        .find(|word| word.starts_with('/') || word.starts_with("./") || word.ends_with(".sh"))
        .map(str::to_string)
}

/// A report the way the agent's panel shows it: plain ASCII without
/// Markdown markers, on at most a few lines, at most [`REPLY_MAX`]
/// characters.
#[must_use]
pub fn plain(text: &str) -> String {
    let ascii = screen(text);
    let lines: Vec<String> = ascii
        .lines()
        .map(|line| {
            let line = line.trim();
            let line = line.trim_start_matches('#').trim_start();
            let line = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .unwrap_or(line);
            line.replace("**", "").replace('`', "")
        })
        .filter(|line| !line.is_empty() && !line.starts_with("```"))
        .collect();
    let joined = lines.join(" ");
    if joined.chars().count() <= REPLY_MAX {
        return joined;
    }
    let cut: String = joined.chars().take(REPLY_MAX - 3).collect();
    format!("{}...", cut.trim_end())
}

/// What running a command may do, decided before anything types it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// It only reads, or builds and tests into the build directory: it
    /// runs without asking.
    ReadOnly,
    /// It may change files, the repository, or this computer: it waits for
    /// the owner's CONFIRM or REJECT. The text says why.
    Approval(String),
    /// The deny list refuses it outright.
    Denied(String),
}

/// The effect class of `command`, from the deny list and a closed list of
/// read-only programs. Pipes and `&&`, `||`, and `;` chains are read-only
/// when every part is; redirection, substitution, and background jobs are
/// not.
#[must_use]
pub fn effect(command: &str) -> Effect {
    if let Some(why) = crate::shell::denied(command) {
        return Effect::Denied(why.to_string());
    }
    let commands = match simple_commands(command) {
        Ok(commands) => commands,
        Err(why) => return Effect::Approval(why),
    };
    if commands.is_empty() {
        return Effect::Approval("the command is empty".into());
    }
    for words in &commands {
        if let Some(why) = part_effect(words) {
            return Effect::Approval(why);
        }
    }
    Effect::ReadOnly
}

/// `command` as its simple commands, split at unquoted `|`, `||`, `&&`, and
/// `;`, each a list of unquoted words. An unquoted redirection other than
/// `2>&1`, a background `&`, a newline, and a command substitution outside
/// single quotes are refused, with why.
fn simple_commands(command: &str) -> Result<Vec<Vec<String>>, String> {
    let writes =
        || "it redirects, substitutes, or runs in the background, which can write".to_string();
    let end_word = |word: &mut String, quoted: &mut bool, words: &mut Vec<String>| {
        if !word.is_empty() || *quoted {
            words.push(std::mem::take(word));
        }
        *quoted = false;
    };
    let chars: Vec<char> = command.trim().chars().collect();
    let mut commands: Vec<Vec<String>> = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' => {
                quoted = true;
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    word.push(chars[i]);
                    i += 1;
                }
                if i == chars.len() {
                    return Err("a quote is not closed".into());
                }
            }
            '"' => {
                quoted = true;
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    match chars[i] {
                        '`' => return Err(writes()),
                        '$' if chars.get(i + 1) == Some(&'(') => return Err(writes()),
                        '\\' if i + 1 < chars.len() => {
                            i += 1;
                            word.push(chars[i]);
                        }
                        other => word.push(other),
                    }
                    i += 1;
                }
                if i == chars.len() {
                    return Err("a quote is not closed".into());
                }
            }
            '\\' if i + 1 < chars.len() => {
                i += 1;
                word.push(chars[i]);
            }
            ' ' | '\t' => end_word(&mut word, &mut quoted, &mut words),
            '2' if word.is_empty()
                && !quoted
                && chars[i + 1..].starts_with(&['>', '&', '1'])
                && chars.get(i + 4).is_none_or(|c| c.is_whitespace()) =>
            {
                i += 3;
            }
            '|' | ';' | '&' => {
                let double = chars.get(i + 1) == Some(&c);
                if c == '&' && !double {
                    return Err(writes());
                }
                if double {
                    i += 1;
                }
                end_word(&mut word, &mut quoted, &mut words);
                if !words.is_empty() {
                    commands.push(std::mem::take(&mut words));
                } else if c == '|' && !double {
                    return Err("a pipe has nothing before it".into());
                }
            }
            '>' | '<' | '`' | '\n' | '\r' => return Err(writes()),
            '$' if chars.get(i + 1) == Some(&'(') => return Err(writes()),
            other => word.push(other),
        }
        i += 1;
    }
    end_word(&mut word, &mut quoted, &mut words);
    if !words.is_empty() {
        commands.push(words);
    }
    Ok(commands)
}

/// Why one simple command is not read-only, or `None` when it is.
fn part_effect(words: &[String]) -> Option<String> {
    let mut words = words.iter().map(String::as_str).peekable();
    // Leading `NAME=value` assignments, without expansion.
    while let Some(word) = words.peek() {
        let assignment = word.split_once('=').is_some_and(|(name, value)| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                && !value.contains('$')
        });
        if !assignment {
            break;
        }
        words.next();
    }
    let words: Vec<&str> = words.collect();
    let Some(first) = words.first() else {
        return Some("a part of the command is empty".into());
    };
    let program = first.rsplit('/').next().unwrap_or(first);
    let args = &words[1..];
    let has = |flag: &str| {
        args.iter()
            .any(|a| *a == flag || a.starts_with(&format!("{flag}=")))
    };
    #[rustfmt::skip]
    const PLAIN: &[&str] = &[
        "ls", "pwd", "cat", "head", "tail", "wc", "grep", "rg", "echo", "date", "uname", "df",
        "du", "which", "whoami", "uptime", "file", "stat", "tree", "sort", "uniq", "cut", "tr",
        "jq", "cd", "true", "basename", "dirname", "realpath", "diff", "cmp", "nl", "ps",
        "sw_vers", "test", "printf", "column", "less", "id", "arch", "nproc", "readlink",
        "shasum", "sha256sum", "md5", "od", "type",
    ];
    let refuse = |why: String| Some(why);
    match program {
        "less" => refuse("`less` waits for keys".into()),
        // Help and version of a program on the path only print text.
        _ if !first.contains('/') && only_help(args) => None,
        "hostname" if args.is_empty() => None,
        "openagents" => openagents_effect(args),
        _ if PLAIN.contains(&program) => None,
        "find" => {
            let writes = [
                "-delete", "-exec", "-execdir", "-ok", "-okdir", "-fprint", "-fls",
            ];
            match args.iter().find(|a| writes.contains(a)) {
                Some(flag) => refuse(format!("`find {flag}` can change files")),
                None => None,
            }
        }
        "sed" => {
            if args
                .iter()
                .any(|a| a.starts_with("-i") || *a == "--in-place")
            {
                refuse("`sed -i` edits files".into())
            } else if has("-n") {
                None
            } else {
                refuse("`sed` is read-only here only with `-n`".into())
            }
        }
        "git" => git_effect(args),
        "cargo" => cargo_effect(args),
        _ => refuse(format!("`{program}` is not on the read-only list")),
    }
}

/// Whether `args` only ask a program for its help or version: `--help`,
/// `-h`, `--version`, or `-V`, alone.
fn only_help(args: &[&str]) -> bool {
    matches!(args, ["--help" | "-h" | "--version" | "-V"])
}

/// Subcommands of `openagents` that only read, under any command group.
const OPENAGENTS_READS: &[&str] = &["status", "list", "ls", "show", "view", "doctor"];

/// Command groups whose `show` or `list` can reveal secrets.
const OPENAGENTS_SECRETS: &[&str] = &["key", "keys", "wallet", "pay", "x402", "sov", "provider"];

/// Why an `openagents` command is not read-only, or `None` when it is: its
/// help or version anywhere, `doctor`, and a group's `status`, `list`,
/// `ls`, `show`, or `doctor`, outside the groups that hold secrets.
fn openagents_effect(args: &[&str]) -> Option<String> {
    let words: Vec<&str> = args
        .iter()
        .copied()
        .filter(|word| *word != "--json")
        .collect();
    if words
        .last()
        .is_some_and(|word| matches!(*word, "--help" | "-h"))
    {
        return None;
    }
    match words.as_slice() {
        [] => None,
        ["help" | "--help" | "-h", ..] => None,
        ["version" | "--version" | "doctor" | "status"] => None,
        [group, verb, ..]
            if !group.starts_with('-')
                && OPENAGENTS_READS.contains(verb)
                && !OPENAGENTS_SECRETS.contains(group) =>
        {
            None
        }
        [group, ..] => Some(format!("`openagents {group}` can change state")),
    }
}

fn git_effect(args: &[&str]) -> Option<String> {
    let mut rest = args.iter().copied();
    let mut sub = None;
    while let Some(word) = rest.next() {
        match word {
            "-C" | "-c" => {
                if word == "-c" {
                    return Some("`git -c` changes git's settings for the command".into());
                }
                rest.next();
            }
            "--no-pager" | "-P" => {}
            w if w.starts_with('-') => {}
            w => {
                sub = Some(w);
                break;
            }
        }
    }
    let after: Vec<&str> = rest.collect();
    let only_flags = |allowed: &[&str]| after.iter().all(|a| allowed.contains(a));
    match sub {
        None => None,
        Some(
            "status" | "log" | "diff" | "show" | "rev-parse" | "ls-files" | "blame" | "describe"
            | "shortlog" | "grep" | "rev-list" | "cat-file" | "ls-tree" | "reflog",
        ) => None,
        Some("branch") if only_flags(&["-a", "-r", "-v", "-vv", "--list", "--show-current"]) => {
            None
        }
        Some("remote") if only_flags(&["-v"]) => None,
        Some("tag") if only_flags(&["-l", "--list"]) => None,
        Some("stash") if after.first().is_some_and(|w| *w == "list") => None,
        Some(other) => Some(format!("`git {other}` can change the repository")),
    }
}

fn cargo_effect(args: &[&str]) -> Option<String> {
    if args.iter().any(|a| *a == "--fix" || *a == "--allow-dirty") {
        return Some("`--fix` edits source files".into());
    }
    let sub = args
        .iter()
        .find(|a| !a.starts_with('-') && !a.starts_with('+'));
    match sub.copied() {
        None if args.iter().any(|a| *a == "--version" || *a == "-V") => None,
        Some(
            "test" | "check" | "build" | "clippy" | "tree" | "metadata" | "nextest" | "doc"
            | "bench" | "locate-project" | "pkgid" | "verify-project",
        ) => None,
        Some("fmt") if args.contains(&"--check") => None,
        Some(other) => Some(format!("`cargo {other}` can change files or this computer")),
        None => Some("a bare `cargo` command".into()),
    }
}

/// What the agent is doing, for its nameplate and its walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doing {
    Idle,
    Thinking,
    Running,
    Testing,
    /// Waiting on the owner's CONFIRM or REJECT.
    Waiting,
    Done,
    Failed,
}

/// The owner's answer to a proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Confirm,
    Reject,
}

/// One structured model call, the Microcoder loop's step.
pub trait Model {
    /// The next action for `prompt` under `system`.
    ///
    /// # Errors
    /// When no model answered, with why.
    fn next(&mut self, system: &str, prompt: &str) -> Result<NextAction, String>;
}

/// How a request ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The agent answered.
    Done,
    /// A command failed, or no model answered.
    Failed,
    /// The owner took the terminal back or rejected the work.
    Stopped,
}

/// What the agent reports when a request ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub outcome: Outcome,
    /// The answer for its panel: plain ASCII, at most [`REPLY_MAX`].
    pub reply: String,
    /// A short status for its nameplate, from what ran, never from the
    /// model's words.
    pub headline: String,
}

/// Whether `command` runs tests, for the nameplate's word.
#[must_use]
pub fn testing(command: &str) -> bool {
    let words: Vec<&str> = command.split_whitespace().collect();
    words.windows(2).any(|w| {
        matches!(w[0], "cargo" | "npm" | "pnpm" | "go") && matches!(w[1], "test" | "nextest")
    }) || words.iter().any(|w| *w == "pytest")
}

/// The model the agent plans with: Microcoder's one structured call on the
/// first connected provider with capacity in the capacity book (the Codex
/// login, then Claude Code, then the OpenAgents cloud), with failover.
pub struct LiveModel {
    runtime: tokio::runtime::Runtime,
    book: PathBuf,
    /// The model that answered last.
    pub model: Option<String>,
    /// What the last call cost, in dollars, or `None` when no cost was
    /// reported.
    pub usd: Option<f64>,
    /// The last call's prompt and completion tokens, each `None` when the
    /// provider reported none.
    pub tokens: crate::delegate_door::microcoder::Tokens,
}

impl LiveModel {
    /// The live model, reading the capacity book in `~/.openagents/tasks`.
    ///
    /// # Errors
    /// When no async runtime starts.
    pub fn new() -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let book = crate::delegate_door::capacity_dir().unwrap_or_else(std::env::temp_dir);
        Ok(Self {
            runtime,
            book,
            model: None,
            usd: None,
            tokens: (None, None),
        })
    }

    /// Which providers this computer has and where each stands, in words.
    #[must_use]
    pub fn standing(&self) -> String {
        let providers = self.providers();
        providers.iter().find(|state| state.usable()).map_or_else(
            || crate::delegate_door::microcoder::none_left(&providers),
            |state| format!("{:?} {}", state.provider, state.model).to_lowercase(),
        )
    }

    /// Whether a provider has capacity now.
    #[must_use]
    pub fn usable(&self) -> bool {
        self.providers().iter().any(|state| state.usable())
    }

    fn providers(&self) -> Vec<crate::delegate_door::microcoder::ProviderState> {
        use crate::delegate_door::microcoder;
        microcoder::providers(
            &microcoder::lineup(None),
            &self.book,
            &crate::task::capacity::probe,
            microcoder::now(),
        )
    }
}

impl Model for LiveModel {
    fn next(&mut self, system: &str, prompt: &str) -> Result<NextAction, String> {
        use crate::delegate_door::microcoder;
        let providers = self.providers();
        let (action, model, usd, tokens) = self.runtime.block_on(microcoder::next_action(
            system,
            prompt,
            &providers,
            &self.book,
            microcoder::now,
        ))?;
        self.model = Some(model);
        self.usd = usd;
        self.tokens = tokens;
        Ok(action)
    }
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
