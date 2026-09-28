//! One agent process and one ACP session in it: start, hand-shake, open or
//! reattach, set the mode, run prompt turns, and stop.

use std::time::Duration;

use serde_json::Value;

use crate::client::{ClientError, Handler, Ignore, OnCancel, Wait};
use crate::process::{Agent, Spec, Unstartable};
use crate::wire::{Initialized, Opened, Prompted};

/// How long the agent may take to answer each handshake request. An agent
/// waiting for a login on a terminal never answers; this bounds the wait.
pub const HANDSHAKE: Duration = Duration::from_secs(60);

/// How to open the session.
#[derive(Clone, Debug)]
pub struct Opening {
    pub spec: Spec,
    /// Reattach to this session with `session/load` when the agent can;
    /// otherwise, or when the load is refused, open a new one.
    pub resume: Option<String>,
    /// The new session's `_meta`.
    pub meta: Option<Value>,
    /// The mode to set after opening, by the agent's own id.
    pub mode: Option<String>,
}

/// Why a session could not be used.
#[derive(Debug)]
pub enum Failure {
    /// The agent did not start.
    Unstartable(String),
    /// The agent started and then failed the protocol or refused.
    Protocol {
        error: ClientError,
        /// The agent's last standard error lines.
        stderr: Vec<String>,
    },
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Unstartable(why) => f.write_str(why),
            Failure::Protocol { error, stderr } => {
                write!(f, "{error}")?;
                if !stderr.is_empty() {
                    write!(
                        f,
                        "; the agent's last standard error lines: {}",
                        stderr.join(" | ")
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl From<Unstartable> for Failure {
    fn from(error: Unstartable) -> Self {
        Failure::Unstartable(error.0)
    }
}

/// An open session.
pub struct Session {
    agent: Agent,
    pub initialized: Initialized,
    pub opened: Opened,
    /// Whether the session is the one [`Opening::resume`] named.
    pub resumed: bool,
    /// Why a requested reattachment fell back to a new session.
    pub resume_refused: Option<String>,
}

impl Session {
    /// Start the agent and open the session. `cancel` stops the handshake.
    ///
    /// # Errors
    /// See [`Failure`]. The agent is stopped before a failure returns.
    pub async fn open(opening: &Opening, cancel: &dyn Fn() -> bool) -> Result<Self, Failure> {
        let mut agent = Agent::start(&opening.spec)?;
        let wait = Wait {
            silence: HANDSHAKE,
            cancel,
            on_cancel: OnCancel::Abandon,
        };
        let cwd = opening.spec.cwd.to_string_lossy().into_owned();
        let result = async {
            let initialized = agent.client.initialize(wait, &mut Ignore).await?;
            let mut resume_refused = None;
            let mut reattached = None;
            if let Some(session) = &opening.resume {
                if initialized.agent_capabilities.load_session {
                    // The agent replays the session as updates; they are
                    // history, not this turn, so they are not handled.
                    match agent
                        .client
                        .load_session(session, &cwd, wait, &mut Ignore)
                        .await
                    {
                        Ok(opened) => reattached = Some(opened),
                        Err(ClientError::Refused { error, .. }) => {
                            resume_refused = Some(error.message);
                        }
                        Err(error) => return Err(error),
                    }
                } else {
                    resume_refused = Some("the agent cannot load a session".into());
                }
            }
            let resumed = reattached.is_some();
            let opened = match reattached {
                Some(opened) => opened,
                None => {
                    agent
                        .client
                        .new_session(&cwd, opening.meta.as_ref(), wait, &mut Ignore)
                        .await?
                }
            };
            if let Some(mode) = &opening.mode {
                agent
                    .client
                    .set_mode(&opened.session_id, mode, wait, &mut Ignore)
                    .await?;
            }
            Ok((initialized, opened, resumed, resume_refused))
        }
        .await;
        match result {
            Ok((initialized, opened, resumed, resume_refused)) => Ok(Session {
                agent,
                initialized,
                opened,
                resumed,
                resume_refused,
            }),
            Err(error) => {
                let stderr = agent.stderr_tail();
                agent.stop(Duration::from_secs(2)).await;
                Err(Failure::Protocol { error, stderr })
            }
        }
    }

    /// The session's identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.opened.session_id
    }

    /// The agent's process identifier.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.agent.pid()
    }

    /// The agent's last standard error lines.
    #[must_use]
    pub fn stderr_tail(&self) -> Vec<String> {
        self.agent.stderr_tail()
    }

    /// Run one prompt turn. While it runs, `cancel` returning `true` sends
    /// `session/cancel` and waits up to `grace` for the agent to end the
    /// turn as `cancelled`. `silence` bounds how long the agent may write
    /// nothing.
    ///
    /// # Errors
    /// See [`ClientError`].
    pub async fn prompt(
        &mut self,
        text: &str,
        silence: Duration,
        cancel: &dyn Fn() -> bool,
        grace: Duration,
        handler: &mut dyn Handler,
    ) -> Result<Prompted, ClientError> {
        let session = self.opened.session_id.clone();
        let wait = Wait {
            silence,
            cancel,
            on_cancel: OnCancel::Notify {
                session: &session,
                grace,
            },
        };
        self.agent
            .client
            .prompt(&session, text, wait, handler)
            .await
    }

    /// Stop the agent and its process group. Returns whether the group is
    /// empty.
    pub async fn close(self, grace: Duration) -> bool {
        self.agent.stop(grace).await
    }
}
