//! Microcoder's no-setup provider chain (#11275): this computer's Codex
//! login, then its Claude Code login, then the OpenAgents gateway, falling
//! over to the next one mid-task without replaying completed commands.
//! Coder's `microcoder` tool and `microcoder --provider local` both resolve
//! providers here, so the same computer runs the same chain from either.
//!
//! The chain reads and writes the shared capacity book
//! ([`microcoder_loop::capacity`]): a provider the book says is out of its
//! usage or rate limit is skipped at build, a refusal a generation meets is
//! recorded so the next run — here or in a repository run — skips it
//! without asking, and once every provider is out the chain reports
//! [`Generate::out_of_capacity`] so the loop ends with the earliest reset
//! instead of retrying a dead login.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use microcoder_loop::capacity::{self, Provider, Refusal};
use microcoder_loop::failover::{self, ClaudeLane, Lane, Refusing, unix_now};
use microcoder_loop::models::{CodexGenerator, Exhausted, Generate, Generated};

/// Local logins followed by the OpenAgents gateway, without a model API key.
pub struct LocalGenerator {
    providers: GeneratorChain<LocalProvider>,
}

impl LocalGenerator {
    /// The provider answering now: `codex`, `claude`, or `openagents`.
    #[must_use]
    pub fn provider(&self) -> &'static str {
        self.providers
            .providers
            .get(self.providers.current.get())
            .map_or("none", LocalProvider::name)
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
        self.providers.providers[..self
            .providers
            .current
            .get()
            .min(self.providers.providers.len())]
            .iter()
            .map(LocalProvider::name)
            .collect()
    }

    /// The capacity refusals this chain holds: ones the book already
    /// recorded at build and ones a generation met since.
    #[must_use]
    pub fn refusals(&self) -> Vec<Refusal> {
        self.providers.refused.borrow().clone()
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
                LocalProvider::Claude(lane) => {
                    format!(
                        "claude:{}#{}",
                        lane.inner.binary.display(),
                        lane.inner.model
                    )
                }
                LocalProvider::Gateway(_) => "openagents:gateway".to_owned(),
            })
            .collect()
    }
}

pub enum LocalProvider {
    Codex(Box<CodexGenerator<Refusing<codex_transport::codex::CodexTransport>>>),
    Claude(ClaudeLane),
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

    /// The provider whose capacity a generation spends.
    fn capacity_provider(&self) -> Provider {
        match self {
            Self::Codex(_) => Provider::Codex,
            Self::Claude(_) => Provider::Claude,
            Self::Gateway(_) => Provider::Vertex,
        }
    }
}

impl Lane for LocalProvider {
    fn refusal(&self) -> Option<Refusal> {
        match self {
            Self::Codex(generator) => generator.refusal(),
            Self::Claude(lane) => lane.refusal(),
            Self::Gateway(lane) => lane.refusal(),
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
            Self::Claude(lane) => lane.generate(system, prompt).await,
            Self::Gateway(generator) => generator.generate(system, &gateway_prompt(prompt)).await,
        }
    }

