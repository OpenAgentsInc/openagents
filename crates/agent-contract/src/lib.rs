//! Shared agent definitions and NIP-AGENT payload validation. No execution or I/O.
use serde::{Deserialize, Serialize};
/// Runtime IDs resolve only to implementations installed and authorized by a host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSpec {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub runtime: String,
    pub model: Option<String>,
    pub access: Access,
    /// Runtimes to try, in order, when `runtime` isn't ready on the host.
    /// Not part of the agent's identity ([`AgentSpec::same_identity`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<RuntimeChoice>,
    /// How the agent runs on its own. Not part of the agent's identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autonomy: Option<Autonomy>,
}

/// How a saved agent runs on its own (#12029). Any agent can declare it;
/// hosts read it from the definition, never from the agent's name. Unknown
/// fields are ignored so later settings don't break older readers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Autonomy {
    /// The longest one shell command may run, in seconds. The host hands it
    /// to the runtime (Claude Code's `BASH_DEFAULT_TIMEOUT_MS` and
    /// `BASH_MAX_TIMEOUT_MS`), so a probe with no timeout of its own can't
    /// hang the agent's turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_timeout_secs: Option<u32>,
}

/// The allowed range of [`Autonomy::shell_timeout_secs`].
pub const MIN_SHELL_TIMEOUT_SECS: u32 = 10;
pub const MAX_SHELL_TIMEOUT_SECS: u32 = 3600;

/// One runtime the host may use for an agent, with its model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeChoice {
    pub runtime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// `runtime` value meaning "the host's own default runtime".
pub const ANY_RUNTIME: &str = "any";
/// Most fallback runtimes an agent may list.
pub const MAX_FALLBACKS: usize = 4;

