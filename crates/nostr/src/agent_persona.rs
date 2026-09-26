//! NIP-AP persona definitions and catalog transport projection.
//!
//! A parsed command is configuration, never authority to launch a process.
//! Foreign adoption accepts portable aliases only; local discovery and host
//! policy still decide whether an alias is available and may execute.

use serde::{Deserialize, Serialize};

use crate::domain::Event;
use crate::read_state_snapshot::hex_bytes;

pub const PERSONA_KIND: u16 = 30_175;
pub const MAX_CONTENT_BYTES: usize = 65_535;
pub const STOCK_COMMAND: &str = "buzz-acp";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionPolicy {
    #[default]
    Channel,
    Thread,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RespondTo {
    Anyone,
    OwnerOnly,
    Allowlist,
}

/// Known definition fields. Readers ignore unknown fields for compatibility.
///
/// The upstream optional-field table still labels response policy and
/// parallelism reserved, while its lifecycle text specifies creation-time
/// copying. This type retains them without implementing an agent launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Persona {
    pub display_name: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_command: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub name_pool: Vec<String>,
    #[serde(default)]
    pub respond_to: Option<RespondTo>,
    #[serde(default)]
    pub respond_to_allowlist: Vec<String>,
    #[serde(default)]
    pub parallelism: Option<i64>,
    #[serde(default)]
    pub session_policy: SessionPolicy,
}

impl Persona {
    /// Parse the client-owned content contract; relay storage need not parse it.
    pub fn parse(content: &str) -> Result<Self, &'static str> {
        if content.len() > MAX_CONTENT_BYTES {
            return Err("content_size");
        }
        let value = crate::contracts::parse_strict(content.as_bytes()).map_err(|_| "content")?;
        if value.get("env_vars").is_some() {
            return Err("secrets");
        }
        let persona: Self = serde_json::from_value(value).map_err(|_| "content")?;
        persona.validate()?;
        Ok(persona)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        for key in &self.respond_to_allowlist {
            hex_bytes::<32>(key).map_err(|_| "allowlist")?;
        }
        if serde_json::to_vec(self).map_err(|_| "content")?.len() > MAX_CONTENT_BYTES {
            return Err("content_size");
        }
        Ok(())
    }

    /// Effective transport for a new definition, before local resolution.
    #[must_use]
    pub fn effective_command(&self) -> &str {
        self.acp_command.as_deref().unwrap_or(STOCK_COMMAND)
    }

    /// Produce catalog content without publishing a machine-local command.
    ///
    /// This does not conceal instructions or the persona's allowlist. Publishing
    /// the shared tag makes the resulting plaintext readable to the community.
    pub fn catalog_content(&self) -> Result<String, &'static str> {
        self.validate()?;
        let mut projected = self.clone();
        projected.acp_command =
            is_portable_command(self.effective_command()).then(|| self.effective_command().into());
        let body = serde_json::to_string(&projected).map_err(|_| "content")?;
        if body.len() > MAX_CONTENT_BYTES {
            return Err("content_size");
        }
        Ok(body)
    }

    /// Local drift identity over the unredacted known definition fields.
    ///
    /// The shared tag and redacted catalog body never enter this digest. A host
    /// can retain this identity independently of an event ID or catalog hash.
    pub fn source_version(&self) -> Result<String, &'static str> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|_| "content")?;
        crate::contracts::digest_value(&value).map_err(|_| "content")
    }
}

/// Stock transport or a bounded portable alias, not a shell command or path.
#[must_use]
pub fn is_portable_command(command: &str) -> bool {
    if command == STOCK_COMMAND {
        return true;
    }
    if command.len() > 255 || !command.is_ascii() {
        return false;
    }
    command
        .strip_prefix("buzz-")
        .and_then(|value| value.strip_suffix("-acp"))
        .is_some_and(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
}

/// Authenticate and load a persona event under the appropriate adoption path.
///
/// A foreign head must be explicitly shared and contain no nonportable command.
/// Replaying an owner's shared, redacted head preserves an existing local custom
/// command. Non-catalog owner sync retains custom-command compatibility.
pub fn adopt(
    event: &Event,
    local_owner: &str,
    existing: Option<&Persona>,
) -> Result<Persona, &'static str> {
    if event.kind != PERSONA_KIND {
        return Err("kind");
    }
    event.validate_nip01_structure().map_err(|_| "event")?;
    event.validate_crypto().map_err(|_| "signature")?;
    hex_bytes::<32>(local_owner).map_err(|_| "owner")?;
    let d = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("d"))
        .collect::<Vec<_>>();
    let [d] = d.as_slice() else {
        return Err("slug");
    };
    if !valid_slug(d.value().ok_or("slug")?) {
        return Err("slug");
    }
    let shared = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("shared"))
        .collect::<Vec<_>>();
    let is_shared = match shared.as_slice() {
        [] => false,
        [tag] if tag.as_slice() == ["shared", "true"] => true,
        _ => return Err("shared"),
    };
    let mut persona = Persona::parse(&event.content)?;
    let foreign = event.pubkey != local_owner;
    if foreign && !is_shared {
        return Err("private");
    }
    if is_shared
        && persona
            .acp_command
            .as_deref()
            .is_some_and(|command| !is_portable_command(command))
    {
        return Err("transport");
    }
    if !foreign
        && is_shared
        && persona.acp_command.is_none()
        && let Some(previous) = existing
        && !is_portable_command(previous.effective_command())
    {
        persona.acp_command = Some(previous.effective_command().into());
    }
    Ok(persona)
}

