//! Dedicated environment setup sessions (ENV-03).
//!
//! A [`SetupSession`] is where an agent drafts and revises one
//! environment's recipe with user steering. It runs on its own computer
//! ([`coder_working_computer::Purpose::EnvironmentSetup`]), never on a
//! chat's working computer, through the same provider abstraction
//! ([`coder_working_computer::provider::Provider`] plus
//! [`coder_working_computer::provider::Commands`]) and CMP-01 driver.
//!
//! - **Admission** ([`admit`]) binds an admitted Coder/Codex operator
//!   profile (`coder_cloud::operator::Profile::qualified_coder_identity`)
//!   to the environment's project and exact source, pins the draft's base
//!   image, runtime, and platform, and admits only credentials the session
//!   names explicitly. A recipe revision that would change a pin is refused;
//!   a new pin needs a new session.
//! - **Recipe tools** ([`service::Setup::update_recipe`]) go through
//!   `coder_environment::transition::apply` with its draft revision fence.
//! - **Commands** carry a stable identity retained before the provider is
//!   called. A provider runs one identity at most once, so a lost start
//!   reply is reconciled by reading that identity, never by running the
//!   command again ([`transition::Run::Unknown`]).
//! - **Install → repair → rerun** is a sequence of identified install
//!   commands, each bound to the exact recipe revision and digest it ran.
//! - **Requests are idempotent**: a repeated request ID with the same
//!   arguments returns the retained result; different arguments conflict.
//! - **Deadline and cancellation**: each command has a deadline inside the
//!   session's admitted deadline; passing either stops the work and records
//!   it as timed out or cancelled, with machine cleanup as separate facts on
//!   the computer record.
//! - **Evidence**: every tool call, argument, and output byte goes through
//!   the ENV-02a [`coder_environment::evidence::Recorder`], redacted of the
//!   selected credential values before anything is persisted.
//! - **Source** ([`source`]): before any install, the session puts the
//!   exact pinned commit on its computer and proves `HEAD` and a clean
//!   tree; the builder runs the same step.
//! - **Ephemeral Git auth** ([`git_auth_env`]): Git reads a credential
//!   helper from per-process `GIT_CONFIG_*` variables that name the
//!   credential variable, so no token reaches `.git/config` or any file.

use coder_cloud::operator::{Adapter, Profile};
use coder_environment::{
    ArtifactPin, Environment, ImagePin, Platform, SourcePin, digest, valid_id,
};
use coder_working_computer::{Principal, credential_name_allowed};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub mod panel;
pub mod service;
pub mod source;
pub mod store;
pub mod transition;

pub use transition::{
    CommandPurpose, CommandRecord, End, Op, Refusal, Run, SetupSession, SetupState, StopReason,
    ToolResult, apply,
};

pub const SCHEMA: &str = "openagents.environment.setup.v1";
/// The engine admitted for setup sessions in this slice.
pub const SETUP_ENGINE: &str = "codex";
/// Credentials the session may use for ephemeral Git auth.
pub const GIT_CREDENTIALS: &[&str] = &["GH_TOKEN", "GITHUB_TOKEN"];
pub const MAX_COMMANDS: usize = 512;
pub const MAX_STEERING: usize = 256;
pub const MAX_REQUESTS: usize = 1024;
pub const MAX_SEGMENTS: usize = 64;
pub const MAX_COMMAND_BYTES: usize = 64 * 1024;
pub const MAX_TEXT_BYTES: usize = 4096;
pub const MAX_DEADLINE_SECONDS: u64 = 24 * 3600;

pub(crate) fn valid_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.contains('\0')
}
pub(crate) fn valid_relative(value: &str) -> bool {
    value == "."
        || (valid_text(value, 1024)
            && !value.starts_with('/')
            && value
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != ".."))
}

/// What a user asks for when they start a setup session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupRequest {
    pub session: String,
    pub environment: String,
    pub owner: Principal,
    /// The operator profile alias the session runs under.
    pub profile: String,
    pub objective: String,
    /// Credentials the setup may use, by name. Nothing else is applied.
    #[serde(default)]
    pub credential_names: BTreeSet<String>,
    /// One of the named credentials, used only for ephemeral Git auth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_credential: Option<String>,
    pub deadline_seconds: u64,
}

/// Everything the session is pinned to, retained before any allocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub profile: String,
    /// Digest of the admitted profile as it was when the session started.
    pub profile_digest: String,
    pub engine: String,
    pub size: String,
    pub source: SourcePin,
    pub base: ImagePin,
    pub runtime: ArtifactPin,
    pub platform: Platform,
    pub credential_names: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_credential: Option<String>,
    /// Absolute deadline for the whole session.
    pub deadline_ms: u64,
    /// Retained evidence bytes (the recipe's admitted output bound).
    pub evidence_budget: u64,
}