fn valid_runtime(runtime: &str) -> bool {
    !runtime.is_empty()
        && runtime.len() <= 128
        && runtime
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn valid_model(model: Option<&String>) -> bool {
    model.is_none_or(|m| !m.is_empty() && m.len() <= 256 && !m.chars().any(char::is_control))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    HostDefault,
    ReadOnly,
    WorkspaceWrite,
}

impl AgentSpec {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty()
            || self.name.len() > 80
            || self.name.chars().any(char::is_control)
        {
            return Err("Enter a short name.");
        }
        if self.description.len() > 512 || self.description.chars().any(char::is_control) {
            return Err("Enter a shorter description.");
        }
        if self.instructions.trim().is_empty()
            || self.instructions.len() > 16_384
            || self.instructions.contains('\0')
        {
            return Err("Add instructions or shorten the text.");
        }
        if !valid_runtime(&self.runtime) {
            return Err("Choose a supported runtime.");
        }
        if !valid_model(self.model.as_ref()) {
            return Err("Choose a supported model.");
        }
        if self.runtime == "sdk" && (self.model.is_none() || self.access != Access::HostDefault) {
            return Err("Choose an installed runner and this computer's access settings.");
        }
        if self.runtime == ANY_RUNTIME && self.model.is_some() {
            return Err("Choose a runtime before choosing a model.");
        }
        if let Some(a) = &self.autonomy
            && a.shell_timeout_secs
                .is_some_and(|t| !(MIN_SHELL_TIMEOUT_SECS..=MAX_SHELL_TIMEOUT_SECS).contains(&t))
        {
            return Err("Choose a shell timeout between 10 seconds and an hour.");
        }
        if self.fallbacks.len() > MAX_FALLBACKS {
            return Err("Choose fewer fallback runtimes.");
        }
        for (i, f) in self.fallbacks.iter().enumerate() {
            if !valid_runtime(&f.runtime)
                || f.runtime == "sdk"
                || f.runtime == ANY_RUNTIME
                || f.runtime == self.runtime
                || self.fallbacks[..i].iter().any(|e| e.runtime == f.runtime)
            {
                return Err("Choose different fallback runtimes.");
            }
            if !valid_model(f.model.as_ref()) {
                return Err("Choose a supported model.");
            }
        }
        Ok(())
    }

    /// The runtimes to try, in order. Empty when the agent leaves it to the
    /// host (`runtime: "any"` and no fallbacks).
    pub fn runtime_order(&self) -> Vec<RuntimeChoice> {
        (self.runtime != ANY_RUNTIME)
            .then(|| RuntimeChoice {
                runtime: self.runtime.clone(),
                model: self.model.clone(),
            })
            .into_iter()
            .chain(self.fallbacks.iter().cloned())
            .collect()
    }

    /// Whether two specs describe the same agent. The runtime, model,
    /// fallbacks and autonomy are settings of the agent, not part of who it is.
    pub fn same_identity(&self, other: &Self) -> bool {
        self.name == other.name
            && self.description == other.description
            && self.instructions == other.instructions
            && self.access == other.access
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDefinition {
    pub id: String,
    pub revision: u64,
    pub spec: AgentSpec,
    pub deleted: bool,
}
pub fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub const SCHEMA: &str = "openagents.agent.definition.v1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub definition: AgentDefinition,
}
impl Profile {
    pub fn new(definition: AgentDefinition) -> Result<Self, &'static str> {
        let p = Self {
            schema: SCHEMA.into(),
            definition,
        };
        p.validate()?;
        Ok(p)
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA || !valid_id(&self.definition.id) || self.definition.revision == 0
        {
            return Err("Invalid agent profile.");
        }
        self.definition.spec.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> AgentSpec {
        AgentSpec {
            name: "Foreman".into(),
            description: String::new(),
            instructions: "Oversee the work.".into(),
            runtime: "claude-code".into(),
            model: Some("claude-opus-5-5".into()),
            access: Access::HostDefault,
            fallbacks: vec![],
            autonomy: None,
        }
    }
    fn choice(runtime: &str) -> RuntimeChoice {
        RuntimeChoice {
            runtime: runtime.into(),
            model: None,
        }
    }

    #[test]
    fn fallbacks_are_ordered_optional_and_omitted_when_empty() {
        let mut s = spec();
        let json = serde_json::to_value(&s).unwrap();
        assert!(json.get("fallbacks").is_none());
        s.fallbacks = vec![choice("codex"), choice("opencode")];
        s.validate().unwrap();
        let order: Vec<_> = s.runtime_order().into_iter().map(|c| c.runtime).collect();
        assert_eq!(order, ["claude-code", "codex", "opencode"]);
        let back: AgentSpec = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        s.runtime = ANY_RUNTIME.into();
        s.model = None;
        s.fallbacks.clear();
        s.validate().unwrap();
        assert!(s.runtime_order().is_empty());
    }

    #[test]
    fn autonomy_is_an_optional_setting_with_a_bounded_shell_timeout() {
        let mut s = spec();
        assert!(serde_json::to_value(&s).unwrap().get("autonomy").is_none());
        let plain = s.clone();
        s.autonomy = Some(Autonomy {
            shell_timeout_secs: Some(120),
        });
        s.validate().unwrap();
        assert!(s.same_identity(&plain), "a setting, not identity");
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["autonomy"]["shell_timeout_secs"], 120);
        assert_eq!(serde_json::from_value::<AgentSpec>(json).unwrap(), s);
        for bad in [0, 9, 3601] {
            s.autonomy = Some(Autonomy {
                shell_timeout_secs: Some(bad),
            });
            assert!(s.validate().is_err(), "{bad}");
        }
    }

    #[test]
    fn fallbacks_reject_repeats_runners_and_long_lists() {
        for fallbacks in [
            vec![choice("claude-code")],
            vec![choice("codex"), choice("codex")],
            vec![choice("sdk")],
            vec![choice(ANY_RUNTIME)],
            vec![choice("sh -c x")],
            vec![choice("a"), choice("b"), choice("c"), choice("d"), choice("e")],
        ] {
            let mut s = spec();
            s.fallbacks = fallbacks;
            assert!(s.validate().is_err());
        }
        let mut s = spec();
        s.runtime = ANY_RUNTIME.into();
        assert!(s.validate().is_err(), "a model needs a runtime");
    }

    #[test]
    fn runtime_and_model_are_not_identity() {
        let a = spec();
        let mut b = a.clone();
        b.runtime = "codex".into();
        b.model = None;
        b.fallbacks = vec![choice("opencode")];
        assert!(a.same_identity(&b));
        b.instructions = "Something else".into();
        assert!(!a.same_identity(&b));
    }
}