/// A changed conversation boundary applies at restart, never in place.
#[must_use]
pub fn session_restart_required(previous: &Persona, current: &Persona) -> bool {
    previous.session_policy != current.session_policy
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn signer(n: u8) -> RelaySigner {
        RelaySigner::from_secret_hex(&format!("{n:064x}")).unwrap()
    }
    fn event(body: &str, shared: bool) -> Event {
        let mut tags = vec![Tag::new(vec!["d".into(), "test".into()])];
        if shared {
            tags.push(Tag::new(vec!["shared".into(), "true".into()]));
        }
        signer(1).sign(1, PERSONA_KIND, tags, body.into())
    }

    #[test]
    fn retains_defaults_and_ignores_extensions_but_rejects_secret_field() {
        let persona = Persona::parse(r#"{"display_name":"Minimal","future":true}"#).unwrap();
        assert_eq!(persona.effective_command(), STOCK_COMMAND);
        assert_eq!(persona.session_policy, SessionPolicy::Channel);
        assert!(persona.system_prompt.is_none());
        assert!(Persona::parse(r#"{"display_name":"Bad","env_vars":{}}"#).is_err());
        assert!(Persona::parse(r#"{"display_name":"Bad","session_policy":null}"#).is_err());
        assert!(
            Persona::parse(r#"{"display_name":"Bad","respond_to_allowlist":["not-a-key"]}"#)
                .is_err()
        );
    }

    #[test]
    fn portable_alias_grammar_rejects_commands_and_paths() {
        for command in ["buzz-acp", "buzz-goose-acp", "buzz-a_B-2-acp"] {
            assert!(is_portable_command(command));
        }
        for command in [
            "/bin/buzz-acp",
            "buzz-acp --unsafe",
            "buzz--acp",
            "buzz-é-acp",
            "buzz-foo-acp;env",
        ] {
            assert!(!is_portable_command(command));
        }
        assert!(!is_portable_command(&format!(
            "buzz-{}-acp",
            "a".repeat(247)
        )));
    }

    #[test]
    fn redaction_preserves_local_transport_but_foreign_adoption_uses_stock() {
        let local =
            Persona::parse(r#"{"display_name":"Local","acp_command":"/private/agent"}"#).unwrap();
        let version = local.source_version().unwrap();
        let content = local.catalog_content().unwrap();
        assert!(!content.contains("acp_command"));
        assert!(!content.contains("/private/agent"));
        let shared = event(&content, true);
        assert_eq!(
            adopt(&shared, signer(1).pubkey(), Some(&local))
                .unwrap()
                .effective_command(),
            "/private/agent"
        );
        assert_eq!(
            adopt(&shared, signer(2).pubkey(), Some(&local))
                .unwrap()
                .effective_command(),
            STOCK_COMMAND
        );
        assert_eq!(local.source_version().unwrap(), version);
        let stock = event(r#"{"display_name":"Local","acp_command":"buzz-acp"}"#, true);
        assert_eq!(
            adopt(&stock, signer(1).pubkey(), Some(&local))
                .unwrap()
                .effective_command(),
            STOCK_COMMAND
        );
        assert!(
            adopt(
                &event(
                    r#"{"display_name":"Local","acp_command":"/private/agent"}"#,
                    true
                ),
                signer(2).pubkey(),
                None
            )
            .is_err()
        );
        assert!(adopt(&event(&content, false), signer(2).pubkey(), None).is_err());
    }

    #[test]
    fn stock_catalog_is_explicit_and_boundary_changes_require_restart() {
        let old = Persona::parse(r#"{"display_name":"Local"}"#).unwrap();
        assert!(
            old.catalog_content()
                .unwrap()
                .contains(r#""acp_command":"buzz-acp""#)
        );
        let mut new = old.clone();
        new.session_policy = SessionPolicy::Thread;
        assert!(session_restart_required(&old, &new));
        assert_ne!(old.source_version().unwrap(), new.source_version().unwrap());
    }
}
