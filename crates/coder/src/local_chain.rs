//! Microcoder's no-setup provider chain (#11275): this computer's Codex
//! login, then its Claude Code login, then the OpenAgents gateway, falling
//! over to the next one mid-task without replaying completed commands.
//! Coder's `microcoder` tool and `microcoder --provider local` both resolve
//! providers here, so the same computer runs the same chain from either.

use std::cell::Cell;
use std::sync::Arc;

use microcoder_loop::models::{CodexGenerator, Generate, Generated};

/// Local logins followed by the OpenAgents gateway, without a model API key.
pub struct LocalGenerator {
    providers: GeneratorChain<LocalProvider>,
}

impl LocalGenerator {
    /// The provider answering now: `codex`, `claude`, or `openagents`.
    #[must_use]
    pub fn provider(&self) -> &'static str {
        self.providers.providers[self.providers.current.get()].name()
    }

    /// Every provider in the chain, in the order it is tried.
    #[must_use]
    pub fn providers(&self) -> Vec<&'static str> {
        self.providers
            .providers
            .iter()
            .map(LocalProvider::name)
            .collect()
    }

    /// The providers given up on so far, in the order they were tried.
    #[must_use]
    pub fn fell_back_from(&self) -> Vec<&'static str> {
        self.providers.providers[..self.providers.current.get()]
            .iter()
            .map(LocalProvider::name)
            .collect()
    }

    /// Where each provider sends a prompt: `codex:<url>#<model>`,
    /// `claude:<binary>#<model>`, or `openagents:gateway`.
    #[must_use]
    pub fn recipients(&self) -> Vec<String> {
        self.providers
            .providers
            .iter()
            .map(|provider| match provider {
                LocalProvider::Codex(generator) => format!(
                    "codex:{}#{}",
                    codex_transport::codex::BASE_URL,
                    generator.model
                ),
                LocalProvider::Claude(generator) => {
                    format!("claude:{}#{}", generator.binary.display(), generator.model)
                }
                LocalProvider::Gateway(_) => "openagents:gateway".to_owned(),
            })
            .collect()
    }
}

enum LocalProvider {
    Codex(Box<CodexGenerator>),
    Claude(microcoder_loop::claude::ClaudeGenerator),
    Gateway(crate::cloud::CloudLane<crate::relay::RelayDoor>),
}

impl LocalProvider {
    fn name(&self) -> &'static str {
        match self {
            Self::Codex(_) => "codex",
            Self::Claude(_) => "claude",
            Self::Gateway(_) => "openagents",
        }
    }
}

impl Generate for LocalProvider {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        match self {
            Self::Codex(generator) => {
                let mut generated = generator.generate(system, prompt).await;
                generated.model = codex_model_label(&generated.model);
                generated
            }
            Self::Claude(generator) => generator.generate(system, prompt).await,
            Self::Gateway(generator) => generator.generate(system, &gateway_prompt(prompt)).await,
        }
    }
}

// Model metadata is also the composer rail label. Qualify it at the provider
// boundary rather than inferring the backend from a GPT model name.
fn codex_model_label(model: &str) -> String {
    if model.is_empty() || model.starts_with("codex:") {
        model.to_owned()
    } else {
        format!("codex:{model}")
    }
}

/// Regenerate a failed action before dispatch, preserving earlier commands.
pub struct GeneratorChain<G> {
    pub providers: Vec<G>,
    pub current: Cell<usize>,
}

impl<G: Generate> Generate for GeneratorChain<G> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let mut spent = None;
        loop {
            let current = self.current.get();
            let generated = self.providers[current].generate(system, prompt).await;
            let retry_elsewhere = generated.action.is_err() && current + 1 < self.providers.len();
            let generated = microcoder_loop::failover::merge(spent.take(), generated);
            if !retry_elsewhere {
                return generated;
            }
            spent = Some(generated);
            self.current.set(current + 1);
        }
    }
}

