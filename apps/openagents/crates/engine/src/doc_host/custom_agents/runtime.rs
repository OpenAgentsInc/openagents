//! Which runtime runs a saved agent on this computer (#11966). The agent's
//! runtime and fallbacks are an ordered preference, not part of the agent:
//! each turn uses the first one that is installed, turned on, signed in,
//! offers the chosen model, and enforces the agent's access, then this
//! computer's own runtimes. Readiness is read without starting a login, so a
//! fallback never opens a sign-in window.
use super::{error, validate_runtime_access};
use crate::{EngineError, registry::HarnessRegistry};
use openagents_account::openagents::agents::{AgentSpec, RuntimeChoice};
use openagents_proto::{HarnessId, saved_agent::runtime_label};
use std::path::{Path, PathBuf};

/// Why a runtime in the agent's order was passed over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Skip {
    Unsupported,
    Access,
    NotInstalled,
    TurnedOff,
    SignedOut,
    Model,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Resolved {
    pub harness: HarnessId,
    pub model: Option<String>,
    /// The agent's first choice and why it wasn't used; `None` when it was
    /// (or when the agent leaves the choice to this computer).
    pub passed: Option<(String, Skip)>,
}

/// This computer's own order for agents that leave the choice to it, or
/// whose choices are all unavailable.
pub(crate) const HOST_DEFAULTS: [HarnessId; 3] =
    [HarnessId::ClaudeCode, HarnessId::Codex, HarnessId::Opencode];

/// Resolve the runtime. `check` answers whether one runtime (with a model)
/// is ready. A runtime that only looks signed out is used when nothing else
/// is ready, so the runtime itself can say what to do.
pub(crate) async fn resolve<F, Fut>(
    spec: &AgentSpec,
    host_order: &[HarnessId],
    mut check: F,
) -> Result<Resolved, EngineError>
where
    F: FnMut(HarnessId, Option<String>) -> Fut,
    Fut: std::future::Future<Output = Result<(), Skip>>,
{
    let order = spec.runtime_order();
    let mut candidates: Vec<(String, Option<HarnessId>, Option<String>)> = order
        .iter()
        .map(|RuntimeChoice { runtime, model }| {
            (
                runtime.clone(),
                serde_json::from_value(runtime.clone().into()).ok(),
                model.clone(),
            )
        })
        .collect();
    for h in host_order {
        if !candidates.iter().any(|(_, id, _)| id == &Some(*h)) {
            let name = serde_json::to_value(h)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            candidates.push((name, Some(*h), None));
        }
    }
    let mut first: Option<(String, Skip)> = None;
    let mut signed_out: Option<Resolved> = None;
    for (i, (name, harness, model)) in candidates.into_iter().enumerate() {
        let outcome = match harness {
            None => Err(Skip::Unsupported),
            Some(h) => match validate_runtime_access(h, spec.access) {
                Err(_)
                    if !matches!(
                        h,
                        HarnessId::ClaudeCode
                            | HarnessId::Codex
                            | HarnessId::Opencode
                            | HarnessId::Sdk
                            | HarnessId::Mock
                    ) =>
                {
                    Err(Skip::Unsupported)
                }
                Err(_) => Err(Skip::Access),
                Ok(()) => check(h, model.clone()).await,
            },
        };
        let passed = first.clone();
        match (outcome, harness) {
            (Ok(()), Some(harness)) => {
                return Ok(Resolved {
                    harness,
                    model,
                    passed,
                });
            }
            (Err(Skip::SignedOut), Some(harness)) if signed_out.is_none() => {
                signed_out = Some(Resolved {
                    harness,
                    model,
                    passed: passed.clone(),
                });
                if i == 0 && !order.is_empty() {
                    first = Some((name, Skip::SignedOut));
                }
            }
            (Err(skip), _) => {
                if i == 0 && !order.is_empty() {
                    first = Some((name, skip));
                }
            }
            (Ok(()), None) => unreachable!("an unknown runtime is never checked"),
        }
    }
    signed_out.ok_or_else(|| {
        error("None of this agent's runtimes is ready on this computer. Turn one on in Settings.")
    })
}

