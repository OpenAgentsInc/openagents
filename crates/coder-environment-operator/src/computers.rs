//! Environment machines through the shared account-scoped Computers client.
//! Saved requests contain references and frozen inputs; account and repository
//! credential values remain in the web service's separate encrypted grant.
mod provider;
use crate::{Config, Layout, Providers};
use coder_environment::{ImageIdentity, Provider as ProviderKind, digest, valid_id};
pub use coder_environment_setup::GIT_CREDENTIALS;
use coder_working_computer::{Computer, Purpose};
pub use computer_driver::{EnvironmentAttempt, EnvironmentRole};
use fs2::FileExt;
use hmac::{Hmac, Mac};
use openagents_sdk::{
    Client, Deployment, Session,
    computers::{CommandRef, CommandRequest, ComputerRef, CreateOptions, CreateRequest},
};
pub use provider::Selected;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub const ACCOUNTS: &str = "OPENAGENTS_ENVIRONMENT_COMPUTERS_ACCOUNTS";
pub const ORIGIN: &str = "OPENAGENTS_ENVIRONMENT_COMPUTERS_ORIGIN";
pub const SCHEMA: &str = "openagents.environment.computer.v1";

/// Shared command output is scrubbed by the Rust backend before persistence.
/// The native recorder must not resolve a different operator's Git credential.
pub fn custody(shared: bool) -> crate::Custody {
    if !shared {
        return crate::environment_custody();
    }
    Arc::new(|names| {
        if names
            .iter()
            .any(|name| !GIT_CREDENTIALS.contains(&name.as_str()))
        {
            return Err("These setup credentials aren't supported.".into());
        }
        Ok(coder_environment::evidence::Redactor::new())
    })
}
pub fn enrolled(account: &str) -> bool {
    std::env::var(ACCOUNTS)
        .unwrap_or_default()
        .split(',')
        .any(|v| v.trim() == account)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub account: String,
    pub workspace: Option<(String, u64)>,
    pub repository: String,
    pub private: bool,
    pub expires_unix: u64,
    /// An oa-seal value. Only the web boundary can open it.
    pub credential: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedCommand {
    pub digest: String,
    pub spec: computer_driver::CommandSpec,
    pub request: CommandRequest,
    pub reference: Option<CommandRef>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub schema: String,
    pub native: String,
    pub account: String,
    pub workspace: Option<(String, u64)>,
    pub expires_unix: u64,
    pub fingerprint: String,
    pub attempt: EnvironmentAttempt,
    /// Complete frozen setup, build, or verification inputs.
    pub inputs: Value,
    pub image: String,
    pub environment_image: Option<ImageIdentity>,
    pub options: CreateOptions,
    pub request: Option<CreateRequest>,
    pub computer: Option<ComputerRef>,
    pub commands: BTreeMap<String, SavedCommand>,
    pub deleted: bool,
    pub image_deleted: bool,
}
impl Binding {
    pub fn validates(&self, c: &Computer) -> bool {
        self.schema == SCHEMA
            && self.native == c.id
            && self.fingerprint == fingerprint(c)
            && self.attempt.validate().is_ok()
            && self.attempt.attempt == c.id
            && self.options.idempotency_key.is_some()
            && !self.account.is_empty()
            && self.computer.as_ref().is_none_or(|reference| {
                reference.account() == self.account
                    && c.resource()
                        .is_none_or(|resource| resource == reference.id())
            })
            && environment(c) == Some(self.attempt.environment.as_str())
            && self.attempt.source_digest == c.source.digest
            && self.attempt.deadline_ms == c.created_ms.saturating_add(c.bounds.absolute_ms)
            && self.attempt.idle_ms == c.bounds.idle_ms
            && matches!(
                (&c.purpose, &self.attempt.role),
                (Purpose::EnvironmentSetup { .. }, EnvironmentRole::Setup)
                    | (Purpose::EnvironmentBuild { .. }, EnvironmentRole::Builder)
                    | (Purpose::EnvironmentVerify { .. }, EnvironmentRole::Verifier)
            )
    }
}
pub fn fingerprint(c: &Computer) -> String {
    digest(
        &serde_json::to_vec(&json!([
            c.id,
            c.owner,
            c.project,
            c.source,
            c.purpose,
            c.provider,
            c.size,
            c.bounds,
            c.credential_names
        ]))
        .expect("computer metadata encodes"),
    )
}
pub fn credential(binding: &Binding, key: &[u8]) -> String {
    let mut h = Hmac::<Sha256>::new_from_slice(key).expect("HMAC key");
    h.update(b"openagents.environment.computer-credential.v1\0");
    h.update(
        &serde_json::to_vec(&json!([
            binding.schema,
            binding.native,
            binding.account,
            binding.workspace,
            binding.expires_unix,
            binding.fingerprint,
            binding.attempt,
            binding.inputs,
            binding.image,
            binding.environment_image,
            binding.options
        ]))
        .expect("binding encodes"),
    );
    format!("ecj_{}.{:x}", binding.native, h.finalize().into_bytes())
}
pub fn environment(c: &Computer) -> Option<&str> {
    match &c.purpose {
        Purpose::EnvironmentSetup { environment }
        | Purpose::EnvironmentBuild { environment, .. }
        | Purpose::EnvironmentVerify { environment, .. } => Some(environment),
        Purpose::Chat => None,
    }
}
#[derive(Clone)]
pub struct Store {
    pub state: PathBuf,
}
impl Store {
    pub fn under(state: impl Into<PathBuf>) -> Self {
        Self {
            state: state.into(),
        }
    }
    fn dir(&self, name: &str) -> PathBuf {
        self.state.join("environment-computers").join(name)
    }
    pub(crate) fn path(&self, group: &str, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) || id.len() > 128 {
            return Err("The Environment identity is invalid.".into());
        }
        Ok(self.dir(group).join(format!("{id}.json")))
    }
    pub fn binding(&self, id: &str) -> Result<Option<Binding>, String> {
        read(&self.path("attempts", id)?)
    }
    pub fn save(&self, b: &Binding) -> Result<(), String> {
        write(&self.path("attempts", &b.native)?, b)
    }
    pub fn grant(&self, id: &str) -> Result<Option<Grant>, String> {
        read(&self.path("grants", id)?)
    }
    pub fn save_grant(&self, id: &str, grant: &Grant) -> Result<(), String> {
        write(&self.path("grants", id)?, grant)
    }
    pub fn image_owner(&self, name: &str) -> Result<Option<String>, String> {
        read(&self.path("images", name)?)
    }
    pub fn bind_image(&self, name: &str, id: &str) -> Result<(), String> {
        let _lock = self.lock(&format!("image-{}", &digest(name.as_bytes())[..48]))?;
        let path = self.path("images", name)?;
        if let Some(owner) = read::<String>(&path)? {
            return if owner == id {
                Ok(())
            } else {
                Err("This image belongs to another build.".into())
            };
        }
        write(&path, &id)
    }
    pub fn computer(&self, id: &str) -> Result<Computer, String> {
        coder_working_computer::store::Store::under(Layout::under(&self.state).computers())
            .read(id)
            .map_err(|_| "The Environment computer couldn't be read.".into())
    }
    pub fn metadata(&self, id: &str) -> Result<Value, String> {
        read(
            &self
                .state
                .join("environment-studio")
                .join(id)
                .join("meta.json"),
        )?
        .ok_or_else(|| "The Environment couldn't be read.".into())
    }
    pub fn lock(&self, id: &str) -> Result<File, String> {
        let path = self.path("locks", id)?;
        crate::private_dir(path.parent().unwrap())?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let f = options
            .open(path)
            .map_err(|_| "The computer record couldn't be opened.")?;
        f.try_lock_exclusive()
            .map_err(|_| "Another worker is handling this computer.")?;
        Ok(f)
    }
}
fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("The Environment record couldn't be read.".into()),
    };
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("The Environment record is too large.".into());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "The Environment record is invalid.".into())
}
pub fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    crate::private_dir(path.parent().ok_or("The Environment folder is invalid.")?)?;
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|_| "The Environment record couldn't be saved.")?;
    let tmp = path.with_extension(format!("{:x}.new", u64::from_be_bytes(nonce)));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&tmp)
            .map_err(|_| "The Environment record couldn't be saved.")?;
        file.write_all(
            &serde_json::to_vec(value)
                .map_err(|_| "The Environment record couldn't be encoded.")?,
        )
        .map_err(|_| "The Environment record couldn't be saved.")?;
        file.sync_all()
            .map_err(|_| "The Environment record couldn't be saved.")?;
        fs::rename(&tmp, path).map_err(|_| "The Environment record couldn't be saved.")?;
        File::open(path.parent().unwrap())
            .and_then(|f| f.sync_all())
            .map_err(|_| "The Environment record couldn't be saved.")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