/// Admit a setup session: an admitted Coder/Codex Boat profile bound to
/// this environment's project and exact source, the draft's pinned base,
/// runtime, and platform, and only explicitly named credentials.
pub fn admit(
    request: &SetupRequest,
    profile: &Profile,
    env: &Environment,
    now_ms: u64,
) -> Result<Admission, Refusal> {
    let refuse = |m| Err(Refusal::Admission(m));
    if !valid_id(&request.session) || request.session.len() > 64 || !valid_id(&request.profile) {
        return refuse("The session and profile need opaque identities.");
    }
    if request.environment != env.id {
        return refuse("The request names a different environment.");
    }
    if env.retired_ms.is_some() {
        return refuse("The environment is retired.");
    }
    if !valid_id(&request.owner.workspace) || !valid_id(&request.owner.principal) {
        return refuse("The owner needs a workspace and principal.");
    }
    if request.owner.workspace != env.project.workspace {
        return refuse("The owner is not in the environment's workspace.");
    }
    if !valid_text(&request.objective, MAX_TEXT_BYTES) {
        return refuse("The objective needs 1 to 4096 bytes.");
    }
    if !profile.qualified_coder_identity() || profile.executor != SETUP_ENGINE {
        return refuse("Setup runs only on an admitted Coder/Codex profile.");
    }
    if profile.placement != coder_cloud::Placement::Boat
        || !matches!(profile.adapter, Adapter::Boat { .. })
    {
        return refuse("Setup needs a dedicated Boat machine; shared pools are not admitted.");
    }
    if profile.workspace != env.project.workspace || profile.project != env.project.project {
        return refuse("The profile is bound to a different project.");
    }
    if profile.source_revision != env.source.revision || profile.source_digest != env.source.digest
    {
        return refuse("The profile's admitted source differs from the environment's pin.");
    }
    for name in &request.credential_names {
        if !credential_name_allowed(name) {
            return Err(Refusal::CredentialNotAdmitted(name.clone()));
        }
        if !profile.credentials.contains_key(name) {
            return Err(Refusal::CredentialNotAdmitted(name.clone()));
        }
    }
    if request.credential_names.len() > 32 {
        return refuse("Too many named credentials.");
    }
    if let Some(git) = &request.git_credential
        && (!GIT_CREDENTIALS.contains(&git.as_str()) || !request.credential_names.contains(git))
    {
        return refuse("Git auth must use a named GitHub token credential.");
    }
    let draft = &env.draft().recipe;
    if request.deadline_seconds == 0
        || request.deadline_seconds > draft.limits.deadline_seconds
        || request.deadline_seconds > profile.max_timeout_seconds
        || request.deadline_seconds > MAX_DEADLINE_SECONDS
    {
        return refuse("The deadline exceeds the recipe or profile bound.");
    }
    Ok(Admission {
        profile: request.profile.clone(),
        profile_digest: digest(&serde_json::to_vec(profile).expect("profile encodes")),
        engine: profile.executor.clone(),
        size: profile.size.clone(),
        source: env.source.clone(),
        base: draft.base.clone(),
        runtime: draft.runtime.clone(),
        platform: draft.platform.clone(),
        credential_names: request.credential_names.clone(),
        git_credential: request.git_credential.clone(),
        deadline_ms: now_ms + request.deadline_seconds * 1000,
        evidence_budget: draft.limits.output_bytes,
    })
}

impl Admission {
    /// Whether a recipe keeps this session's pins.
    pub fn pins(&self, recipe: &coder_environment::Recipe) -> bool {
        recipe.base == self.base
            && recipe.runtime == self.runtime
            && recipe.platform == self.platform
    }
}

/// Per-process Git configuration for ephemeral auth. The helper answers
/// `get` with the value of the credential *variable* at run time; neither
/// this map nor any file Git writes holds the token. The empty first helper
/// clears any helper a repository or user config would add. It answers only
/// for `https://github.com`: setup commands also fetch Git dependencies and
/// submodules from other hosts, and those must never be handed the token.
pub fn git_auth_env(credential: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("GIT_CONFIG_COUNT".into(), "2".into()),
        ("GIT_CONFIG_KEY_0".into(), "credential.helper".into()),
        ("GIT_CONFIG_VALUE_0".into(), String::new()),
        ("GIT_CONFIG_KEY_1".into(), "credential.helper".into()),
        (
            "GIT_CONFIG_VALUE_1".into(),
            format!(
                "!f() {{ test \"$1\" = get || exit 0; p=; h=; while IFS='=' read -r k v; do case \"$k\" in protocol) p=$v;; host) h=$v;; esac; done; test \"$p\" = https && test \"$h\" = github.com || exit 0; echo username=x-access-token; echo \"password=${{{credential}}}\"; }}; f"
            ),
        ),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
    ])
}

/// Whether `text` carries credentials inside a URL (`scheme://user@host`).
/// Setup commands authenticate through [`git_auth_env`] instead.
pub fn embeds_url_credential(text: &str) -> bool {
    text.match_indices("://").any(|(at, _)| {
        let authority = text[at + 3..]
            .split(|c: char| c == '/' || c.is_whitespace() || matches!(c, '"' | '\'' | '`'))
            .next()
            .unwrap_or_default();
        authority.contains('@')
    })
}

#[cfg(test)]
mod tests;
