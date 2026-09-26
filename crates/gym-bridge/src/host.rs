//! Explicit local grants, immutable recipes, and durable launch intent.
pub use crate::sources::{Source, SourceKind, TRAINING_SCHEMA, TrainingSummary};
use crate::{
    protocol::*,
    sources::{self, Root},
    store::Store,
    *,
};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeConfig {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub wall_ms: u64,
    pub max_starts: u32,
    #[serde(default)]
    pub environment: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub sources: Vec<Source>,
    pub recipes: Vec<RecipeConfig>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PinnedRecipe {
    config: RecipeConfig,
    executable_digest: String,
    cwd_device: u64,
    cwd_inode: u64,
    recipe: Recipe,
}
impl PinnedRecipe {
    fn admit(mut config: RecipeConfig) -> Result<Self> {
        if config.args.len() > 64
            || config
                .args
                .iter()
                .any(|a| a.len() > 4096 || a.contains('\0'))
            || config.environment.len() > 16
            || config.environment.iter().any(|n| {
                n.is_empty()
                    || n.len() > 64
                    || !n
                        .bytes()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            })
        {
            return Err(error(
                ErrorCode::Bounds,
                "Gym recipe arguments or environment names exceed their bounds",
            ));
        }
        config.program = config
            .program
            .canonicalize()
            .map_err(|_| error(ErrorCode::Unavailable, "Gym executable unavailable"))?;
        config.cwd = config
            .cwd
            .canonicalize()
            .map_err(|_| error(ErrorCode::Unavailable, "Gym working directory unavailable"))?;
        let m = std::fs::symlink_metadata(&config.cwd)
            .map_err(|_| error(ErrorCode::Unavailable, "Gym working directory unavailable"))?;
        if !m.is_dir() {
            return Err(error(
                ErrorCode::Forbidden,
                "Gym working directory is not a directory",
            ));
        }
        let executable_digest = program_digest(&config.program)?;
        let revision = nostr::contracts::digest_bytes(&encoded(&(
            config.clone(),
            executable_digest.clone(),
            m.dev(),
            m.ino(),
        ))?);
        let recipe = Recipe {
            id: config.id.clone(),
            title: config.title.clone(),
            detail: config.detail.clone(),
            revision,
            budget: Budget {
                wall_ms: config.wall_ms,
                max_starts: config.max_starts,
                spend_limit_usd: None,
                spend_enforced: false,
            },
        };
        recipe.validate()?;
        Ok(Self {
            config,
            executable_digest,
            cwd_device: m.dev(),
            cwd_inode: m.ino(),
            recipe,
        })
    }
    fn current(&self) -> Result<()> {
        let m = std::fs::symlink_metadata(&self.config.cwd).map_err(|_| {
            error(
                ErrorCode::SourceChanged,
                "Gym recipe working directory changed",
            )
        })?;
        if !m.is_dir()
            || m.dev() != self.cwd_device
            || m.ino() != self.cwd_inode
            || self.config.cwd.canonicalize().ok().as_deref() != Some(&self.config.cwd)
            || program_digest(&self.config.program)? != self.executable_digest
        {
            return Err(error(
                ErrorCode::SourceChanged,
                "Gym recipe executable or working directory changed",
            ));
        }
        Ok(())
    }
}
fn program_digest(path: &Path) -> Result<String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| error(ErrorCode::SourceChanged, "Gym executable changed"))?;
    let m = file
        .metadata()
        .map_err(|_| error(ErrorCode::SourceChanged, "Gym executable changed"))?;
    if !m.is_file()
        || m.len() > 128 * 1024 * 1024
        || m.permissions().mode() & 0o111 == 0
        || m.permissions().mode() & 0o022 != 0
    {
        return Err(error(
            ErrorCode::Forbidden,
            "Gym executable must be bounded, executable, and not group or world writable",
        ));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error(ErrorCode::Unavailable, "Gym executable unreadable"))?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(error(
            ErrorCode::Bounds,
            "Gym executable exceeds its byte bound",
        ));
    }
    Ok(nostr::contracts::digest_bytes(&bytes))
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    grant: Grant,
    authorization: Event,
    roots: Vec<Root>,
    recipes: Vec<PinnedRecipe>,
    revoked: bool,
    window_start: u64,
    requests: u32,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Launch {
    grant: String,
    receipt: LaunchReceipt,
    output: Option<Output>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    stdout: String,
    stderr: String,
    stdout_bytes: u64,
    stderr_bytes: u64,
    truncated: bool,
    elapsed_ms: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    v: String,
    host: String,
    admissions: BTreeMap<String, Admission>,
    launches: BTreeMap<String, Launch>,
}
type Dispatch = (String, PinnedRecipe, String);
struct WorkerGuard {
    handles: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        if let Ok(handles) = self.handles.get_mut() {
            for h in handles.drain(..) {
                h.abort();
            }
        }
    }
}
#[derive(Clone)]
pub struct Host {
    directory: PathBuf,
    policy: RelayPolicy,
    workers: Arc<WorkerGuard>,
}
impl Host {
    pub fn new(directory: impl Into<PathBuf>, policy: RelayPolicy) -> Self {
        Self {
            directory: directory.into(),
            policy,
            workers: Arc::new(WorkerGuard {
                handles: Mutex::new(vec![]),
            }),
        }
    }
    pub fn key(&self) -> Result<SecretKey> {
        Store::open(&self.directory, false)?.key(false)
    }
    fn book(&self, store: &Store, secret: &SecretKey, create: bool) -> Result<Book> {
        let book = match store.load::<Book>()? {
            Some(book) => book,
            None if create => Book {
                v: "openagents.gym-host.v1".into(),
                host: pubkey(secret),
                admissions: BTreeMap::new(),
                launches: BTreeMap::new(),
            },
            None => return Err(error(ErrorCode::Unavailable, "Gym host is not initialized")),
        };
        if book.v != "openagents.gym-host.v1"
            || book.host != pubkey(secret)
            || book.admissions.len() > 64
            || book.launches.len() > 1024
        {
            return Err(error(
                ErrorCode::Forbidden,
                "Gym store identity or bounds differ",
            ));
        }
        for (id, admission) in &book.admissions {
            admission.grant.validate(self.policy)?;
            let original: Grant = open(
                &admission.authorization,
                secret,
                &book.host,
                &admission.grant.client,
                GRANT_SCHEMA,
            )?;
            if admission.grant.sources_digest
                != nostr::contracts::digest_bytes(&encoded(&admission.roots)?)
                || *id != admission.grant.grant
                || encoded(&original)? != encoded(&admission.grant)?
                || admission
                    .recipes
                    .iter()
                    .map(|r| &r.recipe)
                    .collect::<Vec<_>>()
                    != admission.grant.recipes.iter().collect::<Vec<_>>()
            {
                return Err(error(
                    ErrorCode::Forbidden,
                    "stored Gym authority differs from its signature",
                ));
            }
            for r in &admission.recipes {
                if r.recipe.revision
                    != nostr::contracts::digest_bytes(&encoded(&(
                        r.config.clone(),
                        r.executable_digest.clone(),
                        r.cwd_device,
                        r.cwd_inode,
                    ))?)
                {
                    return Err(error(
                        ErrorCode::Forbidden,
                        "stored Gym recipe configuration differs from its pin",
                    ));
                }
            }
        }
        Ok(book)
    }
    /// A local operator explicitly chooses both source roots and executable recipes.
    pub fn pair(
        &self,
        client: &str,
        relay: &str,
        config: Config,
        now: u64,
        expires_at: u64,
    ) -> Result<Connection> {
        self.policy.validate(relay)?;
        public(client)?;
        window(now, expires_at, GRANT_LIFETIME)?;
        if config.sources.len() > 8
            || config.recipes.len() > MAX_RECIPES
            || (config.sources.is_empty() && config.recipes.is_empty())
        {
            return Err(error(
                ErrorCode::Bounds,
                "Gym pairing needs bounded explicit sources or recipes",
            ));
        }
        let roots = config
            .sources
            .into_iter()
            .map(Root::admit)
            .collect::<Result<Vec<_>>>()?;
        let recipes = config
            .recipes
            .into_iter()
            .map(PinnedRecipe::admit)
            .collect::<Result<Vec<_>>>()?;
        let mut store = Store::open(&self.directory, true)?;
        let secret = store.key(true)?;
        let mut book = self.book(&store, &secret, true)?;
        // Keep launch tombstones through the grant lifetime; a forgotten ID
        // must never turn an ambiguous retry into a second execution.
        book.admissions
            .retain(|_, a| a.grant.expires_at.saturating_add(REQUEST_LIFETIME) > now);
        if book.admissions.len() >= 64 {
            return Err(error(
                ErrorCode::Bounds,
                "Gym grant retention limit reached",
            ));
        }
        let grant = Grant {
            v: GRANT_SCHEMA.into(),
            host: pubkey(&secret),
            client: client.into(),
            relay: relay.into(),
            grant: random_id(),
            observe: true,
            sources_digest: nostr::contracts::digest_bytes(&encoded(&roots)?),
            recipes: recipes.iter().map(|r| r.recipe.clone()).collect(),
            issued_at: now,
            expires_at,
        };
        grant.validate(self.policy)?;
        let authorization = seal(
            &grant,
            GRANT_SCHEMA,
            &secret,
            client,
            &grant.grant,
            now,
            expires_at,
        )?;
        let connection = Connection {
            v: CONNECTION_SCHEMA.into(),
            host: grant.host.clone(),
            client: client.into(),
            relay: relay.into(),
            grant: grant.grant.clone(),
            expires_at,
            authorization: authorization.clone(),
        };
        book.admissions.insert(
            grant.grant.clone(),
            Admission {
                grant,
                authorization,
                roots,
                recipes,
                revoked: false,
                window_start: now,
                requests: 0,
            },
        );
        store.save(&book)?;
        Ok(connection)
    }
    pub fn revoke(&self, grant: &str) -> Result<()> {
        let mut store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let mut book = self.book(&store, &secret, false)?;
        book.admissions
            .get_mut(grant)
            .ok_or_else(|| error(ErrorCode::Forbidden, "unknown Gym grant"))?
            .revoked = true;
        store.save(&book)
    }
    /// Invoke only when starting an exclusive host process, never for a board read.
    /// A process restart cannot establish whether previously dispatched work ended.
    pub fn recover(&self) -> Result<()> {
        let mut store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let mut book = self.book(&store, &secret, false)?;
        let mut changed = false;
        for launch in book.launches.values_mut() {
            if matches!(launch.receipt.status, Status::Queued | Status::Running) {
                launch.receipt.status = Status::Unknown;
                changed = true;
            }
        }
        if changed {
            store.save(&book)?;
        }
        Ok(())
    }
    pub fn handle_current(&self, event: &Event, relay: &str) -> Result<Event> {
        self.handle_with_clock(event, relay, unix_time)
    }
    /// Deterministic admission clock for fixture callers; production uses handle_current.
    pub fn handle(&self, event: &Event, relay: &str, now: u64) -> Result<Event> {
        self.handle_with_clock(event, relay, || Ok(now))
    }
    pub(crate) fn handle_with_clock(
        &self,
        event: &Event,
        relay: &str,
        clock: impl Fn() -> Result<u64>,
    ) -> Result<Event> {
        let now = clock()?;
        self.policy.validate(relay)?;
        let mut store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let mut book = self.book(&store, &secret, false)?;
        let request: Request = open(event, &secret, &event.pubkey, &book.host, REQUEST_SCHEMA)?;
        request.validate()?;
        fresh(request.issued_at, request.expires_at, now)?;
        if event.tag_values("h").collect::<Vec<_>>() != [request.request.as_str()] {
            return Err(error(
                ErrorCode::Forbidden,
                "Gym mailbox differs from request",
            ));
        }
        let Some(admission) = book.admissions.get_mut(&request.grant) else {
            return Err(error(ErrorCode::Forbidden, "unknown Gym grant"));
        };
        if admission.grant.client != event.pubkey
            || admission.grant.relay != relay
            || admission.authorization.id != request.authorization
            || request.expires_at > admission.grant.expires_at
        {
            return Err(error(
                ErrorCode::Forbidden,
                "Gym request differs from its admitted grant",
            ));
        }
        let refused = if admission.revoked {
            Some(ErrorCode::Revoked)
        } else if fresh(admission.grant.issued_at, admission.grant.expires_at, now).is_err() {
            Some(ErrorCode::Expired)
        } else {
            None
        };
        let mut dispatch = None;
        let response = if let Some(code) = refused {
            Response::Refused(code)
        } else {
            if now.saturating_sub(admission.window_start) >= 60 {
                admission.window_start = now;
                admission.requests = 0;
            }
            if admission.requests >= 120 {
                Response::Refused(ErrorCode::RateLimited)
            } else {
                admission.requests += 1;
                let admission = admission.clone();
                match self.operation(&mut book, &admission, &request, now) {
                    Ok((response, job)) => {
                        dispatch = job;
                        response
                    }
                    Err(e) => Response::Refused(e.code),
                }
            }
        };
        // Observation may read files; expiry is checked again at the last
        // admission point. The explicit test clock is not caller-controlled.
        let final_now = clock()?;
        if final_now >= request.expires_at {
            return Err(error(
                ErrorCode::Expired,
                "Gym request expired during admission",
            ));
        }
        store.save(&book)?;
        if let Some((key, recipe, run_id)) = dispatch {
            let directory = self.directory.clone();
            let policy = self.policy;
            let task = tokio::spawn(async move {
                run_recipe(directory, policy, key, recipe, run_id).await;
            });
            let mut handles =
                self.workers.handles.lock().map_err(|_| {
                    error(ErrorCode::Unavailable, "Gym worker tracking unavailable")
                })?;
            handles.retain(|h| !h.is_finished());
            handles.push(task);
        }
        let reply = Reply {
            v: REPLY_SCHEMA.into(),
            request: request.request.clone(),
            request_event: event.id.clone(),
            grant: request.grant,
            issued_at: final_now,
            expires_at: request.expires_at,
            response,
        };
        seal(
            &reply,
            REPLY_SCHEMA,
            &secret,
            &event.pubkey,
            &request.request,
            final_now,
            request.expires_at,
        )
    }
    fn operation(
        &self,
        book: &mut Book,
        admission: &Admission,
        request: &Request,
        now: u64,
    ) -> Result<(Response, Option<Dispatch>)> {
        match &request.query {
            Query::Snapshot => {
                let mut snapshot = sources::snapshot(&admission.roots, now)?;
                snapshot.recipes = admission.grant.recipes.clone();
                for launch in book
                    .launches
                    .values()
                    .filter(|l| l.grant == admission.grant.grant)
                    .rev()
                    .take(16)
                {
                    let receipt = &launch.receipt;
                    let recipe = admission
                        .grant
                        .recipes
                        .iter()
                        .find(|r| r.id == receipt.recipe_id);
                    snapshot.runs.insert(0,Run{id:receipt.run_id.clone(),title:recipe.map_or_else(||receipt.recipe_id.clone(),|r|r.title.clone()),category:Category::Evaluation,status:receipt.status,completed:receipt.finished_at.map(|_|1),total:Some(1),cost_usd:None,elapsed_ms:launch.output.as_ref().map(|o|o.elapsed_ms),metrics:vec![],source:"Gym recipe host".into(),provenance:"Durable local launch receipt; process exit is not benchmark success; cost unknown".into()});
                }
                snapshot.runs.truncate(MAX_RUNS);
                snapshot.validate(now)?;
                Ok((Response::Snapshot(Box::new(snapshot)), None))
            }
            Query::Launch {
                request_id,
                recipe_id,
                revision,
            } => {
                let key = format!("{}:{request_id}", admission.grant.grant);
                if let Some(old) = book.launches.get(&key) {
                    if old.receipt.recipe_id != *recipe_id || old.receipt.revision != *revision {
                        return Err(error(
                            ErrorCode::Conflict,
                            "launch ID already binds different work",
                        ));
                    }
                    return Ok((Response::Launch(old.receipt.clone()), None));
                }
                let recipe = admission
                    .recipes
                    .iter()
                    .find(|r| r.recipe.id == *recipe_id && r.recipe.revision == *revision)
                    .ok_or_else(|| error(ErrorCode::Forbidden, "recipe revision is not granted"))?;
                recipe.current()?;
                if tokio::runtime::Handle::try_current().is_err() {
                    return Err(error(
                        ErrorCode::Unavailable,
                        "Gym launch needs a supervised host runtime",
                    ));
                }
                if book.launches.len() >= 1024
                    || book.launches.values().any(|l| {
                        matches!(
                            l.receipt.status,
                            Status::Queued | Status::Running | Status::Unknown
                        )
                    })
                {
                    return Err(error(
                        ErrorCode::Conflict,
                        "Gym has an active or unresolved launch; operator reconciliation is required",
                    ));
                }
                let count = book
                    .launches
                    .values()
                    .filter(|l| {
                        l.grant == admission.grant.grant && l.receipt.recipe_id == *recipe_id
                    })
                    .count();
                if count >= recipe.recipe.budget.max_starts as usize {
                    return Err(error(
                        ErrorCode::Bounds,
                        "Gym recipe launch allowance exhausted",
                    ));
                }
                let receipt = LaunchReceipt {
                    request_id: request_id.clone(),
                    run_id: random_id(),
                    recipe_id: recipe_id.clone(),
                    revision: revision.clone(),
                    status: Status::Running,
                    submitted_at: now,
                    finished_at: None,
                    exit_code: None,
                };
                book.launches.insert(
                    key.clone(),
                    Launch {
                        grant: admission.grant.grant.clone(),
                        receipt: receipt.clone(),
                        output: None,
                    },
                );
                Ok((
                    Response::Launch(receipt.clone()),
                    Some((key, recipe.clone(), receipt.run_id)),
                ))
            }
        }
    }
}
async fn run_recipe(
    directory: PathBuf,
    policy: RelayPolicy,
    key: String,
    recipe: PinnedRecipe,
    run_id: String,
) {
    // The durable intent already exists. Any failure before final settlement
    // leaves it unresolved; restarting the host never repeats the command.
    let mut command = std::process::Command::new(&recipe.config.program);
    command
        .args(&recipe.config.args)
        .current_dir(&recipe.config.cwd)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/local/bin")
        .env("GYM_BRIDGE_RUN_ID", run_id);
    for name in &recipe.config.environment {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let ended = supervise::Job::from_command(command)
        .bounded(
            supervise::Limits::within(Duration::from_millis(recipe.config.wall_ms))
                .keeping(16 * 1024),
        )
        .run()
        .await;
    let finish = || -> Result<()> {
        let host = Host::new(directory.clone(), policy);
        let mut store = Store::open(&directory, false)?;
        let secret = store.key(false)?;
        let mut book = host.book(&store, &secret, false)?;
        let launch = book
            .launches
            .get_mut(&key)
            .ok_or_else(|| error(ErrorCode::Unavailable, "Gym launch receipt disappeared"))?;
        if launch.receipt.status != Status::Running {
            return Err(error(
                ErrorCode::Conflict,
                "Gym launch was independently reconciled",
            ));
        }
        launch.receipt.status = if ended.ending.success() {
            Status::Completed
        } else {
            Status::Failed
        };
        launch.receipt.finished_at = Some(unix_time()?);
        launch.receipt.exit_code = ended.ending.code();
        launch.output = Some(Output {
            stdout: ended.stdout.text.clone(),
            stderr: ended.stderr.text.clone(),
            stdout_bytes: ended.stdout.bytes,
            stderr_bytes: ended.stderr.bytes,
            truncated: ended.stdout.truncated || ended.stderr.truncated,
            elapsed_ms: u64::try_from(ended.elapsed.as_millis()).unwrap_or(u64::MAX),
        });
        store.save(&book)
    };
    // A short competing revoke/read lock is retried; a persistent storage
    // failure remains an unresolved retained intent, not a fabricated result.
    for _ in 0..20 {
        match finish() {
            Ok(()) => return,
            Err(e) if e.code == ErrorCode::Conflict => {
                tokio::time::sleep(Duration::from_millis(25)).await
            }
            Err(_) => return,
        }
    }
}
