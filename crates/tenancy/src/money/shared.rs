//! Native journals retain usage and caps; the canonical controller owns funds.
use super::{Ledger, Mutation, Operation, Price, Usage};
use pay_ledger::shared::{Client, ClientConfig, Outcome};
use receipts::shared_spend::{Mode, Reference};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
fn directory_file(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "Retained custody directory unavailable.".into())
}
fn check_directory(file: &File, path: &Path) -> Result<(), String> {
    let held = file
        .metadata()
        .map_err(|_| "Custody directory metadata unavailable.")?;
    let current = std::fs::symlink_metadata(path).map_err(|_| "Custody directory changed.")?;
    if !current.is_dir()
        || current.dev() != held.dev()
        || current.ino() != held.ino()
        || current.uid() != unsafe { libc::geteuid() }
    {
        return Err("Original custody directory changed.".into());
    }
    Ok(())
}
fn marker(directory: &Path, workspace: &str) -> PathBuf {
    directory
        .join("shared-spend")
        .join(format!("{:x}.json", Sha256::digest(workspace.as_bytes())))
}
/// A validated canonical projection can be appended under fresh native authority.
/// Its private fields prevent callers from substituting another mutation.
pub struct Prepared {
    mutation: Mutation,
    mode: Mode,
    head: String,
}
pub fn required(directory: &Path, workspace: &str) -> Result<Option<Mode>, String> {
    let root = directory_file(directory)?;
    let path = marker(directory, workspace);
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Shared custody marker unavailable.".into()),
        Ok(_) => {}
    }
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .map_err(|_| "Shared custody marker unavailable.")?;
    let m = f
        .metadata()
        .map_err(|_| "Shared custody metadata unavailable.")?;
    let parent = std::fs::symlink_metadata(path.parent().unwrap())
        .map_err(|_| "Shared custody directory unavailable.")?;
    if !m.is_file()
        || m.nlink() != 1
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || m.len() > 64 * 1024
        || !parent.is_dir()
        || parent.mode() & 0o077 != 0
        || parent.uid() != unsafe { libc::geteuid() }
    {
        return Err("Shared custody marker is not private.".into());
    }
    let parent_file = directory_file(path.parent().unwrap())?;
    let mut bytes = Vec::new();
    f.take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Shared custody read failed.")?;
    let visible = std::fs::symlink_metadata(&path).map_err(|_| "Custody marker changed.")?;
    if visible.dev() != m.dev()
        || visible.ino() != m.ino()
        || visible.len() != m.len()
        || bytes.len() as u64 != m.len()
    {
        return Err("Original custody marker changed.".into());
    }
    check_directory(&root, directory)?;
    check_directory(&parent_file, path.parent().unwrap())?;
    let mode: Mode =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid retained shared custody.")?;
    if mode.source.workspace.as_deref() != Some(workspace) {
        return Err("Shared native workspace changed.".into());
    }
    Ok(Some(mode))
}
pub fn install_marker(directory: &Path, mode: &Mode) -> Result<(), String> {
    let workspace = mode
        .source
        .workspace
        .as_deref()
        .ok_or("Exact shared workspace required.")?;
    if let Some(old) = required(directory, workspace)? {
        return if &old == mode {
            Ok(())
        } else {
            Err("Shared workspace custody is immutable.".into())
        };
    }
    let path = marker(directory, workspace);
    let parent = path.parent().unwrap();
    match std::fs::create_dir(parent) {
        Ok(()) => std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "Private marker directory failed.")?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("Private marker directory failed.".into()),
    }
    let m =
        std::fs::symlink_metadata(parent).map_err(|_| "Private marker directory unavailable.")?;
    if !m.is_dir() || m.mode() & 0o077 != 0 || m.uid() != unsafe { libc::geteuid() } {
        return Err("Private marker directory required.".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .map_err(|_| "Shared custody marker already exists.")?;
    file.write_all(&serde_json::to_vec(mode).map_err(|_| "Invalid shared marker.")?)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Shared custody seal failed.")?;
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Shared directory seal failed.")?;
    Ok(())
}
pub fn migrate_marker(directory: &Path, old: &Mode, new: &Mode) -> Result<(), String> {
    let root = directory_file(directory)?;
    let workspace = old
        .source
        .workspace
        .as_deref()
        .ok_or("Original native workspace required.")?;
    if !old.same_native(new) {
        return Err("Native custody cannot move to another source or pool.".into());
    }
    let current = required(directory, workspace)?.ok_or("Original custody marker required.")?;
    if &current == new {
        return Ok(());
    }
    if &current != old {
        return Err("Retained original custody changed.".into());
    }
    let path = marker(directory, workspace);
    let parent = path.parent().unwrap();
    let parent_file = directory_file(parent)?;
    let temporary = parent.join(format!(".{}.tmp", new.binding_digest));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temporary)
        .map_err(|_| "Private migration intent already exists; inspect before retrying.")?;
    file.write_all(&serde_json::to_vec(new).map_err(|_| "Invalid custody migration.")?)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Custody migration seal failed.")?;
    if required(directory, workspace)?.as_ref() != Some(old) {
        return Err("Custody changed before migration.".into());
    }
    check_directory(&root, directory)?;
    check_directory(&parent_file, parent)?;
    std::fs::rename(&temporary, &path).map_err(|_| "Custody migration is uncertain.")?;
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Custody directory seal failed.")?;
    check_directory(&root, directory)?;
    check_directory(&parent_file, parent)?;
    Ok(())
}
impl Ledger {
    pub fn migrate_shared(
        &mut self,
        workspace: &str,
        previous: &str,
        mode: Mode,
    ) -> Result<bool, String> {
        self.apply(Mutation {
            workspace: workspace.into(),
            source: format!("shared-migration:{}", mode.binding),
            audit: mode.binding_digest.clone(),
            operation: Operation::SharedMigration {
                previous: previous.into(),
                mode,
            },
        })
    }
    /// Complete controller IPC before acquiring an Accounts authority guard.
    pub fn prepare_shared(&self, mutation: Mutation) -> Result<Prepared, String> {
        self.shared_guard(&mutation)?;
        let mode = self
            .shared_mode(&mutation.workspace)
            .ok_or("Native shared mode is required.")?
            .clone();
        Ok(Prepared {
            mutation,
            mode,
            head: self.head.clone(),
        })
    }
    /// Commit only the original validated projection; this performs no IPC.
    pub fn commit_shared(&mut self, prepared: Prepared) -> Result<bool, String> {
        if self.head != prepared.head
            || self.shared_mode(&prepared.mutation.workspace) != Some(&prepared.mode)
        {
            return Err("Native shared projection changed while awaiting authority.".into());
        }
        self.append_at(prepared.mutation, super::now()?)
    }
    pub fn shared_activation_empty(&self, workspace: &str) -> Result<(), String> {
        let account = self
            .state
            .accounts
            .get(workspace)
            .ok_or("Native account required.")?;
        if account.credited != 0
            || !account.holds.is_empty()
            || account
                .funding
                .as_ref()
                .is_some_and(|b| !b.funding.is_empty() || !b.lots.is_empty())
        {
            return Err("Shared activation requires an empty unencumbered native account.".into());
        }
        Ok(())
    }
    pub fn shared_client(&self, workspace: &str) -> Option<&Client> {
        self.shared_clients.get(workspace)
    }
    pub fn shared_workspaces(&self) -> Vec<String> {
        self.state
            .accounts
            .iter()
            .filter(|(_, a)| a.shared.is_some())
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn shared_mode(&self, workspace: &str) -> Option<&Mode> {
        self.state.accounts.get(workspace)?.shared.as_ref()
    }
    pub fn configure_shared(
        &mut self,
        workspace: &str,
        config: ClientConfig,
    ) -> Result<(), String> {
        let mode = self
            .shared_mode(workspace)
            .ok_or("Native shared custody is required.")?;
        if config.binding != mode.binding
            || config.origin != mode.origin
            || config.socket != mode.socket
        {
            return Err("Shared controller selection changed.".into());
        }
        self.shared_clients
            .insert(workspace.into(), Client { config });
        Ok(())
    }
    pub fn install_shared(&mut self, workspace: &str, mode: Mode) -> Result<bool, String> {
        self.apply(Mutation {
            workspace: workspace.into(),
            source: format!("shared-custody:{}", mode.binding),
            audit: mode.binding_digest.clone(),
            operation: Operation::SharedCustody { mode },
        })
    }
    pub(super) fn shared_guard(&self, mutation: &Mutation) -> Result<(), String> {
        let Operation::SharedProject {
            reference,
            operation,
        } = &mutation.operation
        else {
            return Ok(());
        };
        let mode = self
            .shared_mode(&mutation.workspace)
            .ok_or("Native shared custody required.")?;
        if !reference.mode.same_native(mode) {
            return Err("Original shared projection changed.".into());
        }
        let client = self
            .shared_clients
            .get(&mutation.workspace)
            .ok_or("The original controller is required; native spend is disabled.")?;
        let value = client
            .call(pay_ledger::shared::Operation::Observe {
                id: reference.intent.clone(),
            })
            .map_err(|_| "Canonical shared projection unavailable.")?;
        let outcome: Outcome =
            serde_json::from_value(value).map_err(|_| "Invalid canonical shared projection.")?;
        if outcome.intent.digest() != reference.digest
            || outcome.intent.binding.id != reference.mode.binding
            || outcome.intent.binding.digest() != reference.mode.binding_digest
        {
            return Err("Original canonical intent changed.".into());
        }
        match &**operation {
            Operation::Reserve {
                attempt,
                request_digest,
                price,
                maximum_usage,
            }
            | Operation::ReserveScoped {
                attempt,
                request_digest,
                price,
                maximum_usage,
                ..
            } => {
                if outcome.intent.native_attempt != *attempt
                    || outcome.intent.terms != *request_digest
                    || outcome.intent.maximum_units != price.quote(maximum_usage)?
                    || outcome.intent.quote
                        != format!(
                            "sha256:{:x}",
                            Sha256::digest(
                                serde_json::to_vec(&(price, maximum_usage))
                                    .map_err(|_| "Invalid original native price.")?
                            )
                        )
                {
                    return Err("Shared native reservation terms changed.".into());
                }
            }
            Operation::Settle { attempt, usage, .. } => {
                let hold = self
                    .hold(&mutation.workspace, attempt)
                    .ok_or("Original projected hold missing.")?;
                if outcome.state != "settled"
                    || outcome
                        .intent
                        .convert(hold.price.quote(usage)?)
                        .map_err(|_| "Unsupported original conversion.")?
                        != outcome.hold.charge_msat.unwrap_or(-1) as u64
                {
                    return Err("Known canonical settlement required.".into());
                }
            }
            Operation::Release { .. }
                if outcome.state == "settled" && outcome.hold.charge_msat == Some(0) => {}
            Operation::Unknown { .. }
                if matches!(outcome.state.as_str(), "sending" | "unknown") => {}
            _ => return Err("Unsupported shared projection effect.".into()),
        }
        Ok(())
    }
    pub fn shared_project(
        &mut self,
        workspace: &str,
        source: &str,
        audit: &str,
        reference: Reference,
        operation: Operation,
    ) -> Result<bool, String> {
        self.apply(Mutation {
            workspace: workspace.into(),
            source: source.into(),
            audit: audit.into(),
            operation: Operation::SharedProject {
                reference,
                operation: Box::new(operation),
            },
        })
    }
    pub fn shared_native_reservation(
        &self,
        workspace: &str,
        price: &Price,
        usage: &Usage,
    ) -> Result<u64, String> {
        let amount = price.quote(usage)?;
        let mode = self
            .shared_mode(workspace)
            .ok_or("Shared native mode required.")?;
        if mode.conversion.source
            != (receipts::funding_units::Unit::CurrencyMillionths {
                currency: price.currency.clone(),
            })
        {
            return Err("Explicit reviewed source currency required.".into());
        }
        Ok(amount)
    }
}