pub struct Runtime {
    pub store: Store,
    client: Client,
    http: reqwest::Client,
    origin: String,
    key: [u8; 32],
}
impl Runtime {
    pub fn from_env(state: &Path) -> Result<Option<Arc<Self>>, String> {
        let Ok(origin) = std::env::var(ORIGIN) else {
            return Ok(None);
        };
        let hex = std::env::var("OPENAGENTS_WEB_ASK_SALT")
            .map_err(|_| "The Environment computer key is missing.")?;
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("The Environment computer key is invalid.".into());
        }
        let mut key = [0; 32];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|_| "The Environment computer key is invalid.")?;
        }
        Self::new(state, &origin, key).map(|r| Some(Arc::new(r)))
    }
    pub fn new(state: &Path, origin: &str, key: [u8; 32]) -> Result<Self, String> {
        let deployment = if origin.trim_end_matches('/') == "https://openagents.com" {
            Deployment::production()
        } else {
            Deployment::new(&format!("{}/v1", origin.trim_end_matches('/')), origin)
                .map_err(|_| "The Environment computer address is invalid.")?
        };
        Ok(Self {
            store: Store::under(state),
            client: Client::new(deployment)
                .map_err(|_| "The Environment computer client couldn't start.")?,
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(150))
                .build()
                .map_err(|_| "The Environment computer client couldn't start.")?,
            origin: origin.trim_end_matches('/').into(),
            key,
        })
    }
    /// Recover allocations with a lost create reply and delete expired attempts.
    /// Shared attempts never become private homes or ordinary chat computers.
    pub async fn sweep_expired(
        &self,
        now_ms: u64,
        keep: &std::collections::BTreeSet<String>,
    ) -> usize {
        let Ok(entries) = fs::read_dir(self.store.state.join("environment-computers/attempts"))
        else {
            return 0;
        };
        let mut deleted = 0;
        for entry in entries.flatten() {
            let Ok(bytes) = fs::read(entry.path()) else {
                continue;
            };
            let Ok(found) = serde_json::from_slice::<Binding>(&bytes) else {
                continue;
            };
            if found.deleted
                || found
                    .attempt
                    .deadline_ms
                    .saturating_add(crate::studio::leftovers::DEADLINE_GRACE_MS)
                    >= now_ms
            {
                continue;
            }
            let Ok(_lock) = self.store.lock(&found.native) else {
                continue;
            };
            let Ok(Some(mut b)) = self.store.binding(&found.native) else {
                continue;
            };
            let Ok(c) = self.store.computer(&b.native) else {
                continue;
            };
            if !b.validates(&c) {
                continue;
            }
            let Ok(session) = self.session(&b).await else {
                continue;
            };
            let Ok(api) = self.client.computers(&session) else {
                continue;
            };
            if b.computer.is_none() {
                let Ok(Some(id)): Result<Option<String>, String> =
                    self.private(&b, "computer", &Value::Null).await
                else {
                    continue;
                };
                let Ok(reference) = api.reference(&id) else {
                    continue;
                };
                b.computer = Some(reference);
                if self.store.save(&b).is_err() {
                    continue;
                }
            }
            let reference = b.computer.as_ref().unwrap();
            if keep.contains(reference.id()) {
                continue;
            }
            let cancel = openagents_sdk::CancellationToken::new();
            let _ = api.delete(reference, &cancel).await;
            if api
                .wait(
                    reference,
                    openagents_sdk::computers::WaitFor::Deleted,
                    Duration::from_secs(600),
                    &cancel,
                )
                .await
                .is_err()
            {
                continue;
            }
            b.deleted = true;
            if self.store.save(&b).is_ok() {
                deleted += 1
            }
        }
        deleted
    }
    async fn session(&self, b: &Binding) -> Result<Session, String> {
        let s = self
            .client
            .authenticate_environment_job(&credential(b, &self.key))
            .await
            .map_err(|_| "Environment access couldn't be checked.")?;
        if s.account().id != b.account {
            return Err("This computer belongs to another account.".into());
        }
        Ok(s)
    }
    async fn private<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        b: &Binding,
        action: &str,
        body: &T,
    ) -> Result<R, String> {
        let response = self
            .http
            .post(format!(
                "{}/v1/environment-computers/{}/{}",
                self.origin, b.native, action
            ))
            .bearer_auth(credential(b, &self.key))
            .json(body)
            .send()
            .await
            .map_err(|_| "The Environment computer couldn't be reached.")?;
        if !response.status().is_success() {
            return Err("Environment access is unavailable or has ended.".into());
        }
        if response.content_length().is_some_and(|n| n > 1024 * 1024) {
            return Err("The Environment reply is too large.".into());
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| "The Environment reply couldn't be read.")?;
        if bytes.len() > 1024 * 1024 {
            return Err("The Environment reply is too large.".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "The Environment reply is invalid.".into())
    }
    /// Freeze an existing native attempt before importing its scoped SDK session.
    pub fn prepare(&self, c: &Computer, operation: &str) -> Result<Binding, String> {
        if let Some(binding) = self.store.binding(&c.id)? {
            if !binding.validates(c) {
                return Err("The computer settings changed after this attempt started.".into());
            }
            return Ok(binding);
        }
        let id = environment(c).ok_or("This computer is not an Environment attempt.")?;
        let meta = self.store.metadata(id)?;
        let account = meta["account"]
            .as_str()
            .ok_or("The Environment needs an account.")?
            .to_owned();
        if meta["public_computers"] != true {
            return Err("This Environment uses its original computer connection.".into());
        }
        let grant = self
            .store
            .grant(id)?
            .ok_or("Sign in again before starting this Environment.")?;
        if grant.account != account {
            return Err("The Environment belongs to another account.".into());
        }
        let layout = Layout::under(&self.store.state);
        let env = coder_environment::store::Store::under(layout.environments())
            .read(id)
            .map_err(|_| "The Environment couldn't be read.")?;
        let recipe = &env
            .recipes
            .last()
            .ok_or("The Environment has no recipe.")?
            .recipe;
        let (role, inputs, image, environment_image, recipe_digest, image_name) = match &c.purpose {
            Purpose::EnvironmentSetup { .. } => (
                EnvironmentRole::Setup,
                serde_json::to_value(&env.source).unwrap(),
                recipe.base.image_id.clone(),
                None,
                recipe.digest(),
                None,
            ),
            Purpose::EnvironmentBuild { build, .. } => {
                let job = coder_environment_build::store::Store::under(layout.build_jobs())
                    .read(build)
                    .map_err(|_| "The build couldn't be read.")?;
                if job.computer != c.id
                    || job.inputs.environment != id
                    || job.inputs.source != c.source
                    || job.inputs.base.provider != ProviderKind::Boat
                    || job.inputs.space_requested
                {
                    return Err("This build needs a different image connection.".into());
                }
                (
                    EnvironmentRole::Builder,
                    serde_json::to_value(&job.inputs).unwrap(),
                    job.inputs.base.image_id.clone(),
                    None,
                    job.inputs.recipe_digest.clone(),
                    Some(job.inputs.image_name.clone()),
                )
            }
            Purpose::EnvironmentVerify {
                verification,
                image,
                ..
            } => {
                let job = coder_environment_verify::store::Store::under(layout.verify_jobs())
                    .read(verification)
                    .map_err(|_| "The verification couldn't be read.")?;
                if job.inputs.environment != id
                    || job.inputs.image.image_id != *image
                    || job.inputs.image.provider != ProviderKind::Boat
                {
                    return Err("This verifier needs a different image connection.".into());
                }
                (
                    EnvironmentRole::Verifier,
                    serde_json::to_value(&job.inputs).unwrap(),
                    image.clone(),
                    Some(job.inputs.image.clone()),
                    job.inputs.manifest.recipe_digest.clone(),
                    None,
                )
            }
            _ => return Err("This computer is not an Environment attempt.".into()),
        };
        if !c
            .credential_names
            .iter()
            .all(|name| coder_environment_setup::GIT_CREDENTIALS.contains(&name.as_str()))
        {
            return Err(
                "This Environment needs credentials that aren't supported here yet.".into(),
            );
        }
        let key = format!(
            "environment-{}",
            &digest(format!("{}:{operation}", c.id).as_bytes())[..48]
        );
        let options:CreateOptions=serde_json::from_value(json!({"environment":id,"version":format!("attempt-{}",c.id),"size":c.size,"idempotency_key":key,"needs":{"profiles":["core","suspend"],"command_sessions":true,"files":true}})).map_err(|_|"The computer settings are invalid.")?;
        let binding = Binding {
            schema: SCHEMA.into(),
            native: c.id.clone(),
            account,
            workspace: grant.workspace.clone(),
            expires_unix: grant.expires_unix,
            fingerprint: fingerprint(c),
            attempt: EnvironmentAttempt {
                environment: id.into(),
                attempt: c.id.clone(),
                role,
                source_digest: c.source.digest.clone(),
                recipe_digest,
                inputs_digest: digest(&serde_json::to_vec(&inputs).unwrap()),
                deadline_ms: c.created_ms.saturating_add(c.bounds.absolute_ms),
                idle_ms: c.bounds.idle_ms,
                image_name,
            },
            inputs,
            image,
            environment_image,
            options,
            request: None,
            computer: None,
            commands: BTreeMap::new(),
            deleted: false,
            image_deleted: false,
        };
        binding.attempt.validate().map_err(str::to_owned)?;
        if let Some(name) = binding.attempt.image_name.as_deref() {
            self.store.bind_image(name, &c.id)?;
        }
        self.store.save(&binding)?;
        Ok(binding)
    }
}
pub async fn providers(
    config: &Config,
    state: &Path,
    shared: bool,
) -> Result<Providers<Selected<coder_working_computer::boat::BoatProvider>>, String> {
    let original = crate::boat::providers(config).await?;
    let runtime = if shared {
        Runtime::from_env(state)?
    } else {
        None
    };
    let wrap = |legacy| Selected {
        legacy,
        shared,
        runtime: runtime.clone(),
        store: Store::under(state),
    };
    Ok(Providers {
        setup: wrap(original.setup),
        build: wrap(original.build),
        verify: wrap(original.verify),
    })
}

/// Check the retained build stage before its private image adapter may run.
pub fn capture_ready(store: &Store, binding: &Binding) -> bool {
    let Ok(computer) = store.computer(&binding.native) else {
        return false;
    };
    let Purpose::EnvironmentBuild { build, .. } = &computer.purpose else {
        return false;
    };
    let layout = Layout::under(&store.state);
    let Ok(job) = coder_environment_build::store::Store::under(layout.build_jobs()).read(build)
    else {
        return false;
    };
    job.phase == coder_environment_build::Phase::Capturing
        && job.report.is_some()
        && job.computer == binding.native
        && serde_json::to_value(&job.inputs).ok().as_ref() == Some(&binding.inputs)
}

#[cfg(test)]
mod tests;
