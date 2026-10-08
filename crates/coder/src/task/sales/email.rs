//! Owner-configured email admission. Only the host retrieves a credential.
//! Preparation grants no send; the durable outbox owns effect authorization.
use super::*;
use coder_host::serve::keys::{AccountKeys, Secret};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};

pub mod smtp;

pub const CONFIG_SCHEMA: &str = "openagents.sales.email-config.v1";
pub const COMMAND_SCHEMA: &str = "openagents.sales.email-command.v1";
pub const MESSAGE_SCHEMA: &str = "openagents.sales.email-message.v1";
const MAX_CONFIGS: usize = 64;
const MAX_BODY: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Fixture,
    Smtp,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Validation {
    Unknown,
    Passed,
    Failed,
}
/// Owner-declared configuration evidence; it is not independently observed DNS.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidence {
    pub domain: String,
    pub spf: Validation,
    pub dkim: Validation,
    pub dmarc: Validation,
    pub tls: Validation,
    pub authentication: Validation,
    pub reference_sha256: String,
    pub expires_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub id: String,
    pub version: u64,
    pub provider: Provider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp: Option<smtp::Config>,
    pub sender: String,
    pub reply_to: String,
    pub company: String,
    pub human_responsible: String,
    pub postal_address: String,
    pub identity_reference_sha256: String,
    pub commercial_label: String,
    pub unsubscribe_url: String,
    pub unsubscribe_reference_sha256: String,
    pub unsubscribe_available_until: u64,
    /// An opaque provider account handle, never a token or a filesystem path.
    pub credential_account: String,
    /// The digest of the exact bounded provider credential bytes.
    pub credential_sha256: String,
    pub policy_sha256: String,
    pub templates: BTreeMap<String, String>,
    pub domain_evidence: DomainEvidence,
    pub expires_at: u64,
}
fn header(value: &str, bound: usize) -> Result<()> {
    text(value, bound)?;
    if value.chars().any(char::is_control) {
        return Err("email header contains control characters".into());
    }
    Ok(())
}
fn address(value: &str) -> Result<String> {
    if !value.is_ascii() || value.contains(['\r', '\n']) {
        return Err("email address is outside the supported boundary".into());
    }
    privacy::normalize(&format!("email:{value}")).map(|s| s[6..].into())
}
impl Config {
    fn validate(&self) -> Result<()> {
        if self.schema != CONFIG_SCHEMA
            || self.version == 0
            || self.expires_at == 0
            || self.templates.is_empty()
            || self.templates.len() > 32
        {
            return Err("email configuration schema or bound is unsupported".into());
        }
        if let Some(smtp) = &self.smtp {
            smtp.check()?;
            text(
                &format!(
                    "provider:email:{}:{}",
                    self.id,
                    smtp.server.to_ascii_lowercase()
                ),
                256,
            )?;
        }
        if self.provider == Provider::Fixture && self.smtp.is_some() {
            return Err("fixture mailbox cannot claim a live SMTP endpoint".into());
        }
        fn credential_in_metadata(value: &Value, sha: &str) -> bool {
            match value {
                Value::String(s) => digest(s.as_bytes()) == sha,
                Value::Array(rows) => rows.iter().any(|v| credential_in_metadata(v, sha)),
                Value::Object(rows) => rows
                    .iter()
                    .any(|(k, v)| digest(k.as_bytes()) == sha || credential_in_metadata(v, sha)),
                _ => false,
            }
        }
        let metadata =
            serde_json::to_value(self).map_err(|_| "email configuration serialization failed")?;
        if credential_in_metadata(&metadata, &self.credential_sha256) {
            return Err("email configuration refuses provider credentials in metadata".into());
        }
        id(&self.id)?;
        id(&self.human_responsible)?;
        for value in [
            &self.identity_reference_sha256,
            &self.unsubscribe_reference_sha256,
            &self.credential_sha256,
            &self.policy_sha256,
            &self.domain_evidence.reference_sha256,
        ] {
            token(value)?;
        }
        for (key, sha) in &self.templates {
            id(key)?;
            token(sha)?;
        }
        for value in [&self.company, &self.postal_address, &self.commercial_label] {
            header(value, 256)?;
        }
        if self.commercial_label != "Commercial advertisement" {
            return Err("email requires an explicit commercial advertisement label".into());
        }
        let sender = address(&self.sender)?;
        if sender != self.sender
            || address(&self.reply_to)? != self.reply_to
            || sender.split_once('@').map(|(_, d)| d) != Some(self.domain_evidence.domain.as_str())
        {
            return Err("email sender and domain identity disagree".into());
        }
        if !self.credential_account.starts_with("sales-mailbox:")
            || self.credential_account.len() > 128
            || !self
                .credential_account
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-:_".contains(&b))
        {
            return Err("email credential needs a restricted host account handle".into());
        }
        let url = reqwest::Url::parse(&self.unsubscribe_url)
            .map_err(|_| "email unsubscribe URL is invalid")?;
        if url.scheme() != "https"
            || url.host_str() != Some(self.domain_evidence.domain.as_str())
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || self.unsubscribe_url.len() > 512
        {
            return Err("email unsubscribe requires the declared HTTPS domain".into());
        }
        if self.unsubscribe_available_until < self.expires_at.saturating_add(30 * 86400) {
            return Err(
                "email unsubscribe evidence must cover thirty days after possible dispatch".into(),
            );
        }
        Ok(())
    }
    fn current(&self, now: u64) -> Result<()> {
        self.validate()?;
        let e = &self.domain_evidence;
        if self.expires_at <= now
            || e.expires_at <= now
            || [e.spf, e.dkim, e.dmarc, e.tls, e.authentication]
                .iter()
                .any(|v| *v != Validation::Passed)
        {
            return Err("email domain, TLS, or authentication evidence is unavailable".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigRecord {
    pub config: Config,
    pub sha256: String,
    pub recorded_by: String,
    pub recorded_at: u64,
    pub revoked_at: Option<u64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    pub revision: u64,
    pub current: Option<String>,
    pub configs: BTreeMap<String, ConfigRecord>,
    commands: BTreeMap<String, (String, String, u64)>,
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        if self.configs.len() > MAX_CONFIGS || self.commands.len() > 256 {
            return Err("email configuration history exceeds its bound".into());
        }
        for (sha, record) in &self.configs {
            record.config.validate()?;
            if sha != &record.sha256
                || digest(
                    &serde_json::to_vec(&record.config)
                        .map_err(|_| "email configuration serialization failed")?,
                ) != *sha
            {
                return Err("email configuration identity disagrees".into());
            }
        }
        if self
            .current
            .as_ref()
            .is_some_and(|sha| !self.configs.contains_key(sha))
        {
            return Err("current email configuration is unavailable".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub schema: String,
    pub id: String,
    pub expected_revision: u64,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Configure {
        config: Config,
    },
    Revoke {
        config_sha256: String,
        reference_sha256: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Sender {
    Human {
        principal: String,
    },
    Agent {
        anchor: agents::Anchor,
        assignment: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub schema: String,
    pub lead: String,
    pub expected_lead_revision: u64,
    pub config_sha256: String,
    pub policy_sha256: String,
    pub template: agents::Artifact,
    pub sender: Sender,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub subject_review_sha256: String,
    pub expires_at: u64,
}
/// An admitted message contains no credential and cannot dispatch itself.
pub struct Prepared {
    pub(super) message: Message,
    pub(super) rendered: String,
    pub(super) sha256: String,
    pub(super) scope_sha256: String,
}
impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreparedEmail(..)")
    }
}
impl Prepared {
    #[must_use]
    pub fn view(&self) -> Value {
        json!({"message_sha256":self.sha256,"config_sha256":self.message.config_sha256,"expires_at":self.message.expires_at,"outbound_authority":false,"live_sender_qualified":false})
    }
}
/// Bounded provider authentication material. It cannot be serialized or printed.
pub struct MailboxSecret(Vec<u8>);
impl MailboxSecret {
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.is_empty()
            || bytes.len() > 2048
            || bytes.contains(&0)
            || bytes.contains(&b'\r')
            || bytes.contains(&b'\n')
            || std::str::from_utf8(&bytes).is_err()
        {
            return Err("mailbox credential format is unavailable".into());
        }
        Ok(Self(bytes))
    }
    pub(super) fn expose(&self) -> &[u8] {
        &self.0
    }
}
impl std::fmt::Debug for MailboxSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MailboxSecret(..)")
    }
}
impl Drop for MailboxSecret {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
/// The host injects a protected provider credential source, never an agent tool.
pub trait MailboxCredentials: Send + Sync {
    fn load(&self, account: &str) -> Result<MailboxSecret>;
    /// Re-read current protected custody after preparation; no cached secret suffices.
    fn recheck(&self, account: &str, expected_sha256: &str) -> Result<()> {
        let current = self
            .load(account)
            .map_err(|_| "host mailbox credential is unavailable")?;
        if digest(current.expose()) != expected_sha256 {
            return Err("host mailbox credential is revoked or changed".into());
        }
        Ok(())
    }
}
/// Private file fallback for an explicitly selected mailbox account. This never
/// reads a keychain, creates a key, or accepts a credential through environment.
pub struct FileAccount {
    account: String,
    path: PathBuf,
}
impl FileAccount {
    pub fn new(account: &str, path: &Path) -> Result<Self> {
        if !account.starts_with("sales-mailbox:") {
            return Err("email account handle is unsupported".into());
        }
        Ok(Self {
            account: account.into(),
            path: path.into(),
        })
    }
}
impl MailboxCredentials for FileAccount {
    fn load(&self, account: &str) -> Result<MailboxSecret> {
        if account != self.account {
            return Err("host mailbox credential is unavailable".into());
        }
        let bytes = privacy::read_command(&self.path)
            .map_err(|_| "host mailbox credential is unavailable")?;
        MailboxSecret::new(bytes).map_err(|_| "host mailbox credential is unavailable".into())
    }
}
/// Arbitrary provider credentials encrypted under the existing host account key.
/// AccountKeys retains the 32-byte authority key; it is not the mailbox password.
pub struct SealedAccount {
    account: String,
    path: PathBuf,
    keys: std::sync::Arc<dyn AccountKeys>,
}
fn vault_key(master: &Secret) -> [u8; 32] {
    let mut bytes = b"openagents.sales.mailbox-vault.v1:".to_vec();
    bytes.extend_from_slice(master.expose());
    let hash = Sha256::digest(&bytes);
    let mut key = [0; 32];
    key.copy_from_slice(&hash);
    key
}
impl SealedAccount {
    pub fn new(account: &str, path: &Path, keys: std::sync::Arc<dyn AccountKeys>) -> Result<Self> {
        if !account.starts_with("sales-mailbox:") {
            return Err("email account handle is unsupported".into());
        }
        Ok(Self {
            account: account.into(),
            path: path.into(),
            keys,
        })
    }
}
impl MailboxCredentials for SealedAccount {
    fn load(&self, account: &str) -> Result<MailboxSecret> {
        if account != self.account {
            return Err("host mailbox credential is unavailable".into());
        }
        let master = self
            .keys
            .load_account(account)
            .map_err(|_| "host mailbox authority key is unavailable")?
            .ok_or("host mailbox authority key is unavailable")?;
        let bytes = privacy::read_command(&self.path)
            .map_err(|_| "sealed mailbox credential is unavailable")?;
        let payload =
            std::str::from_utf8(&bytes).map_err(|_| "sealed mailbox credential is unavailable")?;
        let plaintext = nostr::nip44::decrypt(payload, &vault_key(&master))
            .map_err(|_| "sealed mailbox credential is unavailable")?;
        let value: Value = serde_json::from_str(&plaintext)
            .map_err(|_| "sealed mailbox credential is unavailable")?;
        if value["account"].as_str() != Some(account) {
            return Err("sealed mailbox account binding changed".into());
        }
        let credential = value["credential"]
            .as_str()
            .ok_or("sealed mailbox credential is unavailable")?;
        MailboxSecret::new(credential.as_bytes().to_vec())
    }
}
impl Store {
    /// Seals an exact configured provider credential outside the host root.
    /// The owner supplies an existing authority key; this creates no keychain item.
    pub fn seal_email_credential(
        &mut self,
        access: &Access,
        keys: &dyn AccountKeys,
        account: &str,
        secret: &MailboxSecret,
        path: &Path,
    ) -> Result<String> {
        self.refresh()?;
        self.admin(access)?;
        let sha = self
            .state
            .email
            .current
            .clone()
            .ok_or("email configuration is unavailable")?;
        let config = self.email_config(&sha, (self.clock)())?;
        if config.credential_account != account
            || config.credential_sha256 != digest(secret.expose())
        {
            return Err(
                "mailbox sealing requires the exact configured account and credential".into(),
            );
        }
        self.external_file(path)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(|_| "mailbox vault parent is unavailable")?;
        if parent.starts_with(self.dir.parent().ok_or("host root is unavailable")?) {
            return Err("mailbox vault must remain outside the host agent root".into());
        }
        let held = agents::native::directory(&parent)?;
        let master = keys
            .load_account(account)
            .map_err(|_| "host mailbox authority key is unavailable")?
            .ok_or("host mailbox authority key is unavailable")?;
        let credential = std::str::from_utf8(secret.expose())
            .map_err(|_| "mailbox credential is unavailable")?;
        let plaintext = serde_json::to_string(&json!({"account":account,"credential":credential}))
            .map_err(|_| "mailbox sealing failed")?;
        let payload = nostr::nip44::encrypt(
            &plaintext,
            &vault_key(&master),
            secp256k1::rand::random::<[u8; 32]>(),
        )
        .map_err(|_| "mailbox sealing failed")?;
        let target = parent.join(
            path.file_name()
                .ok_or("mailbox vault filename is unavailable")?,
        );
        let mut next = self.state.clone();
        privacy::remember_mailbox_credential(&mut next, credential)?;
        self.email_config(&sha, (self.clock)())?;
        self.admin(access)?;
        self.persist(next)?;
        agents::native::same_directory(&parent, &held)?;
        let mut file = super::super::private_open(&target, true, true)
            .map_err(|_| "mailbox vault create refused")?;
        file.write_all(payload.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|_| "mailbox vault write failed")?;
        super::super::verify_same_file(&target, &file)
            .map_err(|_| "mailbox vault custody changed")?;
        agents::native::same_directory(&parent, &held)?;
        super::super::sync_directory(&parent).map_err(|_| "mailbox vault did not persist")?;
        Ok(digest(payload.as_bytes()))
    }
    pub fn email_view(&mut self, access: &Access) -> Result<Value> {
        self.refresh()?;
        self.admin(access)?;
        Ok(
            json!({"revision":self.state.email.revision,"current":self.state.email.current,"configurations":self.state.email.configs.values().collect::<Vec<_>>(),"outbound_authority":false,"live_sender_qualified":false}),
        )
    }
    pub fn apply_email(&mut self, access: &Access, bytes: &[u8]) -> Result<u64> {
        self.refresh()?;
        self.admin(access)?;
        let command: Command = agents::parse(bytes)?;
        if command.schema != COMMAND_SCHEMA {
            return Err("email command schema is unsupported".into());
        }
        id(&command.id)?;
        privacy::check_credentials(
            &self.state,
            std::str::from_utf8(bytes).map_err(|_| "email command is not UTF-8")?,
        )?;
        let hash = digest(bytes);
        if let Some((actor, input, revision)) = self.state.email.commands.get(&command.id) {
            return if actor == access.principal() && input == &hash {
                Ok(*revision)
            } else {
                Err("email command idempotency conflict".into())
            };
        }
        if command.expected_revision != self.state.email.revision {
            return Err("email configuration revision conflict".into());
        }
        let now = (self.clock)();
        let mut next = self.state.clone();
        match command.operation {
            Operation::Configure { config } => {
                config.current(now)?;
                if config.human_responsible != access.principal() {
                    return Err("email configuration requires the responsible owner".into());
                }
                self.email_policy(&config.policy_sha256, now)?;
                let previous = next
                    .email
                    .configs
                    .values()
                    .filter(|r| r.config.id == config.id)
                    .map(|r| r.config.version)
                    .max()
                    .unwrap_or(0);
                if config.version
                    != previous
                        .checked_add(1)
                        .ok_or("email configuration version overflow")?
                {
                    return Err("email configuration version must advance once".into());
                }
                let sha256 = digest(
                    &serde_json::to_vec(&config)
                        .map_err(|_| "email configuration serialization failed")?,
                );
                privacy::remember_credential(&mut next, &config.credential_sha256)?;
                next.email.current = Some(sha256.clone());
                next.email.configs.insert(
                    sha256.clone(),
                    ConfigRecord {
                        config,
                        sha256,
                        recorded_by: access.principal().into(),
                        recorded_at: now,
                        revoked_at: None,
                    },
                );
            }
            Operation::Revoke {
                config_sha256,
                reference_sha256,
            } => {
                token(&reference_sha256)?;
                let config = next
                    .email
                    .configs
                    .get_mut(&config_sha256)
                    .ok_or("email configuration is unavailable")?;
                config.revoked_at = Some(now);
                if next.email.current.as_ref() == Some(&config_sha256) {
                    next.email.current = None;
                }
            }
        }
        next.email.revision = next
            .email
            .revision
            .checked_add(1)
            .ok_or("email revision overflow")?;
        next.email.commands.insert(
            command.id,
            (access.principal().into(), hash, next.email.revision),
        );
        next.email.check()?;
        let revision = next.email.revision;
        self.admin(access)?;
        self.persist(next)?;
        Ok(revision)
    }
    pub(super) fn email_policy(&self, sha: &str, now: u64) -> Result<&agents::Policy> {
        let record = self
            .state
            .agents
            .policies
            .get(sha)
            .ok_or("email policy is unavailable")?;
        let p = &record.policy;
        if record.revoked_at.is_some()
            || p.expires_at <= now
            || self.state.agents.current.get(&p.id) != Some(&record.sha256)
            || !p.channels.iter().any(|c| c == "email")
            || p.jurisdictions != ["US"]
        {
            return Err("email policy is expired, revoked, or outside its scope".into());
        }
        Ok(p)
    }
    pub(super) fn email_config(&self, sha: &str, now: u64) -> Result<&Config> {
        let record = self
            .state
            .email
            .configs
            .get(sha)
            .ok_or("email configuration is unavailable")?;
        if record.revoked_at.is_some() || self.state.email.current.as_deref() != Some(sha) {
            return Err("email configuration is revoked or superseded".into());
        }
        record.config.current(now)?;
        Ok(&record.config)
    }
    pub fn prepare_email(
        &mut self,
        access: &Access,
        message: Message,
        keys: &dyn MailboxCredentials,
    ) -> Result<Prepared> {
        self.refresh()?;
        self.admin(access)?;
        let now = (self.clock)();
        let config = self.email_config(&message.config_sha256, now)?.clone();
        let policy = self.email_policy(&message.policy_sha256, now)?.clone();
        if message.schema != MESSAGE_SCHEMA
            || message.policy_sha256 != config.policy_sha256
            || message.expires_at <= now
            || message.expires_at > config.expires_at
            || message.expires_at > policy.expires_at
        {
            return Err("email message versions or expiry disagree".into());
        }
        let lead = self
            .state
            .leads
            .get(&message.lead)
            .ok_or("email contact is unavailable")?
            .clone();
        self.readable(access, &lead)?;
        self.contact_admitted(&lead, "email")?;
        if lead.revision != message.expected_lead_revision
            || address(&message.recipient)? != message.recipient
            || privacy::normalize(&lead.contact)? != format!("email:{}", message.recipient)
            || message.expires_at > lead.details.permission.expires_at
            || message.expires_at > lead.details.data.retain_until
        {
            return Err("email contact or permission pins changed".into());
        }
        let recipient = format!("provider:email:{}", config.id);
        if !lead.details.data.recipients.contains(&recipient)
            || !policy.data_recipients.contains(&recipient)
        {
            return Err("email provider is outside the admitted recipient boundary".into());
        }
        if let Some(smtp) = &config.smtp {
            let endpoint = format!(
                "provider:email:{}:{}",
                config.id,
                smtp.server.to_ascii_lowercase()
            );
            if !lead.details.data.recipients.contains(&endpoint)
                || !policy.data_recipients.contains(&endpoint)
            {
                return Err(
                    "selected SMTP endpoint is outside the original recipient boundary".into(),
                );
            }
        }
        if config.templates.get(&message.template.reference) != Some(&message.template.sha256) {
            return Err("email template version is unavailable".into());
        }
        token(&message.subject_review_sha256)?;
        token(&message.template.sha256)?;
        header(&message.subject, 256)?;
        text(&message.body, MAX_BODY)?;
        privacy::check_credentials(&self.state, &message.body)?;
        privacy::check_credentials(&self.state, &message.subject)?;
        let mut native_guard = None;
        let disclosure = match &message.sender {
            Sender::Human { principal }
                if principal == access.principal() && principal == &config.human_responsible =>
            {
                format!("Human sender: {principal}, {}", config.company)
            }
            Sender::Human { .. } => {
                return Err("email human sender is not the responsible owner".into());
            }
            Sender::Agent { anchor, assignment } => {
                let grant = lead
                    .agent_records
                    .assignments
                    .get(assignment)
                    .ok_or("email agent assignment is unavailable")?;
                let native = agents::native::Native::read(
                    self.dir.parent().ok_or("email host root is unavailable")?,
                    &anchor.name,
                    now,
                    self.native_keys.clone(),
                )?;
                if !grant.active
                    || grant.expires_at <= now
                    || grant.anchor != *anchor
                    || native.anchor != *anchor
                    || grant.policy_sha256 != message.policy_sha256
                    || grant.scope_sha256 != agents::scope(&lead)?
                    || !policy.allowed_agents.contains(&anchor.pubkey)
                {
                    return Err("email agent identity or grant changed".into());
                }
                native.recheck()?;
                native_guard = Some(native);
                format!(
                    "AI agent sender: {}; responsible human: {}, {}",
                    anchor.name, config.human_responsible, config.company
                )
            }
        };
        let secret = keys
            .load(&config.credential_account)
            .map_err(|_| "host mailbox credential is unavailable")?;
        if digest(secret.expose()) != config.credential_sha256 {
            return Err("host mailbox credential is revoked or changed".into());
        }
        let mut next = self.state.clone();
        privacy::remember_mailbox_credential(
            &mut next,
            std::str::from_utf8(secret.expose())
                .map_err(|_| "host mailbox credential is unavailable")?,
        )?;
        privacy::check_credentials(
            &next,
            &serde_json::to_string(&config)
                .map_err(|_| "email configuration serialization failed")?,
        )?;
        self.persist(next)?;
        let rendered = format!(
            "From: {}\nReply-To: {}\nTo: {}\nSubject: {}\n\n{}\n\n{}\n{}\nPostal address: {}\nStop all marketing email: {}\n",
            config.sender,
            config.reply_to,
            message.recipient,
            message.subject,
            message.body,
            disclosure,
            config.commercial_label,
            config.postal_address,
            config.unsubscribe_url
        );
        privacy::check_credentials(&self.state, &rendered)?;
        let sha256 = digest(
            &serde_json::to_vec(&message).map_err(|_| "email message serialization failed")?,
        );
        let scope_sha256 = agents::scope(&lead)?;
        keys.recheck(&config.credential_account, &config.credential_sha256)?;
        self.admin(access)?;
        let current_now = (self.clock)();
        self.email_config(&message.config_sha256, current_now)?;
        self.email_policy(&message.policy_sha256, current_now)?;
        if message.expires_at <= current_now {
            return Err("email message expired before preparation completed".into());
        }
        if let Some(native) = native_guard {
            native.recheck()?;
        }
        self.contact_admitted(&lead, "email")?;
        Ok(Prepared {
            message,
            rendered,
            sha256,
            scope_sha256,
        })
    }
    pub fn email_provider_evidence(
        &mut self,
        access: &Access,
        bytes: &[u8],
        message_sha256: &str,
    ) -> Result<ProviderEvidence> {
        self.refresh()?;
        self.admin(access)?;
        privacy::check_credentials(
            &self.state,
            std::str::from_utf8(bytes).map_err(|_| "email provider evidence is not UTF-8")?,
        )?;
        provider_evidence(bytes, message_sha256)
    }
    /// Rechecks the current host boundary before a synthetic adapter observation.
    /// This method neither reaches a mailbox nor authorizes a live sender.
    pub fn observe_email_fixture(
        &mut self,
        access: &Access,
        prepared: Prepared,
        keys: &dyn MailboxCredentials,
        transport: &mut FakeTransport,
        cancel: &AtomicBool,
    ) -> Result<ProviderEvidence> {
        let current = self.prepare_email(access, prepared.message.clone(), keys)?;
        if current.sha256 != prepared.sha256
            || current.scope_sha256 != prepared.scope_sha256
            || self
                .email_config(&current.message.config_sha256, (self.clock)())?
                .provider
                != Provider::Fixture
        {
            return Err("email fixture handoff scope changed or requires a live adapter".into());
        }
        privacy::check_credentials(
            &self.state,
            std::str::from_utf8(&transport.result)
                .map_err(|_| "email fixture evidence is not UTF-8")?,
        )?;
        transport.observe(&current, cancel)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Accepted,
    Delivered,
    Failed,
    HardBounce,
    AuthenticationFailed,
    Unknown,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEvidence {
    pub message_sha256: String,
    pub provider_id: String,
    pub reference_sha256: String,
    pub delivery: Delivery,
    pub tls: Validation,
    pub authentication: Validation,
}
/// Bounded provider evidence retains acceptance separately from delivery.
fn provider_evidence(bytes: &[u8], expected_message: &str) -> Result<ProviderEvidence> {
    if bytes.len() > 4096 {
        return Err("email provider evidence exceeds its bound".into());
    }
    let value: ProviderEvidence =
        serde_json::from_slice(bytes).map_err(|_| "email provider evidence is malformed")?;
    token(&value.reference_sha256)?;
    if value.message_sha256 != expected_message
        || value.provider_id.is_empty()
        || value.provider_id.len() > 128
        || !value
            .provider_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("email provider evidence identity disagrees".into());
    }
    if matches!(value.delivery, Delivery::Accepted | Delivery::Delivered)
        && (value.tls != Validation::Passed || value.authentication != Validation::Passed)
    {
        return Err("email provider success lacks transport authentication evidence".into());
    }
    Ok(ProviderEvidence {
        provider_id: format!("provider-{}", digest(value.provider_id.as_bytes())),
        reference_sha256: digest(bytes),
        ..value
    })
}
/// A synthetic adapter never represents a live mailbox or DNS qualification.
pub struct FakeTransport {
    pub result: Vec<u8>,
    pub calls: usize,
}
impl FakeTransport {
    fn observe(&mut self, message: &Prepared, cancel: &AtomicBool) -> Result<ProviderEvidence> {
        if cancel.load(Ordering::SeqCst) {
            return Ok(ProviderEvidence {
                message_sha256: message.sha256.clone(),
                provider_id: "fixture-cancelled".into(),
                reference_sha256: digest(b"cancelled"),
                delivery: Delivery::Cancelled,
                tls: Validation::Unknown,
                authentication: Validation::Unknown,
            });
        }
        self.calls += 1;
        if self.result.is_empty() {
            return Ok(ProviderEvidence {
                message_sha256: message.sha256.clone(),
                provider_id: "fixture-unknown".into(),
                reference_sha256: digest(b"no provider telemetry"),
                delivery: Delivery::Unknown,
                tls: Validation::Unknown,
                authentication: Validation::Unknown,
            });
        }
        provider_evidence(&self.result, &message.sha256)
    }
}
