//! One session's hooks, as every backend runs them: a [`Runner`] shared by
//! the session's callbacks, and the host's [`Asker`] for an `Ask`.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use coder_delegate::session::HookBackend;

use super::compose::{Applied, Budget, Refused, Runner, check_backend};
use super::events::Call;
use super::outcome::{Permission, Stop};
use super::registry::Registry;

/// What an `Ask` came to: the person's answer, as a permission. Usually
/// `NoOpinion` (go ahead) or `Deny`.
pub type Asked = Pin<Box<dyn Future<Output = Permission> + Send>>;

/// The host's approval surface: answers an `Ask` given what the hooks came
/// to (the final input, the step).
pub type Asker = Arc<dyn Fn(Applied) -> Asked + Send + Sync>;

/// One session's hooks on one backend.
#[derive(Clone)]
pub struct Hooks {
    runner: Arc<Mutex<Runner>>,
    asker: Option<Asker>,
}

impl std::fmt::Debug for Hooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hooks")
            .field("registry", &self.registry())
            .field("asker", &self.asker.is_some())
            .finish()
    }
}

impl Hooks {
    /// `registry`'s hooks on `backend`.
    ///
    /// # Errors
    ///
    /// The hooks `backend` can't honor.
    pub fn new(
        registry: Registry,
        budget: Budget,
        backend: HookBackend,
    ) -> Result<Self, Vec<Refused>> {
        check_backend(&registry, backend)?;
        Ok(Hooks {
            runner: Arc::new(Mutex::new(Runner::new(registry, budget))),
            asker: None,
        })
    }

    /// `registry`'s hooks on `backend`, kept even where `backend` can't
    /// honor one: the caller has stated those (an ACP agent's asks are
    /// still answered by them, #11508).
    pub(crate) fn keeping(registry: Registry, budget: Budget, backend: HookBackend) -> Self {
        let _ = backend;
        Hooks {
            runner: Arc::new(Mutex::new(Runner::new(registry, budget))),
            asker: None,
        }
    }

    /// The session's runner, for a host that answers the same hooks
    /// elsewhere (the hook bridge's socket, [`super::bridge::Host`]).
    #[must_use]
    pub fn runner(&self) -> Arc<Mutex<Runner>> {
        self.runner.clone()
    }

    /// Answers each `Ask` through `asker`.
    #[must_use]
    pub fn asking(mut self, asker: Asker) -> Self {
        self.asker = Some(asker);
        self
    }

    /// The session's registry.
    #[must_use]
    pub fn registry(&self) -> Registry {
        self.runner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .registry
            .clone()
    }

    /// Runs the hooks on `call`, off the async executor when there is one,
    /// and resolves an `Ask` through the asker; with none, an `Ask` is held:
    /// denied with its step.
    pub async fn fire(&self, call: Call) -> Applied {
        let runner = self.runner.clone();
        let fallback = call.clone();
        let fire = move || {
            runner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .fire(&call)
        };
        let mut applied = if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::spawn_blocking(fire)
                .await
                .unwrap_or_else(|_| Applied::unchanged(&fallback))
        } else {
            fire()
        };
        if let Permission::Ask { step } = &applied.permission {
            applied.permission = match &self.asker {
                Some(asker) => asker(applied.clone()).await,
                None => Permission::Deny {
                    reason: step.clone(),
                },
            };
        }
        applied
    }
}

impl Applied {
    /// The outcome when no hook answered: the call as it came.
    #[must_use]
    pub fn unchanged(call: &Call) -> Self {
        Applied {
            event: call.event,
            tool: call.tool.clone(),
            permission: Permission::NoOpinion,
            decided_by: None,
            input: call.input.clone(),
            rewritten_by: Vec::new(),
            context: Vec::new(),
            dropped: Vec::new(),
            stop: Stop::Continue,
            failures: Vec::new(),
        }
    }

    /// The kept context's texts, each labeled with its source.
    #[must_use]
    pub fn context_lines(&self) -> Vec<String> {
        self.context
            .iter()
            .map(|c| format!("[{}] {}", c.source, c.text))
            .collect()
    }
}