/// The line the conversation shows when the runtime differs from the agent's
/// first choice or from the previous turn's runtime.
pub(crate) fn note(
    agent: &str,
    resolved: &Resolved,
    previous: Option<HarnessId>,
) -> Option<String> {
    let now = runtime_label(resolved.harness);
    if let Some((first, skip)) = &resolved.passed {
        let first = serde_json::from_value::<HarnessId>(first.clone().into())
            .map(runtime_label)
            .unwrap_or(first.as_str());
        let why = match skip {
            Skip::Unsupported => "isn't supported here",
            Skip::Access => "can't use this agent's access setting",
            Skip::NotInstalled => "isn't installed on this computer",
            Skip::TurnedOff => "is turned off on this computer",
            Skip::SignedOut => "isn't signed in on this computer",
            Skip::Model => "doesn't offer the chosen model on this computer",
        };
        return (previous != Some(resolved.harness))
            .then(|| format!("{agent} is running on {now} because {first} {why}."));
    }
    (previous.is_some() && previous != Some(resolved.harness))
        .then(|| format!("{agent} now runs on {now}."))
}

/// Where each runtime keeps its sign-in, read without starting anything.
#[derive(Clone, Debug)]
pub(crate) struct SignIns {
    pub claude_config_file: PathBuf,
    pub codex_auth_file: PathBuf,
    pub env: fn(&str) -> bool,
}

impl SignIns {
    pub(crate) fn detect() -> Self {
        let config = crate::agent_accounts::AgentAccountsConfig::detect(Path::new(""));
        Self {
            claude_config_file: config.claude_config_file,
            codex_auth_file: config.codex_home.join("auth.json"),
            env: |name| std::env::var_os(name).is_some_and(|v| !v.is_empty()),
        }
    }

    /// `false` only when the runtime is known to have no sign-in. Others
    /// (OpenCode's providers, SDK runners) count as signed in.
    pub(crate) fn signed_in(&self, harness: HarnessId) -> bool {
        let env = |names: &[&str]| names.iter().any(|n| (self.env)(n));
        match harness {
            HarnessId::ClaudeCode => {
                env(&[
                    "ANTHROPIC_API_KEY",
                    "ANTHROPIC_AUTH_TOKEN",
                    "CLAUDE_CODE_OAUTH_TOKEN",
                    "CLAUDE_CODE_USE_BEDROCK",
                    "CLAUDE_CODE_USE_VERTEX",
                ]) || std::fs::read(&self.claude_config_file)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                    .is_some_and(|c| {
                        c.get("oauthAccount").is_some_and(|a| !a.is_null())
                            || c.get("primaryApiKey").is_some_and(|k| k.is_string())
                    })
            }
            HarnessId::Codex => {
                env(&["OPENAI_API_KEY", "CODEX_API_KEY"]) || self.codex_auth_file.is_file()
            }
            _ => true,
        }
    }
}

/// Whether one runtime is ready on this computer.
pub(crate) async fn check(
    registry: &HarnessRegistry,
    sign_ins: &SignIns,
    harness: HarnessId,
    model: Option<&str>,
) -> Result<(), Skip> {
    let descriptor = registry
        .descriptors()
        .into_iter()
        .find(|d| d.id == harness)
        .ok_or(Skip::NotInstalled)?;
    if !descriptor.installed {
        return Err(Skip::NotInstalled);
    }
    if !crate::registry::descriptor_enabled(&descriptor) {
        return Err(Skip::TurnedOff);
    }
    if !sign_ins.signed_in(harness) {
        return Err(Skip::SignedOut);
    }
    if let Some(model) = model {
        let models = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            registry.discover_models(harness),
        )
        .await
        .map_err(|_| Skip::Model)?
        .map_err(|_| Skip::Model)?;
        if !models.iter().any(|m| m.id == model) {
            return Err(Skip::Model);
        }
    }
    Ok(())
}

