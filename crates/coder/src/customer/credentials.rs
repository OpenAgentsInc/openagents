//! Private single-attempt credential effects; uncertainty never authorizes replay.
use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};

const MAX_OPERATIONS: usize = 128;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CredentialAction {
    SignIn {
        output_alias: String,
    },
    Recover {
        workspace: String,
        output_alias: String,
    },
    Rotate {
        workspace: String,
        key: String,
        output_alias: String,
    },
    Revoke {
        workspace: String,
        key: String,
    },
    SignOut,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CredentialCommand {
    pub id: String,
    pub origin: String,
    pub account: String,
    /// Recovery authenticates with its once-issued token, not an old account key.
    pub credential_alias: Option<String>,
    pub action: CredentialAction,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialStatus {
    Pending,
    Applied,
    Unknown,
}
#[derive(Clone, Debug, Serialize)]
pub struct CredentialView {
    pub command: CredentialCommand,
    pub status: CredentialStatus,
    pub credential_available: bool,
    pub selected: bool,
    pub limitation: Option<&'static str>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Operation {
    command: CredentialCommand,
    /// A private fingerprint, never emitted in public operation views.
    authority_digest: String,
    pub(super) status: CredentialStatus,
}
impl Operation {
    pub(super) fn command_output_alias(&self) -> Option<&str> {
        self.command.output_alias()
    }
    pub(super) fn command_origin(&self) -> &str {
        &self.command.origin
    }
    pub(super) fn command_account(&self) -> &str {
        &self.command.account
    }
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
impl CredentialCommand {
    fn validate(&self) -> Result<()> {
        if !alias(&self.id) || origin(&self.origin)? != self.origin || !identifier(&self.account) {
            return Err("Invalid credential operation identity.".into());
        }
        let recovering = matches!(self.action, CredentialAction::Recover { .. });
        if recovering != self.credential_alias.is_none()
            || self.credential_alias.as_ref().is_some_and(|v| !alias(v))
        {
            return Err("Use an account credential alias, or a separate recovery token.".into());
        }
        if self.output_alias().is_some_and(|v| !alias(v)) {
            return Err("Invalid issued credential alias.".into());
        }
        match &self.action {
            CredentialAction::Recover { workspace, .. } => {
                if !identifier(workspace) {
                    return Err("Invalid recovery workspace.".into());
                }
            }
            CredentialAction::Rotate { workspace, key, .. }
            | CredentialAction::Revoke { workspace, key } => {
                if !identifier(workspace) || !identifier(key) {
                    return Err("Invalid credential workspace or key.".into());
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn output_alias(&self) -> Option<&str> {
        match &self.action {
            CredentialAction::SignIn { output_alias }
            | CredentialAction::Recover { output_alias, .. }
            | CredentialAction::Rotate { output_alias, .. } => Some(output_alias),
            _ => None,
        }
    }
    fn effect_scope(&self) -> Value {
        match &self.action {
            CredentialAction::SignIn { .. } => json!(["sign-in"]),
            CredentialAction::Recover { .. } => json!(["recover"]),
            CredentialAction::Rotate { workspace, key, .. } => json!(["rotate", workspace, key]),
            CredentialAction::Revoke { workspace, key } => json!(["revoke", workspace, key]),
            CredentialAction::SignOut => json!(["sign-out"]),
        }
    }
}
pub(super) fn check_operations(operations: &BTreeMap<String, Operation>) -> Result<()> {
    if operations.len() > MAX_OPERATIONS {
        return Err("Credential operation history exceeds its bound.".into());
    }
    for (id, operation) in operations {
        operation.command.validate()?;
        let hash = operation
            .authority_digest
            .strip_prefix("sha256:")
            .unwrap_or("");
        if id != &operation.command.id
            || hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("Invalid retained credential operation.".into());
        }
    }
    Ok(())
}
impl Store {
    fn retain_issued(&mut self, id: &str, key: &jev::ApiKey) -> Result<()> {
        if !alias(id) || !credential_valid(key.expose()) {
            return Err("Invalid once-issued credential.".into());
        }
        let dir = self.dir.join("issued");
        task::prepare_directory(&dir).map_err(|_| "Issued credential vault is unsafe.")?;
        let mut file = task::private_open(&dir.join(id), true, true)
            .map_err(|_| "Issued credential cannot be retained privately.")?;
        file.write_all(key.expose().as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|_| "Issued credential custody is uncertain.")?;
        task::sync_directory(&dir).map_err(|_| "Issued credential vault sync failed.".into())
    }
    fn retained_issued(&self, id: &str) -> Result<jev::ApiKey> {
        if !alias(id) {
            return Err("Invalid credential operation ID.".into());
        }
        let bytes = Self::private_input(&self.dir.join("issued").join(id), 4096)?;
        let text = String::from_utf8(bytes).map_err(|_| "Invalid retained issued credential.")?;
        if !credential_valid(&text) {
            return Err("Invalid retained issued credential.".into());
        }
        Ok(jev::ApiKey::new(text))
    }
    /// Read explicit private inputs without following links or echoing content.
    pub fn private_input(path: &Path, maximum: usize) -> Result<Vec<u8>> {
        if maximum == 0 || maximum > MAX_STATE {
            return Err("Invalid private input bound.".into());
        }
        let mut bytes = Vec::new();
        task::private_open(path, false, false)
            .map_err(|_| "Private customer input is unavailable or unsafe.")?
            .take(maximum as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Private customer input read failed.")?;
        if bytes.is_empty() || bytes.len() > maximum {
            return Err("Private customer input exceeds its bound.".into());
        }
        Ok(bytes)
    }
    fn credential_view(&self, operation: &Operation) -> CredentialView {
        CredentialView {
            command: operation.command.clone(), status: operation.status,
            credential_available: operation.command.output_alias().is_some_and(|name| self.credential(name).is_ok()),
            selected: self.book.selected.as_ref().is_some_and(|selected|
                selected.origin == operation.command.origin
                    && selected.context.account == operation.command.account
                    && operation.command.output_alias() == Some(selected.credential_alias.as_str())),
            limitation: (operation.status != CredentialStatus::Applied).then_some(
                "The credential effect is unresolved. Do not replay it; inspect any retained credential and restore account access explicitly."),
        }
    }
    pub fn credential_history(&self) -> Vec<CredentialView> {
        let Some(selected) = &self.book.selected else {
            return vec![];
        };
        self.book
            .credential_operations
            .values()
            .filter(|op| {
                op.command.origin == selected.origin
                    && op.command.account == selected.context.account
            })
            .map(|op| self.credential_view(op))
            .collect()
    }
    /// Validate a retained once-issued credential after a crash, without
    /// claiming that the original effect was acknowledged or selecting it.
    pub async fn inspect_credential(&mut self, id: &str) -> Result<Value> {
        let operation = self
            .book
            .credential_operations
            .get(id)
            .cloned()
            .ok_or("Credential operation is unavailable.")?;
        let name = operation
            .command
            .output_alias()
            .ok_or("This operation issues no credential.")?;
        let candidate = self.retained_issued(id)?;
        let client = self.client_with_key(&operation.command.origin, candidate.clone())?;
        let details = client
            .account()
            .details()
            .await
            .map_err(|_| "The retained credential's current account is unavailable.")?;
        if details.account.id != operation.command.account {
            return Err("Retained credential account differs from the original operation.".into());
        }
        if let CredentialAction::Recover { workspace, .. }
        | CredentialAction::Rotate { workspace, .. } = &operation.command.action
        {
            let member = client
                .account()
                .workspace(workspace)
                .await
                .map_err(|_| "Retained credential workspace is unavailable.")?;
            if member.workspace.id != *workspace {
                return Err(
                    "Retained credential workspace differs from the original operation.".into(),
                );
            }
        }
        self.import_credential(name, &candidate)?;
        Ok(
            json!({"operation": self.credential_view(&operation), "account": details.account.id,
            "current_authentication_verified": true, "historical_effect_acknowledged": operation.status == CredentialStatus::Applied}),
        )
    }
    /// Persist intent before the first remote mutation. Exact command retries
    /// return the retained result, and an unknown effect blocks renamed retries.
    pub async fn change_credential(
        &mut self,
        command: CredentialCommand,
        recovery: Option<jev::ApiKey>,
    ) -> Result<CredentialView> {
        command.validate()?;
        let key = if matches!(command.action, CredentialAction::Recover { .. }) {
            let key = recovery.ok_or("A private recovery token is required.")?;
            if !key.expose().starts_with("rcv_")
                || key.expose().len() <= 4
                || key.expose().len() > 4096
                || key
                    .expose()
                    .bytes()
                    .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
            {
                return Err("Invalid private recovery token.".into());
            }
            key
        } else {
            if recovery.is_some() {
                return Err("Recovery material is only allowed for recovery.".into());
            }
            self.credential(command.credential_alias.as_deref().unwrap())?
        };
        let authority_digest = format!("sha256:{:x}", Sha256::digest(key.expose().as_bytes()));
        if let Some(old) = self.book.credential_operations.get(&command.id) {
            if old.command != command || old.authority_digest != authority_digest {
                return Err(
                    "Credential operation identity was already used for different intent.".into(),
                );
            }
            return Ok(self.credential_view(old));
        }
        if self.book.credential_operations.len() >= MAX_OPERATIONS {
            return Err("Credential operation history is full.".into());
        }
        if self.book.credential_operations.values().any(|old| {
            (old.status != CredentialStatus::Applied
                || matches!(old.command.action, CredentialAction::Recover { .. }))
                && old.command.origin == command.origin
                && old.authority_digest == authority_digest
                && old.command.effect_scope() == command.effect_scope()
        }) {
            return Err("An earlier credential effect is unresolved; changing its ID or output alias cannot authorize replay.".into());
        }
        if let Some(name) = command.output_alias() {
            if task::regular_or_absent(&self.dir.join("credentials").join(name))
                .map_err(|_| "Unsafe issued credential destination.")?
            {
                return Err("Use a new immutable alias for the issued credential.".into());
            }
        }
        let client = self.client_with_key(&command.origin, key.clone())?;
        if !matches!(command.action, CredentialAction::Recover { .. }) {
            let details = client
                .account()
                .details()
                .await
                .map_err(|_| "Current account authentication is unavailable.")?;
            if details.account.id != command.account {
                return Err(
                    "Authenticated account differs from the reviewed credential operation.".into(),
                );
            }
        }
        let mut next = self.book.clone();
        next.credential_operations.insert(
            command.id.clone(),
            Operation {
                command: command.clone(),
                authority_digest,
                status: CredentialStatus::Pending,
            },
        );
        self.persist(next)?;
        let result = self.credential_effect(&command, &client, &key).await;
        let mut next = self.book.clone();
        let operation = next.credential_operations.get_mut(&command.id).unwrap();
        operation.status = if result.is_ok() {
            CredentialStatus::Applied
        } else {
            CredentialStatus::Unknown
        };
        self.persist(next)?;
        Ok(self.credential_view(&self.book.credential_operations[&command.id]))
    }
    async fn credential_effect(
        &mut self,
        command: &CredentialCommand,
        client: &jev::Client,
        recovery: &jev::ApiKey,
    ) -> Result<()> {
        let issued = match &command.action {
            CredentialAction::SignIn { .. } => {
                let grant = client
                    .account()
                    .sign_in()
                    .await
                    .map_err(|_| "Sign-in outcome is unknown.")?;
                self.retain_issued(&command.id, &grant.token)?;
                if grant.session.account.as_deref() != Some(&command.account)
                    || grant.session.kind != "account"
                {
                    return Err("Issued session does not match the reviewed account.".into());
                }
                Some(grant.token)
            }
            CredentialAction::Recover { workspace, .. }
            | CredentialAction::Rotate { workspace, .. } => {
                let grant = match &command.action {
                    CredentialAction::Recover { .. } => client.account().recover(recovery).await,
                    CredentialAction::Rotate { key, .. } => {
                        client.account().rotate_key(workspace, key).await
                    }
                    _ => unreachable!(),
                }
                .map_err(|_| "Credential issuance outcome is unknown.")?;
                self.retain_issued(&command.id, &grant.token)?;
                // Rotation's wire response omits account; authenticate the
                // actual issued key before accepting its commercial identity.
                if grant
                    .account
                    .as_ref()
                    .is_some_and(|account| account != &command.account)
                {
                    return Err("Issued key belongs to another account.".into());
                }
                let fresh = self.client_with_key(&command.origin, grant.token.clone())?;
                let details = fresh
                    .account()
                    .details()
                    .await
                    .map_err(|_| "Issued key authentication is unavailable.")?;
                let member = fresh
                    .account()
                    .workspace(workspace)
                    .await
                    .map_err(|_| "Issued key workspace is unavailable.")?;
                if details.account.id != command.account
                    || member.workspace.id != *workspace
                    || member.workspace.tenant != grant.key.tenant
                {
                    return Err(
                        "Issued key account or workspace differs from the reviewed intent.".into(),
                    );
                }
                Some(grant.token)
            }
            CredentialAction::Revoke { workspace, key } => {
                client
                    .account()
                    .revoke_key(workspace, key)
                    .await
                    .map_err(|_| "Revocation outcome is unknown.")?;
                None
            }
            CredentialAction::SignOut => {
                let session = client
                    .account()
                    .session()
                    .await
                    .map_err(|_| "Current session is unavailable.")?;
                if session.session.account.as_deref() != Some(&command.account) {
                    return Err("Session account differs from the reviewed intent.".into());
                }
                client
                    .account()
                    .sign_out()
                    .await
                    .map_err(|_| "Sign-out outcome is unknown.")?;
                None
            }
        };
        if let Some(key) = issued {
            self.import_credential(command.output_alias().unwrap(), &key)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