impl Generate for LocalGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        self.providers.generate(system, prompt).await
    }
}

/// Keep the task and recent observations within the gateway's request size.
#[must_use]
pub fn gateway_prompt(prompt: &str) -> String {
    const HEAD: usize = 12 * 1024;
    const TAIL: usize = 32 * 1024;
    if prompt.len() <= HEAD + TAIL {
        return prompt.to_owned();
    }
    let mut head = HEAD;
    while !prompt.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = prompt.len().saturating_sub(TAIL);
    while !prompt.is_char_boundary(tail) {
        tail += 1;
    }
    format!(
        "{}\n\n[Earlier observations were compacted. Inspect current files to recover details; \
         do not repeat completed changes.]\n\n{}",
        &prompt[..head],
        &prompt[tail..]
    )
}

/// Resolve local model providers and the no-setup gateway fallback.
/// `session` keys the Codex prompt cache; `model` and `effort` override
/// `CODER_CODEX_MODEL` and `CODER_CODEX_REASONING` for the Codex login.
///
/// # Errors
///
/// No login is available here and the gateway is off or unreachable.
pub fn local_generator(
    session: &str,
    model: Option<String>,
    effort: Option<String>,
) -> Result<LocalGenerator, String> {
    use codex_transport::codex::{CodexTransport, Login};
    let mut providers = vec![];
    if let Some(path) = Login::default_path() {
        if let Ok(transport) = CodexTransport::new(path, session) {
            providers.push(LocalProvider::Codex(Box::new(CodexGenerator {
                transport,
                model: model
                    .or_else(|| std::env::var("CODER_CODEX_MODEL").ok())
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| microcoder_loop::MODEL.into()),
                effort: effort
                    .or_else(|| std::env::var("CODER_CODEX_REASONING").ok())
                    .filter(|v| !v.is_empty()),
                cache_key: session.to_owned(),
                images: vec![],
            })));
        }
    }
    if let Ok(generator) =
        microcoder_loop::claude::ClaudeGenerator::from_env(microcoder_loop::MODEL, None)
    {
        providers.push(LocalProvider::Claude(generator));
    }
    if std::env::var("CODER_CLOUD").as_deref() != Ok("off") {
        match crate::cloud::door(&|name| std::env::var(name).ok()) {
            Ok(door) => providers.push(LocalProvider::Gateway(crate::cloud::CloudLane::new(
                Arc::new(door),
            ))),
            Err(error) if providers.is_empty() => {
                return Err(format!(
                    "Cannot connect to the OpenAgents AI Gateway: {error}"
                ));
            }
            Err(_) => {}
        }
    }
    if providers.is_empty() {
        return Err("No Codex or Claude Code login is available, and the OpenAgents AI Gateway is turned off. Enable OpenRouter BYOK or unset CODER_CLOUD=off.".into());
    }
    Ok(LocalGenerator {
        providers: GeneratorChain {
            providers,
            current: Cell::new(0),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualifies_codex_model_metadata() {
        assert_eq!(codex_model_label("gpt-6.1-sol"), "codex:gpt-6.1-sol");
        assert_eq!(
            codex_model_label("gpt-6.1-sol:high"),
            "codex:gpt-6.1-sol:high"
        );
        assert_eq!(codex_model_label("codex:gpt-6.1-sol"), "codex:gpt-6.1-sol");
        assert_eq!(codex_model_label(""), "");
    }

    #[test]
    fn gateway_context_keeps_the_task_and_latest_results_without_splitting_unicode() {
        let prompt = format!(
            "TASK: review these files\n{}\nLATEST: changed file",
            "界".repeat(25_000)
        );
        let compact = gateway_prompt(&prompt);
        assert!(compact.starts_with("TASK: review these files"));
        assert!(compact.ends_with("LATEST: changed file"));
        assert!(compact.contains("observations were compacted"));
        assert!(compact.len() < 48 * 1024);
        assert_eq!(gateway_prompt("A short prompt"), "A short prompt");
    }
}
