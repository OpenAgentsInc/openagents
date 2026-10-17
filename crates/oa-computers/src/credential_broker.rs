//! Scoped reference control and live authorization for the separate host broker.
//! User session keys stay in bounded broker memory and expire with their grant.
use crate::{Db, Store, store::Desired};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use computer_credentials::{
    self as credentials, Binding, Clock, Current, Failure, Owner, Secret, Source,
    State as BindingState, SystemClock, Workspace, gateway,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;
use zeroize::Zeroizing;

const MAX_REFERENCES: usize = 256;
#[derive(Clone)]
pub struct Repository {
    db: Db,
}
impl Repository {
    pub async fn existing(db: Db) -> Result<Self, Failure> {
        db.client()
            .await
            .map_err(|_| Failure::Unavailable)?
            .query("SELECT id FROM computer_credentials.bindings LIMIT 0", &[])
            .await
            .map_err(|_| Failure::Unavailable)?;
        Ok(Self { db })
    }
    pub async fn new(db: Db) -> Result<Self, Failure> {
        db.client()
            .await
            .map_err(|_| Failure::Unavailable)?
            .batch_execute(include_str!("../migrations/0014_credential_references.sql"))
            .await
            .map_err(|_| Failure::Unavailable)?;
        Ok(Self { db })
    }
    async fn prepare(
        &self,
        request_key: &str,
        fingerprint: &str,
        b: &Binding,
    ) -> Result<Binding, Failure> {
        let json = serde_json::to_value(b).map_err(|_| Failure::Invalid)?;
        let client = self.db.client().await.map_err(|_| Failure::Unavailable)?;
        client.execute("INSERT INTO computer_credentials.bindings(id,owner,computer,request_key,fingerprint,binding) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(owner,computer,request_key) DO NOTHING",&[&b.id,&b.owner,&b.computer,&request_key,&fingerprint,&json]).await.map_err(|_|Failure::Unavailable)?;
        let row=client.query_one("SELECT fingerprint,binding FROM computer_credentials.bindings WHERE owner=$1 AND computer=$2 AND request_key=$3",&[&b.owner,&b.computer,&request_key]).await.map_err(|_|Failure::Unavailable)?;
        if row.get::<_, String>(0) != fingerprint {
            return Err(Failure::Invalid);
        }
        serde_json::from_value(row.get(1)).map_err(|_| Failure::Unavailable)
    }
    async fn activate(&self, b: &Binding, digest: &str) -> Result<(), Failure> {
        if !credentials::digest(digest) {
            return Err(Failure::Denied);
        }
        let mut next = b.clone();
        next.state = BindingState::Active;
        next.command_digest = Some(digest.into());
        next.revision = next.revision.checked_add(1).ok_or(Failure::Denied)?;
        let count = self
            .db
            .client()
            .await
            .map_err(|_| Failure::Unavailable)?
            .execute(
                "UPDATE computer_credentials.bindings SET binding=$3 WHERE id=$1 AND binding=$2",
                &[
                    &b.id,
                    &serde_json::to_value(b).map_err(|_| Failure::Invalid)?,
                    &serde_json::to_value(&next).map_err(|_| Failure::Invalid)?,
                ],
            )
            .await
            .map_err(|_| Failure::Unavailable)?;
        if count != 1 {
            use credentials::Store as _;
            if self.get(b.id).await? != Some(next) {
                return Err(Failure::Denied);
            }
        }
        Ok(())
    }
    async fn revoke(&self, owner: &str, id: Uuid) -> Result<bool, Failure> {
        Ok(self.db.client().await.map_err(|_|Failure::Unavailable)?.execute("UPDATE computer_credentials.bindings SET binding=jsonb_set(binding,'{state}','\"revoked\"') WHERE id=$1 AND owner=$2",&[&id,&owner]).await.map_err(|_|Failure::Unavailable)?==1)
    }
    /// In-memory account authority cannot survive a broker restart.
    pub async fn revoke_after_restart(&self, deployment: &str) -> Result<(), Failure> {
        self.db.client().await.map_err(|_|Failure::Unavailable)?.execute("UPDATE computer_credentials.bindings SET binding=jsonb_set(binding,'{state}','\"revoked\"') WHERE binding->>'state' != 'revoked' AND computer IN (SELECT id FROM computers.computers WHERE binding->>'deployment'=$1)",&[&deployment]).await.map_err(|_|Failure::Unavailable)?;
        Ok(())
    }
    async fn candidates(&self, c: &gateway::Context) -> Result<Vec<Binding>, Failure> {
        let rows=self.db.client().await.map_err(|_|Failure::Unavailable)?.query("SELECT binding FROM computer_credentials.bindings WHERE binding->>'native_resource'=$1 AND binding->>'node'=$2 AND binding->>'state' IN ('prepared','active') AND (binding->>'expires_unix')::numeric > extract(epoch from now()) ORDER BY id LIMIT 257",&[&c.native_resource,&c.node]).await.map_err(|_|Failure::Unavailable)?;
        if rows.len() > MAX_REFERENCES {
            return Err(Failure::Unavailable);
        }
        rows.into_iter()
            .map(|row| serde_json::from_value(row.get(0)).map_err(|_| Failure::Unavailable))
            .collect()
    }
}
#[async_trait]
impl credentials::Store for Repository {
    async fn get(&self, id: Uuid) -> Result<Option<Binding>, Failure> {
        self.db
            .client()
            .await
            .map_err(|_| Failure::Unavailable)?
            .query_opt(
                "SELECT binding FROM computer_credentials.bindings WHERE id=$1",
                &[&id],
            )
            .await
            .map_err(|_| Failure::Unavailable)?
            .map(|row| serde_json::from_value(row.get(0)).map_err(|_| Failure::Unavailable))
            .transpose()
    }
}

struct Grant {
    token: Secret,
    expires_unix: u64,
}
/// Non-secret source definitions are reread at use. Editing or removing a
/// connection invalidates its old handles, even without a process restart.
pub struct Authority {
    pub repository: Arc<Repository>,
    pub computers: Store,
    pub source_file: PathBuf,
    pub public_origin: String,
    pub native_origin: String,
    pub native_token: Secret,
    pub deployment: String,
    http: reqwest::Client,
    grants: Mutex<BTreeMap<Uuid, Arc<Grant>>>,
}
impl Authority {
    pub fn new(
        repository: Arc<Repository>,
        computers: Store,
        source_file: PathBuf,
        public_origin: &str,
        native_origin: &str,
        native_token: Secret,
        deployment: &str,
    ) -> Result<Self, Failure> {
        fn origin(s: &str) -> Result<String, Failure> {
            let url = url::Url::parse(s).map_err(|_| Failure::Invalid)?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || url.path() != "/"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(Failure::Invalid);
            }
            Ok(s.trim_end_matches('/').into())
        }
        if !credentials::opaque(deployment, 128) {
            return Err(Failure::Invalid);
        }
        Ok(Self {
            repository,
            computers,
            source_file,
            public_origin: origin(public_origin)?,
            native_origin: origin(native_origin)?,
            native_token,
            deployment: deployment.into(),
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|_| Failure::Unavailable)?,
            grants: Mutex::new(BTreeMap::new()),
        })
    }
    pub fn sources(&self) -> Result<Vec<Source>, Failure> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&self.source_file)
            .map_err(|_| Failure::Unavailable)?;
        let metadata = file.metadata().map_err(|_| Failure::Unavailable)?;
        if !metadata.is_file() || metadata.mode() & 0o077 != 0 {
            return Err(Failure::Unavailable);
        }
        let mut bytes = Vec::new();
        file.take(262145)
            .read_to_end(&mut bytes)
            .map_err(|_| Failure::Unavailable)?;
        if bytes.len() > 262144 {
            return Err(Failure::Unavailable);
        }
        let sources: Vec<Source> =
            serde_json::from_slice(&bytes).map_err(|_| Failure::Unavailable)?;
        if sources.len() > 128 {
            return Err(Failure::Invalid);
        }
        let mut ids = std::collections::BTreeSet::new();
        for source in &sources {
            source.validate()?;
            if source.rules.values().any(|r| r.destination.port != 443) {
                return Err(Failure::Invalid);
            }

            if !ids.insert(source.id) {
                return Err(Failure::Invalid);
            }
        }
        Ok(sources)
    }
    async fn read(&self, url: &str, token: &Secret) -> Result<Value, Failure> {
        let bearer = std::str::from_utf8(token.bytes()).map_err(|_| Failure::Invalid)?;
        let mut response = self
            .http
            .get(url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|_| Failure::Unavailable)?;
        match response.status().as_u16() {
            200 => {}
            401 | 403 | 404 => return Err(Failure::Denied),
            _ => return Err(Failure::Unavailable),
        }
        let mut bytes = Zeroizing::new(Vec::new());
        while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Unavailable)? {
            if bytes.len().saturating_add(chunk.len()) > 1048576 {
                return Err(Failure::Unavailable);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| Failure::Unavailable)
    }
    async fn account(&self, token: &Secret) -> Result<Value, Failure> {
        self.read(&format!("{}/v1/account", self.public_origin), token)
            .await
    }
    fn owner(account: &Value) -> Result<String, Failure> {
        account["account"]["id"]
            .as_str()
            .filter(|id| credentials::opaque(id, 128))
            .map(str::to_owned)
            .ok_or(Failure::Denied)
    }
    fn computer_owner(account: &str) -> String {
        let mut hash = Sha256::new();
        hash.update(b"openagents.web.chat.account.v1\0");
        hash.update(account.as_bytes());
        format!("account:{:x}", hash.finalize())
    }
    async fn workspace(
        &self,
        source: &Source,
        account: &Value,
        token: &Secret,
    ) -> Result<Option<Workspace>, Failure> {
        match &source.owner {
            Owner::Account { id } => {
                if Self::owner(account)? == *id {
                    Ok(None)
                } else {
                    Err(Failure::Denied)
                }
            }
            Owner::Workspace { id } => {
                let workspace = account["workspaces"]
                    .as_array()
                    .ok_or(Failure::Denied)?
                    .iter()
                    .find(|w| {
                        w["id"].as_str() == Some(id)
                            && matches!(w["role"].as_str(), Some("owner" | "admin" | "member"))
                    })
                    .ok_or(Failure::Denied)?;
                let _ = workspace;
                let detail = self
                    .read(&format!("{}/v1/workspaces/{id}", self.public_origin), token)
                    .await?;
                if detail["workspace"]["id"].as_str() != Some(id)
                    || !matches!(detail["role"].as_str(), Some("owner" | "admin" | "member"))
                {
                    return Err(Failure::Denied);
                }
                let members_epoch = detail["workspace"]["members_epoch"]
                    .as_u64()
                    .ok_or(Failure::Denied)?;
                Ok(Some(Workspace {
                    id: id.clone(),
                    members_epoch,
                }))
            }
        }
    }
    async fn native(&self, resource: &str) -> Result<Value, Failure> {
        if !credentials::opaque(resource, 128) || !resource.starts_with("sbx_") {
            return Err(Failure::Denied);
        }
        self.read(
            &format!(
                "{}/v1/sandboxes/{resource}/credential-context",
                self.native_origin
            ),
            &self.native_token,
        )
        .await
    }
    async fn current_with(
        &self,
        b: &Binding,
        token: &Secret,
        require_command: bool,
    ) -> Result<Current, Failure> {
        let source = self
            .sources()?
            .into_iter()
            .find(|s| s.id == b.source)
            .ok_or(Failure::Denied)?;
        if source.digest()? != b.source_digest {
            return Err(Failure::Denied);
        }
        let account = self.account(token).await?;
        if Self::owner(&account)? != b.owner
            || self.workspace(&source, &account, token).await? != b.workspace
        {
            return Err(Failure::Denied);
        }
        // This public read rechecks current computer permission and key scope.
        let visible = self
            .read(
                &format!("{}/v1/computers/{}", self.public_origin, b.computer),
                token,
            )
            .await?;
        if visible["id"].as_str() != Some(&b.computer) {
            return Err(Failure::Denied);
        }
        let entry = self
            .computers
            .get(&Self::computer_owner(&b.owner), &b.computer)
            .await
            .map_err(|_| Failure::Unavailable)?
            .ok_or(Failure::Denied)?;
        let binding = entry.binding.as_ref().ok_or(Failure::Denied)?;
        if !matches!(entry.phase.as_str(), "awake" | "turn")
            || entry.desired != Desired::Running
            || binding.deployment != self.deployment
            || !binding.credential_proxy
            || binding.network_profile.as_ref() != Some(&b.profile)
            || entry.intent_generation < 0
            || entry.intent_generation as u64 != b.grant_revision
        {
            return Err(Failure::Denied);
        }
        let native = self
            .native(entry.record.resource().ok_or(Failure::Denied)?)
            .await?;
        let context:gateway::Context=serde_json::from_value(json!({"native_resource":native["native_resource"],"node":native["node"],"execution_generation":native["execution_generation"],"native_policy_ref":native["native_policy_ref"]})).map_err(|_|Failure::Denied)?;
        if context.native_resource != b.native_resource
            || context.node != b.node
            || context.execution_generation != b.execution_generation
            || context.native_policy_ref != b.native_policy_ref
            || normalized_policy(native["policy"].clone())
                != normalized_policy(b.profile.space_policy().map_err(|_| Failure::Denied)?)
        {
            return Err(Failure::Denied);
        }
        let mut command_digest = String::new();
        if require_command {
            let id = self
                .computers
                .command_by_request(&Self::computer_owner(&b.owner), &b.computer, &b.command_key)
                .await
                .map_err(|_| Failure::Unavailable)?
                .ok_or(if b.state == BindingState::Prepared {
                    Failure::Unavailable
                } else {
                    Failure::Denied
                })?;
            let command = self
                .computers
                .command(&Self::computer_owner(&b.owner), &b.computer, &id)
                .await
                .map_err(|_| Failure::Unavailable)?
                .ok_or(Failure::Denied)?;
            if b.state == BindingState::Prepared && command.status == "queued" && !command.cancel {
                return Err(Failure::Unavailable);
            }
            if !matches!(command.status.as_str(), "dispatching" | "running")
                || command.cancel
                || command.resource.as_deref() != entry.record.resource()
            {
                return Err(Failure::Denied);
            }
            command_digest = command.digest;
        }
        Ok(Current {
            owner: b.owner.clone(),
            workspace: b.workspace.clone(),
            computer: b.computer.clone(),
            native_resource: context.native_resource,
            node: context.node,
            execution_generation: context.execution_generation,
            native_policy_ref: context.native_policy_ref,
            command_key: b.command_key.clone(),
            command_digest,
            policy_digest: b.profile.digest().map_err(|_| Failure::Denied)?,
            source_revision: source.revision,
            grant_revision: entry.intent_generation as u64,
            running: true,
            grant_active: true,
        })
    }
    /// Remove expired or transactionally revoked caller authority from memory.
    pub async fn prune_grants(&self) -> Result<(), Failure> {
        let now = SystemClock.now_unix();
        let ids = {
            let mut grants = self.grants.lock().map_err(|_| Failure::Unavailable)?;
            grants.retain(|_, grant| grant.expires_unix > now);
            grants.keys().copied().collect::<Vec<_>>()
        };
        if ids.is_empty() {
            return Ok(());
        }
        let rows=self.repository.db.client().await.map_err(|_|Failure::Unavailable)?.query("SELECT id FROM computer_credentials.bindings WHERE id=ANY($1) AND binding->>'state' IN ('prepared','active') AND (binding->>'expires_unix')::numeric > extract(epoch from now())",&[&ids]).await.map_err(|_|Failure::Unavailable)?;
        let live = rows
            .into_iter()
            .map(|row| row.get::<_, Uuid>(0))
            .collect::<std::collections::BTreeSet<_>>();
        self.grants
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .retain(|id, _| !ids.contains(id) || live.contains(id));
        Ok(())
    }

    fn grant(&self, id: Uuid) -> Result<Arc<Grant>, Failure> {
        self.grants
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .get(&id)
            .cloned()
            .ok_or(Failure::Denied)
    }
}
fn normalized_policy(mut policy: Value) -> Value {
    if policy.is_object() {
        for name in ["network_credential_rules", "tls_interception_exemptions"] {
            if policy.get(name).is_none() {
                policy[name] = json!([]);
            }
        }
    }
    policy
}
#[async_trait]
impl credentials::Authorizer for Authority {
    async fn current(&self, b: &Binding) -> Result<Current, Failure> {
        let grant = self.grant(b.id)?;
        let result = self.current_with(b, &grant.token, true).await;
        if matches!(result, Err(Failure::Denied)) {
            self.repository.revoke(&b.owner, b.id).await?;
        }
        result
    }
}
#[async_trait]
impl gateway::Authority for Authority {
    async fn admit(&self, c: &gateway::Context) -> Result<gateway::Admission, Failure> {
        c.validate()?;
        let mut profile = None;
        let mut references = Vec::new();
        let now = SystemClock.now_unix();
        for b in self.repository.candidates(c).await? {
            if b.expires_unix <= now
                || c.execution_generation != 0 && b.execution_generation != c.execution_generation
                || b.native_policy_ref != c.native_policy_ref
            {
                continue;
            }
            if b.state == BindingState::Prepared {
                let owner = Self::computer_owner(&b.owner);
                let Some(id) = self
                    .computers
                    .command_by_request(&owner, &b.computer, &b.command_key)
                    .await
                    .map_err(|_| Failure::Unavailable)?
                else {
                    continue;
                };
                let Some(command) = self
                    .computers
                    .command(&owner, &b.computer, &id)
                    .await
                    .map_err(|_| Failure::Unavailable)?
                else {
                    continue;
                };
                if command.status == "queued" && !command.cancel {
                    continue;
                }
            }
            let grant = match self.grant(b.id) {
                Ok(g) => g,
                Err(Failure::Denied) => continue,
                Err(f) => return Err(f),
            };
            let current = match self.current_with(&b, &grant.token, true).await {
                Ok(current) => current,
                Err(Failure::Denied) => {
                    self.repository.revoke(&b.owner, b.id).await?;
                    continue;
                }
                Err(f) => return Err(f),
            };
            if b.state == BindingState::Prepared {
                self.repository
                    .activate(&b, &current.command_digest)
                    .await?;
            }
            if profile.as_ref().is_some_and(|p| p != &b.profile) {
                return Err(Failure::Denied);
            }
            profile = Some(b.profile);
            references.push(b.id);
        }
        Ok(gateway::Admission {
            profile: profile.ok_or(Failure::Denied)?,
            references,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    source: Uuid,
    rule: String,
    command_key: String,
    idempotency_key: String,
    ttl_seconds: u64,
}
fn control_token(headers: &HeaderMap) -> Result<Secret, Failure> {
    if headers.get_all(header::AUTHORIZATION).iter().count() != 1 {
        return Err(Failure::Denied);
    }
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(Failure::Denied)?;
    if value.len() > 512 || !value.bytes().all(|b| (b'!'..=b'~').contains(&b)) {
        return Err(Failure::Denied);
    }
    Secret::new(value.as_bytes().to_vec())
}
fn view(b: &Binding) -> Value {
    json!({"id":b.id,"computer":b.computer,"source":b.source,"rule":b.rule,"command_key":b.command_key,"state":b.state,"created_unix":b.created_unix,"expires_unix":b.expires_unix,"egress_profile":b.profile.selector().ok()})
}
fn control_error(f: Failure) -> Response {
    let status = match f {
        Failure::Denied => StatusCode::NOT_FOUND,
        Failure::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        Failure::Invalid => StatusCode::BAD_REQUEST,
    };
    (status,[(header::CACHE_CONTROL,"no-store")],Json(json!({"error":"credential_unavailable","message":"We couldn't use those credential settings."}))).into_response()
}
async fn prepare(
    State(a): State<Arc<Authority>>,
    Path(computer): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    match prepare_inner(&a, &computer, &headers, &body).await {
        Ok(b) => (
            StatusCode::CREATED,
            [(header::CACHE_CONTROL, "no-store")],
            Json(view(&b)),
        )
            .into_response(),
        Err(f) => control_error(f),
    }
}
async fn prepare_inner(
    a: &Authority,
    computer: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Binding, Failure> {
    use credentials::Store as _;
    let token = control_token(headers)?;
    let request: Prepare = serde_json::from_slice(body).map_err(|_| Failure::Invalid)?;
    let valid_key = |s: &str| credentials::opaque(s, 128) && !s.contains(':');
    if !computer.starts_with("cmp_")
        || !valid_key(computer)
        || !valid_key(&request.command_key)
        || !valid_key(&request.idempotency_key)
        || !(30..=credentials::MAX_HANDLE_SECONDS).contains(&request.ttl_seconds)
    {
        return Err(Failure::Invalid);
    }
    let account = a.account(&token).await?;
    let owner = Authority::owner(&account)?;
    let source = a
        .sources()?
        .into_iter()
        .find(|s| s.id == request.source)
        .ok_or(Failure::Denied)?;
    let workspace = a.workspace(&source, &account, &token).await?;
    let rule = source.rules.get(&request.rule).ok_or(Failure::Denied)?;
    let visible = a
        .read(
            &format!("{}/v1/computers/{computer}", a.public_origin),
            &token,
        )
        .await?;
    if visible["id"].as_str() != Some(computer) {
        return Err(Failure::Denied);
    }
    let entry = a
        .computers
        .get(&Authority::computer_owner(&owner), computer)
        .await
        .map_err(|_| Failure::Unavailable)?
        .ok_or(Failure::Denied)?;
    let pinned = entry.binding.as_ref().ok_or(Failure::Denied)?;
    if !matches!(entry.phase.as_str(), "awake" | "turn")
        || entry.desired != Desired::Running
        || pinned.deployment != a.deployment
        || !pinned.credential_proxy
        || entry.intent_generation < 0
    {
        return Err(Failure::Denied);
    }
    let profile = pinned.network_profile.as_ref().ok_or(Failure::Denied)?;
    if !profile.allow_hosts.contains(&rule.destination.host) || !profile.tls_exempt_hosts.is_empty()
    {
        return Err(Failure::Denied);
    }
    let native = a
        .native(entry.record.resource().ok_or(Failure::Denied)?)
        .await?;
    if normalized_policy(native["policy"].clone())
        != normalized_policy(profile.space_policy().map_err(|_| Failure::Invalid)?)
    {
        return Err(Failure::Denied);
    }
    let c:gateway::Context=serde_json::from_value(json!({"native_resource":native["native_resource"],"node":native["node"],"execution_generation":native["execution_generation"],"native_policy_ref":native["native_policy_ref"]})).map_err(|_|Failure::Denied)?;
    c.validate()?;
    let now = SystemClock.now_unix();
    let b = Binding {
        id: Uuid::new_v4(),
        revision: 1,
        state: BindingState::Prepared,
        source: source.id,
        source_revision: source.revision,
        source_digest: source.digest()?,
        rule: request.rule.clone(),
        owner,
        workspace,
        grant_revision: entry.intent_generation as u64,
        computer: computer.into(),
        native_resource: c.native_resource,
        node: c.node,
        execution_generation: c.execution_generation,
        native_policy_ref: c.native_policy_ref,
        command_key: request.command_key.clone(),
        command_digest: None,
        profile: profile.clone(),
        created_unix: now,
        expires_unix: now
            .checked_add(request.ttl_seconds)
            .ok_or(Failure::Invalid)?,
    };
    b.validate()?;
    a.current_with(&b, &token, false).await?;
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&request).map_err(|_| Failure::Invalid)?)
    );
    // Reserve bounded memory before the durable write. The mutex is never held
    // across I/O. Removing an expired grant drops its last in-memory token.
    {
        let mut grants = a.grants.lock().map_err(|_| Failure::Unavailable)?;
        grants.retain(|_, g| g.expires_unix > now);
        if grants.len() >= MAX_REFERENCES {
            return Err(Failure::Unavailable);
        }
    }
    let saved = a
        .repository
        .prepare(&request.idempotency_key, &fingerprint, &b)
        .await?;
    if saved.state == BindingState::Revoked
        || saved.expires_unix <= now
        || saved.source_digest != b.source_digest
        || saved.grant_revision != b.grant_revision
        || saved.native_resource != b.native_resource
        || saved.execution_generation != b.execution_generation
        || saved.native_policy_ref != b.native_policy_ref
        || saved.profile != b.profile
    {
        return Err(Failure::Denied);
    }
    a.current_with(&saved, &token, false).await?;
    {
        let mut grants = a.grants.lock().map_err(|_| Failure::Unavailable)?;
        if grants.len() >= MAX_REFERENCES && !grants.contains_key(&saved.id) {
            return Err(Failure::Unavailable);
        }
        grants.insert(
            saved.id,
            Arc::new(Grant {
                token,
                expires_unix: saved.expires_unix,
            }),
        );
    }
    // A concurrent stop/revoke must not turn a retry into renewed authority.
    let after = a.repository.get(saved.id).await?.ok_or(Failure::Denied)?;
    if after.state == BindingState::Revoked {
        a.grants
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .remove(&saved.id);
        return Err(Failure::Denied);
    }
    Ok(after)
}
async fn read_reference(
    State(a): State<Arc<Authority>>,
    Path((computer, id)): Path<(String, Uuid)>,
    headers: HeaderMap,
) -> Response {
    match reference(&a, &computer, id, &headers, false).await {
        Ok(b) => (
            StatusCode::OK,
            [(header::CACHE_CONTROL, "no-store")],
            Json(view(&b)),
        )
            .into_response(),
        Err(f) => control_error(f),
    }
}
async fn delete_reference(
    State(a): State<Arc<Authority>>,
    Path((computer, id)): Path<(String, Uuid)>,
    headers: HeaderMap,
) -> Response {
    match reference(&a, &computer, id, &headers, true).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(f) => control_error(f),
    }
}
async fn reference(
    a: &Authority,
    computer: &str,
    id: Uuid,
    headers: &HeaderMap,
    revoke: bool,
) -> Result<Binding, Failure> {
    use credentials::Store as _;
    let token = control_token(headers)?;
    let account = a.account(&token).await?;
    let owner = Authority::owner(&account)?;
    let b = a
        .repository
        .get(id)
        .await?
        .filter(|b| b.owner == owner && b.computer == computer)
        .ok_or(Failure::Denied)?;
    if revoke {
        if !a.repository.revoke(&owner, id).await? {
            return Err(Failure::Denied);
        }
        a.grants
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .remove(&id);
    }
    Ok(b)
}
pub fn control_router(authority: Arc<Authority>) -> Router {
    Router::new()
        .route(
            "/v1/computers/{computer}/credential-references",
            post(prepare),
        )
        .route(
            "/v1/computers/{computer}/credential-references/{id}",
            get(read_reference).delete(delete_reference),
        )
        .layer(DefaultBodyLimit::max(8192))
        .with_state(authority)
}

