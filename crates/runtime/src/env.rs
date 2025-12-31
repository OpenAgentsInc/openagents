//! Agent environment with mounted filesystem services.

use crate::budget::BudgetTracker;
use crate::fs::{AccessLevel, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat, WatchHandle};
use crate::identity::{InMemorySigner, SigningService};
use crate::namespace::Namespace;
use crate::services::{DeadletterFs, GoalsFs, IdentityFs, InboxFs, LogsFs, StatusFs, StatusSnapshot};
use crate::storage::AgentStorage;
use crate::types::AgentId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Default inbox capacity for agent environments.
const DEFAULT_INBOX_CAPACITY: usize = 1024;

/// Agent environment with mounted capabilities.
pub struct AgentEnv {
    /// Agent id.
    pub id: AgentId,
    /// Namespace of mounted services.
    pub namespace: Namespace,
    /// Status service.
    pub status: Arc<StatusFs>,
    /// Inbox service.
    pub inbox: Arc<InboxFs>,
    /// Deadletter service.
    pub deadletter: Arc<DeadletterFs>,
    /// Goals service.
    pub goals: Arc<GoalsFs>,
    /// Identity service.
    pub identity: Arc<IdentityFs>,
    /// Logs service.
    pub logs: Arc<LogsFs>,
    budgets: Arc<Mutex<HashMap<String, BudgetTracker>>>,
}

impl AgentEnv {
    /// Create an environment with default services and a stub signer.
    pub fn new(agent_id: AgentId, storage: Arc<dyn AgentStorage>) -> Self {
        let signer = Arc::new(InMemorySigner::new());
        Self::with_signer(agent_id, storage, signer)
    }

    /// Create an environment with default services and a provided signer.
    pub fn with_signer(
        agent_id: AgentId,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        let snapshot = Arc::new(RwLock::new(StatusSnapshot::empty(agent_id.clone())));
        let status = Arc::new(StatusFs::new(snapshot));
        let inbox = Arc::new(InboxFs::with_capacity(DEFAULT_INBOX_CAPACITY));
        let deadletter = Arc::new(DeadletterFs::new(inbox.deadletter()));
        let goals = Arc::new(GoalsFs::new(agent_id.clone(), storage));
        let identity = Arc::new(IdentityFs::new(agent_id.clone(), signer));
        let logs = Arc::new(LogsFs::new());

        let namespace = Namespace::new();
        let budgets = Arc::new(Mutex::new(HashMap::new()));
        let mut env = Self {
            id: agent_id,
            namespace,
            status,
            inbox,
            deadletter,
            goals,
            identity,
            logs,
            budgets,
        };

        env.mount("/status", env.status.clone(), AccessLevel::ReadOnly);
        env.mount("/inbox", env.inbox.clone(), AccessLevel::ReadWrite);
        env.mount("/deadletter", env.deadletter.clone(), AccessLevel::ReadOnly);
        env.mount("/goals", env.goals.clone(), AccessLevel::ReadWrite);
        env.mount("/identity", env.identity.clone(), AccessLevel::SignOnly);
        env.mount("/logs", env.logs.clone(), AccessLevel::ReadOnly);

        env
    }

    /// Open a file handle with access enforcement.
    pub fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        if !path.starts_with('/') {
            return Err(FsError::InvalidPath);
        }

        let (service, relative, access, mount_path) =
            self.namespace.resolve(path).ok_or(FsError::NotFound)?;

        ensure_access(&access, relative, flags.write)?;
        self.charge_budget(&mount_path, &access, flags.write)?;

        service.open(relative, flags)
    }

    /// Read all bytes from a path.
    pub fn read(&self, path: &str) -> FsResult<Vec<u8>> {
        let mut handle = self.open(path, OpenFlags::read())?;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = handle.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        Ok(buf)
    }

    /// Write bytes to a path.
    pub fn write(&self, path: &str, data: &[u8]) -> FsResult<()> {
        let mut handle = self.open(path, OpenFlags::write())?;
        handle.write(data)?;
        handle.flush()?;
        Ok(())
    }

    /// Watch a path for updates.
    pub fn watch(&self, path: &str) -> FsResult<Option<Box<dyn WatchHandle>>> {
        if !path.starts_with('/') {
            return Err(FsError::InvalidPath);
        }
        let (service, relative, access, _mount_path) =
            self.namespace.resolve(path).ok_or(FsError::NotFound)?;
        ensure_access(&access, relative, false)?;
        service.watch(relative)
    }

    /// List directory contents.
    pub fn list(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        if !path.starts_with('/') {
            return Err(FsError::InvalidPath);
        }
        let (service, relative, access, _mount_path) =
            self.namespace.resolve(path).ok_or(FsError::NotFound)?;
        ensure_access(&access, relative, false)?;
        service.readdir(relative)
    }

    /// Stat a path.
    pub fn stat(&self, path: &str) -> FsResult<Stat> {
        if !path.starts_with('/') {
            return Err(FsError::InvalidPath);
        }
        let (service, relative, access, _mount_path) =
            self.namespace.resolve(path).ok_or(FsError::NotFound)?;
        ensure_access(&access, relative, false)?;
        service.stat(relative)
    }

    /// Mount an additional service.
    pub fn mount(&mut self, path: &str, service: Arc<dyn FileService>, access: AccessLevel) {
        if let AccessLevel::Budgeted(policy) = &access {
            let mut budgets = self.budgets.lock().unwrap_or_else(|e| e.into_inner());
            budgets.entry(path.to_string()).or_insert_with(|| BudgetTracker::new(policy.clone()));
        }
        self.namespace.mount(path, service, access);
    }
}

fn is_sign_only_path(relative: &str) -> bool {
    matches!(relative, "pubkey" | "sign" | "verify" | "encrypt" | "decrypt")
}

fn ensure_access(access: &AccessLevel, relative: &str, write: bool) -> FsResult<()> {
    match access {
        AccessLevel::Disabled => Err(FsError::PermissionDenied),
        AccessLevel::ReadOnly if write => Err(FsError::PermissionDenied),
        AccessLevel::SignOnly => {
            if relative.is_empty() || is_sign_only_path(relative) {
                Ok(())
            } else {
                Err(FsError::PermissionDenied)
            }
        }
        _ => Ok(()),
    }
}

impl AgentEnv {
    fn charge_budget(&self, mount_path: &str, access: &AccessLevel, write: bool) -> FsResult<()> {
        if !write {
            return Ok(());
        }
        // Compute and containers manage budget reservations internally.
        if matches!(mount_path, "/compute" | "/containers") {
            return Ok(());
        }
        let policy = match access {
            AccessLevel::Budgeted(policy) => policy.clone(),
            _ => return Ok(()),
        };
        let mut budgets = self.budgets.lock().unwrap_or_else(|e| e.into_inner());
        let tracker = budgets
            .entry(mount_path.to_string())
            .or_insert_with(|| BudgetTracker::new(policy));
        tracker.charge(1).map_err(|_| FsError::BudgetExceeded)
    }
}