    fn warm(&self, system: &str) {
        if let Self::Claude(lane) = self {
            lane.warm(system);
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
/// A provider that met a usage or rate limit has its refusal recorded in
/// `book` and kept in `refused`, so later chains skip it.
pub struct GeneratorChain<G> {
    pub providers: Vec<G>,
    pub current: Cell<usize>,
    /// The capacity book's directory a refusal is recorded into, when this
    /// chain shares one; `None` for a fixture chain.
    pub book: Option<PathBuf>,
    /// The refusals this chain knows: ones read from the book at build and
    /// ones met since, in the order met.
    pub refused: RefCell<Vec<Refusal>>,
}

impl<G: Lane> Generate for GeneratorChain<G> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let mut spent = None;
        loop {
            let current = self.current.get();
            let Some(lane) = self.providers.get(current) else {
                return microcoder_loop::failover::refused_generation(
                    "none",
                    false,
                    failover::NO_CAPACITY,
                );
            };
            let generated = lane.generate(system, prompt).await;
            if generated.action.is_err()
                && let Some(refusal) = lane.refusal()
            {
                if let Some(book) = &self.book {
                    let _ = capacity::record(book, refusal.clone());
                }
                self.refused.borrow_mut().push(refusal);
            }
            let retry_elsewhere = generated.action.is_err() && current + 1 < self.providers.len();
            let generated = failover::merge(spent.take(), generated);
            if !retry_elsewhere {
                return generated;
            }
            spent = Some(generated);
            self.current.set(current + 1);
        }
    }

    fn warm(&self, system: &str) {
        if let Some(lane) = self.providers.get(self.current.get()) {
            lane.warm(system);
        }
    }
}

impl Generate for LocalGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        self.providers.generate(system, prompt).await
    }

    /// Whether every provider in the chain met a capacity refusal, in the
    /// book at build or during this run. The earliest reset is the
    /// soonest any provider may answer again.
    fn out_of_capacity(&self) -> Option<Exhausted> {
        let refused = self.providers.refused.borrow();
        let all_out = !self.providers.providers.is_empty()
            && self.providers.providers.iter().all(|provider| {
                refused
                    .iter()
                    .any(|refusal| refusal.provider == provider.capacity_provider())
            });
        (all_out || self.providers.providers.is_empty() && !refused.is_empty()).then(|| Exhausted {
            resets_at: refused.iter().map(|refusal| refusal.until).min(),
        })
    }

    fn warm(&self, system: &str) {
        self.providers.warm(system);
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

/// The capacity book the interactive chain shares with repository runs:
/// the task store's `capacity.json`, so a login one flow finds empty is
/// skipped by the other until it resets.
fn book() -> Option<PathBuf> {
    crate::delegate_door::capacity_dir()
}

/// Resolve local model providers and the no-setup gateway fallback.
/// `session` keys the Codex prompt cache; `model` and `effort` override
/// `CODER_CODEX_MODEL` and `CODER_CODEX_REASONING` for the Codex login.
///
/// A provider the capacity book holds a live refusal for is left out: its
/// usage or rate limit already said when it may answer again.
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
    local_generator_in(book().as_deref(), |providers| {
        if let Some(path) = Login::default_path() {
            if let Ok(transport) = CodexTransport::new(path, session) {
                providers.push(LocalProvider::Codex(Box::new(CodexGenerator {
                    transport: Refusing::new(transport),
                    model: model
                        .clone()
                        .or_else(|| std::env::var("CODER_CODEX_MODEL").ok())
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| microcoder_loop::MODEL.into()),
                    effort: effort
                        .clone()
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
            providers.push(LocalProvider::Claude(ClaudeLane::new(generator)));
        }
        if std::env::var("CODER_CLOUD").as_deref() != Ok("off") {
            match crate::cloud::door(&|name| std::env::var(name).ok()) {
                Ok(door) => providers.push(LocalProvider::Gateway(crate::cloud::CloudLane::new(
                    Arc::new(door),
                ))),
                Err(_) => {}
            }
        }
    })
}

/// `local_generator` with the capacity book's directory named, for a caller
/// (or test) that keeps its own. `connect` appends the providers this
/// computer can use; the ones the book says are out are left off and kept
/// as known refusals.
pub fn local_generator_in(
    book_dir: Option<&Path>,
    connect: impl Fn(&mut Vec<LocalProvider>),
) -> Result<LocalGenerator, String> {
    let now = unix_now();
    let mut wanted = Vec::new();
    connect(&mut wanted);
    let booked = book_dir.map(capacity::Book::load);
    let mut seen = Vec::new();
    let providers: Vec<LocalProvider> = wanted
        .into_iter()
        .filter(|provider| {
            let blocking = booked
                .as_ref()
                .and_then(|book| book.blocking(provider.capacity_provider(), now))
                .cloned();
            if let Some(refusal) = blocking {
                seen.push(refusal);
                false
            } else {
                true
            }
        })
        .collect();
    if providers.is_empty() && seen.is_empty() {
        return Err("No Codex or Claude Code login is available, and the OpenAgents AI Gateway is turned off. Enable OpenRouter BYOK or unset CODER_CLOUD=off.".into());
    }
    Ok(LocalGenerator {
        providers: GeneratorChain {
            providers,
            current: Cell::new(0),
            book: book_dir.map(Path::to_path_buf),
            refused: RefCell::new(seen),
        },
    })
}

/// Coder's own agent on the OpenAgents AI Gateway only: no Codex login and
/// no Claude Code, for a task a Claude plan's usage limit paused and the
/// person handed to OpenAgents (#11295). It never starts `claude`.
///
/// # Errors
///
/// The gateway is turned off or unreachable.
pub fn gateway_generator() -> Result<LocalGenerator, String> {
    if std::env::var("CODER_CLOUD").as_deref() == Ok("off") {
        return Err("OpenAgents can't continue it here: OpenAgents is turned off on this computer (CODER_CLOUD=off).".into());
    }
    let door = crate::cloud::door(&|name| std::env::var(name).ok())
        .map_err(|error| format!("Cannot connect to the OpenAgents AI Gateway: {error}"))?;
    Ok(LocalGenerator {
        providers: GeneratorChain {
            providers: vec![LocalProvider::Gateway(crate::cloud::CloudLane::new(
                Arc::new(door),
            ))],
            current: Cell::new(0),
            book: book(),
            refused: RefCell::new(Vec::new()),
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

    fn fixture_auth() -> String {
        // A ChatGPT login whose access token expires in 2033.
        serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "eyJhbGciOiJub25lIn0.eyJleHAiOjIwMDAwMDAwMDB9.sig",
                "account_id": "acct-1"
            },
        })
        .to_string()
    }

    /// A login the book already holds a live refusal for is left off the
    /// chain, kept as a known refusal, and the chain that has only it is
    /// out of capacity with its reset.
    #[test]
    fn a_book_blocked_login_is_not_connected_and_the_chain_reports_it_out() {
        let dir = tempfile::tempdir().unwrap();
        let now = unix_now();
        capacity::record(
            dir.path(),
            Refusal::new(
                Provider::Codex,
                capacity::Kind::UsageLimit,
                now,
                Some(now + 3_600),
            ),
        )
        .unwrap();
        let login = dir.path().join("auth.json");
        std::fs::write(&login, fixture_auth()).unwrap();
        let generator = local_generator_in(Some(dir.path()), move |providers| {
            let transport =
                codex_transport::codex::CodexTransport::new(login.clone(), "s-1").unwrap();
            providers.push(LocalProvider::Codex(Box::new(CodexGenerator {
                transport: Refusing::new(transport),
                model: "fixture/model".into(),
                effort: None,
                cache_key: "s-1".into(),
                images: vec![],
            })));
        })
        .unwrap();
        assert!(generator.providers().is_empty());
        assert_eq!(generator.refusals().len(), 1);
        let exhausted = generator.out_of_capacity().expect("every door is out");
        assert_eq!(exhausted.resets_at, Some(now + 3_600));
    }

    /// A refusal a generation meets is written to the book, so the next
    /// chain — here or in a repository run — skips that login without
    /// asking it again.
    #[tokio::test]
    async fn a_refusal_met_during_generation_is_recorded_in_the_book() {
        struct Door {
            refusal: Option<Refusal>,
        }
        impl Generate for Door {
            async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
                if self.refusal.is_some() {
                    failover::refused_generation("fixture/model", false, "fixture refusal")
                } else {
                    Generated {
                        action: microcoder_loop::models::parse_action(
                            r#"{"rationale":"done","commands":[],"view":[],"freeze_tests":false,"expand":[],"finished":true,"reply":"done","ask":"none"}"#,
                        ),
                        model: "fixture/model".into(),
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        usd: Some(0.0),
                        known_usd: 0.0,
                        cost_unknown: None,
                        usd_upper: Some(0.0),
                        cost_basis: microcoder_loop::models::Basis::ListPrice,
                        milliseconds: 0,
                    }
                }
            }
        }
        impl Lane for Door {
            fn refusal(&self) -> Option<Refusal> {
                self.refusal.clone()
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let now = unix_now();
        let chain = GeneratorChain {
            providers: vec![
                Door {
                    refusal: Some(Refusal::new(
                        Provider::Claude,
                        capacity::Kind::UsageLimit,
                        now,
                        None,
                    )),
                },
                Door { refusal: None },
            ],
            current: Cell::new(0),
            book: Some(dir.path().to_path_buf()),
            refused: RefCell::new(Vec::new()),
        };
        let generated = chain.generate("system", "prompt").await;
        assert!(generated.action.is_ok());
        assert_eq!(chain.current.get(), 1);
        assert!(!capacity::Book::load(dir.path()).has_capacity(Provider::Claude, now));
        assert_eq!(chain.refused.borrow().len(), 1);
    }
}