#[cfg(test)]
mod tests {
    use super::*;
    use credentials::Store as _;
    fn fixture(computer: &str) -> Binding {
        Binding {
            id: Uuid::new_v4(),
            revision: 1,
            state: BindingState::Prepared,
            source: Uuid::from_u128(1),
            source_revision: 1,
            source_digest: "b".repeat(64),
            rule: "messages".into(),
            owner: "test-account".into(),
            workspace: None,
            grant_revision: 0,
            computer: computer.into(),
            native_resource: Uuid::from_u128(2).to_string(),
            node: "node-one".into(),
            execution_generation: 1,
            native_policy_ref: format!(
                "cluster:sandbox:test:generation:1:sha256:{}",
                "a".repeat(64)
            ),
            command_key: "one-command".into(),
            command_digest: None,
            profile: credentials::policy::Profile::new(
                "test-v1",
                [credentials::policy::Host::parse("api.example.com").unwrap()].into(),
            ),
            created_unix: SystemClock.now_unix(),
            expires_unix: SystemClock.now_unix() + 300,
        }
    }
    #[tokio::test]
    async fn lifecycle_and_command_changes_revoke_in_the_same_transaction() {
        let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
            return;
        };
        let (db, _) = Db::scratch(&admin).await.unwrap();
        let repository = Repository::existing(db.clone()).await.unwrap();
        let store = Store::new(db.clone());
        let app = crate::app(
            store.clone(),
            computer_driver::Capabilities::new("fake", computer_driver::Profile::ALL, &["small"]),
            "synthetic-service-token",
            crate::turns::Secrets::default(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let computer: Value = reqwest::Client::new()
            .post(format!("http://{address}/v1/computers"))
            .bearer_auth("synthetic-service-token")
            .header("X-OpenAgents-Account", "test-account")
            .json(&json!({"idempotency_key":"credential-revocation"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let computer = computer["id"].as_str().unwrap();
        let client = db.client().await.unwrap();
        client
            .execute(
                "UPDATE computers.computers SET phase='awake' WHERE id=$1",
                &[&computer],
            )
            .await
            .unwrap();
        let b = fixture(computer);
        b.validate().unwrap();
        assert_eq!(
            repository
                .prepare("reference-one", "fingerprint-one", &b)
                .await
                .unwrap(),
            b
        );
        let mut other = b.clone();
        other.id = Uuid::new_v4();
        assert_eq!(
            repository
                .prepare("reference-one", "fingerprint-one", &other)
                .await
                .unwrap()
                .id,
            b.id
        );
        assert!(
            repository
                .prepare("reference-one", "different-settings", &other)
                .await
                .is_err()
        );
        let digest = "c".repeat(64);
        let (first, second) = tokio::join!(
            repository.activate(&b, &digest),
            repository.activate(&b, &digest)
        );
        first.unwrap();
        second.unwrap();
        let active = repository.get(b.id).await.unwrap().unwrap();
        assert_eq!(active.command_digest.as_deref(), Some(digest.as_str()));
        assert!(!repository.revoke("foreign-account", b.id).await.unwrap());
        store
            .set_desired("test-account", computer, Desired::Stopped)
            .await
            .unwrap();
        assert_eq!(
            repository.get(b.id).await.unwrap().unwrap().state,
            BindingState::Revoked
        );
        assert!(repository.activate(&active, &digest).await.is_err());
        store
            .set_desired("test-account", computer, Desired::Running)
            .await
            .unwrap();
        assert_eq!(
            repository
                .prepare("reference-one", "fingerprint-one", &other)
                .await
                .unwrap()
                .state,
            BindingState::Revoked
        );
        client
            .execute(
                "UPDATE computers.computers SET phase='awake' WHERE id=$1",
                &[&computer],
            )
            .await
            .unwrap();
        let request = crate::commands::Request {
            command: "true".into(),
            cwd: ".".into(),
            timeout_seconds: 30,
            env: BTreeMap::new(),
            idempotency_key: "one-command".into(),
            credential_ref: None,
        };
        let (command, _) = store
            .add_command("test-account", computer, &request)
            .await
            .unwrap();
        let fresh = repository
            .prepare("reference-two", "fingerprint-two", &other)
            .await
            .unwrap();
        client
            .execute(
                "UPDATE computers.commands SET cancel_requested=true WHERE turn=$1",
                &[&command],
            )
            .await
            .unwrap();
        assert_eq!(
            repository.get(fresh.id).await.unwrap().unwrap().state,
            BindingState::Revoked
        );
        let mut next = fixture(computer);
        next.command_key = "another-command".into();
        let saved = repository
            .prepare("reference-three", "fingerprint-three", &next)
            .await
            .unwrap();
        client
            .execute(
                "UPDATE computers.computers SET phase='unknown' WHERE id=$1",
                &[&computer],
            )
            .await
            .unwrap();
        assert_eq!(
            repository.get(saved.id).await.unwrap().unwrap().state,
            BindingState::Revoked
        );
        server.abort();
    }
    #[test]
    fn public_preparation_cannot_choose_a_cloud_path_owner_or_header() {
        let base = json!({"source":"00000000-0000-0000-0000-000000000001","rule":"messages","command_key":"command","idempotency_key":"request","ttl_seconds":300});
        for field in [
            "owner",
            "workspace",
            "secret_version",
            "project",
            "header",
            "proxy_token",
        ] {
            let mut sent = base.clone();
            sent[field] = json!("untrusted");
            assert!(serde_json::from_value::<Prepare>(sent).is_err());
        }
        let a = json!({"account":{"id":"acct_one"}});
        let b = json!({"account":{"id":"acct_two"}});
        assert_ne!(Authority::owner(&a).unwrap(), Authority::owner(&b).unwrap());
    }
}
