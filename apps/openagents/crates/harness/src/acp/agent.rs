//! The agent process behind one ACP run: a child of this engine, or an agent
//! a keeper holds (#11930), which outlives the engine.

use std::process::ExitStatus;
use std::time::Duration;

use super::child::Child;

pub(super) enum AgentProcess {
    Owned(Child),
    #[cfg(unix)]
    Kept(crate::process::RuntimeChild),
}

impl AgentProcess {
    pub(super) fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        match self {
            AgentProcess::Owned(child) => child.try_wait(),
            #[cfg(unix)]
            AgentProcess::Kept(child) => child.try_wait(),
        }
    }

    pub(super) async fn wait(&mut self) -> std::io::Result<ExitStatus> {
        match self {
            AgentProcess::Owned(child) => child.wait().await,
            #[cfg(unix)]
            AgentProcess::Kept(child) => child.wait().await,
        }
    }

    /// Stop the agent and everything it started. A kept agent's keeper does
    /// that, unless the engine is leaving it running for the next engine.
    pub(super) async fn shutdown(&mut self, grace: Duration) {
        match self {
            AgentProcess::Owned(child) => child.shutdown(grace).await,
            #[cfg(unix)]
            AgentProcess::Kept(child) => child.shutdown(Vec::new(), grace).await,
        }
    }

    /// After the agent exited: end what it left in its process group. The
    /// keeper does that for a kept agent.
    pub(super) fn request_group_shutdown(&self) {
        if let AgentProcess::Owned(child) = self {
            child.request_group_shutdown();
        }
    }

    #[cfg(unix)]
    pub(super) fn signal_target(&self) -> Option<i32> {
        match self {
            AgentProcess::Owned(child) => crate::process::signal_target(child),
            AgentProcess::Kept(child) => child.signal_target(),
        }
    }

    #[cfg(windows)]
    pub(super) fn signal_target(&self) -> Option<std::sync::Arc<crate::windows_process::Job>> {
        match self {
            AgentProcess::Owned(child) => crate::process::signal_target(child),
        }
    }

    #[cfg(unix)]
    pub(super) fn keeper(&self) -> Option<&std::sync::Arc<crate::keeper::kept::Kept>> {
        match self {
            AgentProcess::Owned(_) => None,
            AgentProcess::Kept(child) => child.keeper(),
        }
    }
}