/// This computer's runtimes in its registry order, limited to those an agent
/// may be given by default.
pub(crate) fn host_order(registry: &HarnessRegistry) -> Vec<HarnessId> {
    registry
        .descriptors()
        .into_iter()
        .map(|d| d.id)
        .filter(|h| HOST_DEFAULTS.contains(h))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_account::openagents::agents::Access;
    use std::collections::HashMap;

    fn spec(runtime: &str, fallbacks: &[&str]) -> AgentSpec {
        AgentSpec {
            name: "Foreman".into(),
            description: String::new(),
            instructions: "Oversee.".into(),
            runtime: runtime.into(),
            model: None,
            access: Access::HostDefault,
            fallbacks: fallbacks
                .iter()
                .map(|r| RuntimeChoice {
                    runtime: (*r).into(),
                    model: None,
                })
                .collect(),
            autonomy: None,
        }
    }
    async fn run(
        spec: &AgentSpec,
        ready: &[(HarnessId, Result<(), Skip>)],
    ) -> Result<Resolved, EngineError> {
        let ready: HashMap<_, _> = ready.iter().cloned().collect();
        resolve(spec, &HOST_DEFAULTS, |h, _| {
            let r = ready.get(&h).cloned().unwrap_or(Err(Skip::NotInstalled));
            async move { r }
        })
        .await
    }

    #[tokio::test]
    async fn the_first_ready_runtime_in_the_agents_order_runs() {
        let s = spec("claude-code", &["codex"]);
        let r = run(
            &s,
            &[(HarnessId::ClaudeCode, Ok(())), (HarnessId::Codex, Ok(()))],
        )
        .await
        .unwrap();
        assert_eq!((r.harness, r.passed), (HarnessId::ClaudeCode, None));

        let r = run(
            &s,
            &[
                (HarnessId::ClaudeCode, Err(Skip::NotInstalled)),
                (HarnessId::Codex, Ok(())),
            ],
        )
        .await
        .unwrap();
        assert_eq!(r.harness, HarnessId::Codex);
        assert_eq!(r.passed, Some(("claude-code".into(), Skip::NotInstalled)));
        assert_eq!(
            note("Foreman", &r, Some(HarnessId::ClaudeCode)).as_deref(),
            Some(
                "Foreman is running on Codex because Claude Code isn't installed on this computer."
            )
        );
        assert_eq!(note("Foreman", &r, Some(HarnessId::Codex)), None);
    }

    #[tokio::test]
    async fn the_computers_default_follows_and_a_signed_out_runtime_is_the_last_resort() {
        let s = spec("claude-code", &[]);
        let r = run(
            &s,
            &[
                (HarnessId::ClaudeCode, Err(Skip::SignedOut)),
                (HarnessId::Opencode, Ok(())),
            ],
        )
        .await
        .unwrap();
        assert_eq!(r.harness, HarnessId::Opencode);
        assert_eq!(r.passed, Some(("claude-code".into(), Skip::SignedOut)));
        let r = run(&s, &[(HarnessId::ClaudeCode, Err(Skip::SignedOut))])
            .await
            .unwrap();
        assert_eq!(r.harness, HarnessId::ClaudeCode);
        assert!(run(&s, &[]).await.is_err());

        let any = spec(openagents_account::openagents::agents::ANY_RUNTIME, &[]);
        let r = run(&any, &[(HarnessId::Codex, Ok(()))]).await.unwrap();
        assert_eq!((r.harness, r.passed.clone()), (HarnessId::Codex, None));
        assert_eq!(note("Foreman", &r, None), None);
        assert_eq!(
            note("Foreman", &r, Some(HarnessId::ClaudeCode)).as_deref(),
            Some("Foreman now runs on Codex.")
        );
    }

    #[tokio::test]
    async fn access_and_unknown_runtimes_are_passed_over_without_a_check() {
        let mut s = spec("cursor", &["claude-code", "codex"]);
        s.access = Access::ReadOnly;
        let checked = std::sync::Mutex::new(vec![]);
        let r = resolve(&s, &HOST_DEFAULTS, |h, _| {
            checked.lock().unwrap().push(h);
            async { Ok(()) }
        })
        .await
        .unwrap();
        assert_eq!(r.harness, HarnessId::Codex);
        assert_eq!(r.passed, Some(("cursor".into(), Skip::Unsupported)));
        assert_eq!(*checked.lock().unwrap(), [HarnessId::Codex]);
    }

    #[test]
    fn sign_in_is_read_from_files_and_keys_only() {
        let root = tempfile::tempdir().unwrap();
        let mut s = SignIns {
            claude_config_file: root.path().join("claude.json"),
            codex_auth_file: root.path().join("auth.json"),
            env: |_| false,
        };
        assert!(!s.signed_in(HarnessId::ClaudeCode));
        assert!(!s.signed_in(HarnessId::Codex));
        assert!(s.signed_in(HarnessId::Opencode));
        std::fs::write(
            &s.claude_config_file,
            r#"{"oauthAccount":{"emailAddress":"a@b"}}"#,
        )
        .unwrap();
        std::fs::write(&s.codex_auth_file, "{}").unwrap();
        assert!(s.signed_in(HarnessId::ClaudeCode));
        assert!(s.signed_in(HarnessId::Codex));
        std::fs::write(&s.claude_config_file, "{}").unwrap();
        s.env = |name| name == "ANTHROPIC_API_KEY";
        assert!(s.signed_in(HarnessId::ClaudeCode));
    }
}
