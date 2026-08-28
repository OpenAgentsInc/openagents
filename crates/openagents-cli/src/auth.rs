//! Authentication: endpoint resolution, secret handling, the credential store,
//! the browser device-authorization flow, and the pending-authorization state.
//!
//! The rule this module is written around: a credential command that cannot
//! reach its data refuses. It never returns a plausible token, never reports an
//! account it did not read from the server, and never treats a store that
//! answered with an error as a store that answered "nothing is there". An
//! invented credential does not fail here — it fails much later, inside some
//! unrelated request, or worse, appears to succeed.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zeroize::Zeroize;

/// A refusal. Every variant carries the sentence the CLI prints after `oa: `.
#[derive(Debug)]
pub struct AuthError(pub String);

impl AuthError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AuthError {}

// ---------------------------------------------------------------------------
// secrets
// ---------------------------------------------------------------------------

/// A token held in memory.
///
/// Two properties matter and both are enforced here rather than at every call
/// site: the value is wiped when it is dropped, and neither `Debug` nor
/// `Display` can leak it into a log line, a panic message, or a `{:?}` in some
/// future edit. There is deliberately no `Display`.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The only way to read the value. Named so that every use is visible in a
    /// grep for `expose`.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Wipe the buffer now rather than at drop.
    ///
    /// `oa auth logout` calls this before it updates the credential file, so
    /// the plaintext is gone from this process even if deletion fails.
    pub fn zeroize_now(&mut self) {
        self.0.zeroize();
    }

    /// The prefix a person can compare against without the value leaving the
    /// machine. Never widen this.
    pub fn fingerprint(&self) -> String {
        let head: String = self.0.chars().take(11).collect();
        format!("{head}…")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

// ---------------------------------------------------------------------------
// endpoint
// ---------------------------------------------------------------------------

pub const PRODUCTION_ORIGIN: &str = "https://openagents.com";
pub const STAGING_ORIGIN: &str = "https://staging.openagents.com";
pub const LOCAL_ORIGIN: &str = "http://localhost:4000";

/// The API this invocation talks to, and the name for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub origin: String,
    pub profile: String,
}

fn loopback_hostname(hostname: &str) -> bool {
    let normalized = hostname.to_ascii_lowercase();
    normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized.starts_with("127.")
        || normalized == "::1"
        || normalized == "[::1]"
        || normalized == "host.docker.internal"
}

/// Reduce a URL to a bare origin, refusing anything that is not one.
///
/// Credentials are stored per origin, so a URL carrying a path, a query, or an
/// embedded username would silently key the store under something other than
/// the authority the token is good for.
pub fn normalize_api_origin(input: &str) -> Result<String, AuthError> {
    let value = input.trim();
    if value.is_empty() {
        return Err(AuthError::new("the API URL cannot be empty"));
    }
    let url = reqwest::Url::parse(value)
        .map_err(|_| AuthError::new(format!("invalid API URL: {value}")))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AuthError::new("the API URL cannot contain credentials"));
    }
    if url.path() != "/" && !url.path().is_empty() {
        return Err(AuthError::new(
            "the API URL must be an origin without a path, query, or fragment",
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(AuthError::new(
            "the API URL must be an origin without a path, query, or fragment",
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| AuthError::new(format!("invalid API URL: {value}")))?;
    match url.scheme() {
        "https" => {}
        "http" if loopback_hostname(host) => {}
        _ => {
            return Err(AuthError::new(
                "API URLs must use HTTPS. HTTP is allowed only for loopback development",
            ));
        }
    }
    Ok(match url.port() {
        Some(port) => format!("{}://{}:{}", url.scheme(), host, port),
        None => format!("{}://{}", url.scheme(), host),
    })
}

fn profile_origin(profile: &str) -> Option<&'static str> {
    match profile {
        "production" => Some(PRODUCTION_ORIGIN),
        "staging" => Some(STAGING_ORIGIN),
        "local" => Some(LOCAL_ORIGIN),
        _ => None,
    }
}

fn profile_for_origin(origin: &str) -> String {
    match origin {
        PRODUCTION_ORIGIN => "production".to_string(),
        STAGING_ORIGIN => "staging".to_string(),
        LOCAL_ORIGIN => "local".to_string(),
        _ => "custom".to_string(),
    }
}

/// Resolve the endpoint from the flags and the environment.
///
/// Precedence: `--api-url`, then `--profile`, then `OPENAGENTS_API_URL`, then
/// `OPENAGENTS_PROFILE`, then production.
pub fn resolve_endpoint(
    api_url: Option<&str>,
    profile: Option<&str>,
) -> Result<Endpoint, AuthError> {
    if api_url.is_some() && profile.is_some() {
        return Err(AuthError::new(
            "use either --api-url or --profile, not both",
        ));
    }
    if let Some(url) = api_url {
        let origin = normalize_api_origin(url)?;
        let name = profile_for_origin(&origin);
        return Ok(Endpoint {
            origin,
            profile: name,
        });
    }
    if let Some(name) = profile {
        let origin = profile_origin(name).ok_or_else(|| {
            AuthError::new(format!(
                "unknown profile {name}. Use production, staging, or local"
            ))
        })?;
        return Ok(Endpoint {
            origin: origin.to_string(),
            profile: name.to_string(),
        });
    }
    if let Ok(url) = std::env::var("OPENAGENTS_API_URL") {
        if !url.trim().is_empty() {
            let origin = normalize_api_origin(&url)?;
            let name = profile_for_origin(&origin);
            return Ok(Endpoint {
                origin,
                profile: name,
            });
        }
    }
    if let Ok(name) = std::env::var("OPENAGENTS_PROFILE") {
        if !name.trim().is_empty() {
            let name = name.trim();
            let origin = profile_origin(name).ok_or_else(|| {
                AuthError::new(format!(
                    "unknown OPENAGENTS_PROFILE {name}. Use production, staging, or local"
                ))
            })?;
            return Ok(Endpoint {
                origin: origin.to_string(),
                profile: name.to_string(),
            });
        }
    }
    Ok(Endpoint {
        origin: PRODUCTION_ORIGIN.to_string(),
        profile: "production".to_string(),
    })
}

/// The `oa auth login` a person pointed at this endpoint has to type.
///
/// A hint that drops the endpoint selection sends the reader around a loop that
/// never signs them in: the login would store a token for production while the
/// session keeps reading the one for staging.
pub fn login_command_for(endpoint: &Endpoint) -> String {
    match endpoint.profile.as_str() {
        "production" => "oa auth login".to_string(),
        "custom" => format!("oa --api-url {} auth login", endpoint.origin),
        other => format!("oa --profile {other} auth login"),
    }
}

pub fn resume_command_for(endpoint: &Endpoint) -> String {
    match endpoint.profile.as_str() {
        "production" => "oa auth login --resume".to_string(),
        "custom" => format!("oa --api-url {} auth login --resume", endpoint.origin),
        other => format!("oa --profile {other} auth login --resume"),
    }
}

pub fn connect_github_command_for(endpoint: &Endpoint) -> String {
    match endpoint.profile.as_str() {
        "production" => "oa auth connect-github".to_string(),
        "custom" => format!("oa --api-url {} auth connect-github", endpoint.origin),
        other => format!("oa --profile {other} auth connect-github"),
    }
}

pub fn connect_github_resume_command_for(endpoint: &Endpoint) -> String {
    match endpoint.profile.as_str() {
        "production" => "oa auth connect-github --resume".to_string(),
        "custom" => format!(
            "oa --api-url {} auth connect-github --resume",
            endpoint.origin
        ),
        other => format!("oa --profile {other} auth connect-github --resume"),
    }
}

/// The connect command as an origin-only string, for error text raised where
/// no `Endpoint` is in hand (the repository client, which carries just the
/// origin). A custom origin needs `--api-url`; anything else is production.
pub fn connect_github_command_for_origin(origin: &str) -> String {
    if origin == PRODUCTION_ORIGIN {
        "oa auth connect-github".to_string()
    } else {
        format!("oa --api-url {origin} auth connect-github")
    }
}

/// The `kind` recorded for a pending GitHub connect authorization, so
/// `--resume` can refuse the record a login wrote instead of waiting on it.
pub const GITHUB_CONNECT_PENDING_KIND: &str = "github_connect";

// ---------------------------------------------------------------------------
// on-disk paths
// ---------------------------------------------------------------------------

pub fn home_directory() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

/// `~/.config/openagents`, the directory the TypeScript CLI already uses. Both
/// binaries read each other's state, so a person can switch between them
/// without signing in twice.
pub fn config_directory() -> PathBuf {
    home_directory().join(".config").join("openagents")
}

/// The one cross-platform credential file.
pub fn credentials_path() -> PathBuf {
    credentials_path_for_home(&home_directory())
}

fn credentials_path_for_home(home: &Path) -> PathBuf {
    home.join(".openagents").join("credentials.json")
}

/// The file path released clients used before `0.0.19`.
pub(crate) fn previous_credentials_path() -> PathBuf {
    config_directory().join("cli-credentials.json")
}

/// Move the previous file store into the current location without exposing
/// its contents. A rename handles the usual same-filesystem case atomically;
/// the staged copy handles uncommon split mounts.
pub(crate) fn migrate_credential_file(previous: &Path, current: &Path) -> Result<(), AuthError> {
    if current.exists() || !previous.exists() {
        return Ok(());
    }
    let parent = current
        .parent()
        .ok_or_else(|| AuthError::new(format!("{} has no parent directory", current.display())))?;
    ensure_private_directory(parent)?;

    match fs::rename(previous, current) {
        Ok(()) => restrict_private_file(current),
        Err(_) if current.exists() => Ok(()),
        Err(_) => {
            let contents = fs::read_to_string(previous).map_err(|error| {
                AuthError::new(format!("could not read {}: {error}", previous.display()))
            })?;
            write_private_file(current, &contents)?;
            match fs::remove_file(previous) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(AuthError::new(format!(
                    "could not remove {} after migration: {error}",
                    previous.display()
                ))),
            }
        }
    }
}

pub fn device_authorizations_path() -> PathBuf {
    config_directory().join("device-authorizations.json")
}

/// Create a directory 0700, or fail saying which one.
fn ensure_private_directory(directory: &Path) -> Result<(), AuthError> {
    fs::create_dir_all(directory).map_err(|error| {
        AuthError::new(format!("could not create {}: {error}", directory.display()))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).map_err(|error| {
            AuthError::new(format!(
                "could not restrict {} to 0700: {error}",
                directory.display()
            ))
        })?;
    }
    Ok(())
}

/// The suffix every staging file carries, so a sweep can recognise one.
pub(crate) const TEMP_SUFFIX: &str = ".tmp";

/// A staging path beside `path` that no other writer can be using.
///
/// The name a staged write goes through must not be shared. A fixed one —
/// `path.with_extension("tmp")` — gives every process writing this file the
/// same staging path, so two `oa` runs over one config directory truncate and
/// rename each other's half-written bytes: one wins, and the other's `rename`
/// finds nothing and reports a failed credential write for a credential that
/// may in fact have been stored. That is the worst shape a credential error
/// can take, because the caller cannot tell what is on disk. A fleet of agents
/// under one `$HOME` is the normal way this CLI runs, so the overlap is not a
/// corner case.
///
/// Process id, wall clock, and a per-process counter make the name unique
/// across processes, across threads, and across calls on one thread. The
/// original file name stays in it so a human reading the directory knows what
/// crashed, and so that path guards matching on `credentials.json` still match
/// the staging file.
pub(crate) fn unique_temp_path(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or(0);
    let ordinal = COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(
        ".{name}.{}.{nanos}.{ordinal}{TEMP_SUFFIX}",
        std::process::id()
    ))
}

/// Write a file 0600 through a temporary file in the same directory.
///
/// Staged and renamed, so a reader never sees half a file and a crash leaves
/// the previous contents intact. The staging name is unique per call — see
/// [`unique_temp_path`] — which is what makes concurrent writers safe rather
/// than merely atomic for one.
fn write_private_file(path: &Path, contents: &str) -> Result<(), AuthError> {
    let parent = path
        .parent()
        .ok_or_else(|| AuthError::new(format!("{} has no parent directory", path.display())))?;
    ensure_private_directory(parent)?;
    let temporary = unique_temp_path(path);
    let staged = stage_private_file(&temporary, contents);
    if staged.is_err() {
        // The name was ours alone, so removing it can strand nobody else's
        // write. Leaving it would litter the config directory once per failure.
        let _ = fs::remove_file(&temporary);
    }
    staged?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        AuthError::new(format!("could not write {}: {error}", path.display()))
    })
}

fn restrict_private_file(path: &Path) -> Result<(), AuthError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            AuthError::new(format!(
                "could not restrict {} to 0600: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

/// Create the staging file 0600 and put `contents` in it.
fn stage_private_file(temporary: &Path, contents: &str) -> Result<(), AuthError> {
    let mut options = fs::OpenOptions::new();
    // `create_new` rather than `truncate`: the name is unique to this call, so
    // finding one already there means the assumption broke and the write must
    // refuse rather than trample whatever is in it.
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(temporary).map_err(|error| {
        AuthError::new(format!("could not write {}: {error}", temporary.display()))
    })?;
    file.write_all(contents.as_bytes()).map_err(|error| {
        AuthError::new(format!("could not write {}: {error}", temporary.display()))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(temporary, fs::Permissions::from_mode(0o600)).map_err(|error| {
            AuthError::new(format!(
                "could not restrict {} to 0600: {error}",
                temporary.display()
            ))
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// legacy profile configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: std::collections::HashMap<String, ProfileConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    pub api_url: Option<String>,
    pub token: Option<String>,
    pub identity_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialFile {
    version: u8,
    #[serde(default)]
    tokens: std::collections::BTreeMap<String, String>,
}

// ---------------------------------------------------------------------------
// credential store
// ---------------------------------------------------------------------------

/// Where a token was read from. `oa auth status` reports this, because "the
/// environment overrides the store" is the single most common reason a person
/// is signed in as somebody they did not expect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    Environment,
    File,
    LegacyConfig,
}

impl TokenSource {
    pub fn label(self) -> &'static str {
        match self {
            TokenSource::Environment => "environment",
            TokenSource::File => "file",
            TokenSource::LegacyConfig => "legacy config",
        }
    }
}

pub struct StoredToken {
    pub token: Secret,
    pub source: TokenSource,
}

impl fmt::Debug for StoredToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredToken")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

pub struct CredentialStore {
    origin: String,
    credentials_path: PathBuf,
    previous_credentials_path: Option<PathBuf>,
    legacy_config_path: PathBuf,
}

impl CredentialStore {
    pub fn default_path() -> PathBuf {
        home_directory().join(".openagents").join("config.json")
    }

    /// The store for production, reading the legacy profile file at `path`.
    /// Kept because the rest of the CLI constructs the store this way.
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            origin: PRODUCTION_ORIGIN.to_string(),
            credentials_path: credentials_path(),
            previous_credentials_path: Some(previous_credentials_path()),
            legacy_config_path: path.unwrap_or_else(Self::default_path),
        }
    }

    pub fn for_origin(origin: &str) -> Self {
        Self {
            origin: origin.to_string(),
            credentials_path: credentials_path(),
            previous_credentials_path: Some(previous_credentials_path()),
            legacy_config_path: Self::default_path(),
        }
    }

    /// A store confined to a directory. Tests use it to exercise the real
    /// read, write, and delete paths without touching the owner's credentials.
    pub fn isolated(origin: &str, directory: &Path) -> Self {
        Self {
            origin: origin.to_string(),
            credentials_path: directory.join("credentials.json"),
            previous_credentials_path: None,
            legacy_config_path: directory.join("config.json"),
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    // -- legacy profile file -------------------------------------------------

    pub fn load(&self) -> Result<AuthConfig, Box<dyn std::error::Error>> {
        if !self.legacy_config_path.exists() {
            return Ok(AuthConfig {
                default_profile: Some("default".to_string()),
                profiles: std::collections::HashMap::new(),
            });
        }
        let data = fs::read_to_string(&self.legacy_config_path)?;
        Ok(serde_json::from_str(&data)?)
    }

    pub fn save(&self, config: &AuthConfig) -> Result<(), Box<dyn std::error::Error>> {
        write_private_file(
            &self.legacy_config_path,
            &serde_json::to_string_pretty(config)?,
        )?;
        Ok(())
    }

    // -- credentials.json ----------------------------------------------------

    fn load_credential_file(&self) -> Result<CredentialFile, AuthError> {
        if let Some(previous) = &self.previous_credentials_path {
            migrate_credential_file(previous, &self.credentials_path)?;
        }
        if !self.credentials_path.exists() {
            return Ok(CredentialFile {
                version: 1,
                tokens: Default::default(),
            });
        }
        let text = fs::read_to_string(&self.credentials_path).map_err(|error| {
            AuthError::new(format!(
                "could not read {}: {error}",
                self.credentials_path.display()
            ))
        })?;
        serde_json::from_str(&text).map_err(|error| {
            AuthError::new(format!(
                "could not decode {}: {error}",
                self.credentials_path.display()
            ))
        })
    }

    fn save_credential_file(&self, file: &CredentialFile) -> Result<(), AuthError> {
        if file.tokens.is_empty() {
            if self.credentials_path.exists() {
                fs::remove_file(&self.credentials_path).map_err(|error| {
                    AuthError::new(format!(
                        "could not remove {}: {error}",
                        self.credentials_path.display()
                    ))
                })?;
            }
            return Ok(());
        }
        let encoded = serde_json::to_string(file)
            .map_err(|error| AuthError::new(format!("could not encode credentials: {error}")))?;
        write_private_file(&self.credentials_path, &encoded)
    }

    // -- public API ----------------------------------------------------------

    /// Find the token for this endpoint, refusing when the file cannot be read.
    pub fn find_token(&self) -> Result<Option<StoredToken>, AuthError> {
        if let Ok(value) = std::env::var("OPENAGENTS_TOKEN") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(Some(StoredToken {
                    token: Secret::new(trimmed),
                    source: TokenSource::Environment,
                }));
            }
        }
        let file = self.load_credential_file()?;
        if let Some(value) = file.tokens.get(&self.origin) {
            if !value.trim().is_empty() {
                return Ok(Some(StoredToken {
                    token: Secret::new(value.trim()),
                    source: TokenSource::File,
                }));
            }
        }
        // The legacy profile file predates per-endpoint keying. Its token is
        // admitted only when the profile names this origin, or names none and
        // this origin is production — the assumption the old file was written
        // under. Reading it unconditionally would hand a production token to a
        // session pointed at staging or at a developer's own server.
        if let Ok(config) = self.load() {
            let key = config
                .default_profile
                .clone()
                .unwrap_or_else(|| "default".to_string());
            if let Some(profile) = config.profiles.get(&key) {
                let profile_origin = profile
                    .api_url
                    .as_deref()
                    .map(|url| normalize_api_origin(url).unwrap_or_else(|_| url.to_string()));
                let admitted = match profile_origin {
                    Some(origin) => origin == self.origin,
                    None => self.origin == PRODUCTION_ORIGIN,
                };
                if admitted {
                    if let Some(value) = profile.token.clone() {
                        if !value.trim().is_empty() {
                            return Ok(Some(StoredToken {
                                token: Secret::new(value.trim()),
                                source: TokenSource::LegacyConfig,
                            }));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// The lenient read the rest of the CLI uses when it wants a bearer token
    /// for an unrelated command. Auth and repository commands call
    /// [`CredentialStore::find_token`] instead.
    pub fn get_token(&self) -> Option<String> {
        self.find_token()
            .ok()
            .flatten()
            .map(|stored| stored.token.expose().to_string())
    }

    /// Store a token for this endpoint, and say where it landed.
    pub fn store(&self, token: &Secret) -> Result<TokenSource, AuthError> {
        if token.is_empty() {
            return Err(AuthError::new("refusing to store an empty token"));
        }
        // Another CLI process can update a different endpoint in the same
        // file between this write and its read-back. Merge and retry instead
        // of claiming success for a value that did not reach disk.
        for _ in 0..32 {
            let mut file = self.load_credential_file()?;
            file.tokens
                .insert(self.origin.clone(), token.expose().to_string());
            self.save_credential_file(&file)?;
            let stored = self
                .load_credential_file()?
                .tokens
                .get(&self.origin)
                .cloned();
            if stored.as_deref() == Some(token.expose()) {
                return Ok(TokenSource::File);
            }
            std::thread::yield_now();
        }
        Err(AuthError::new(format!(
            "could not verify credentials in {} after concurrent writes",
            self.credentials_path.display()
        )))
    }

    pub fn set_token(&self, token: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.store(&Secret::new(token))?;
        Ok(())
    }

    /// Remove the token for this endpoint.
    ///
    /// The in-memory copy is wiped before the file is updated, so the plaintext
    /// is gone from this process even if deletion fails.
    pub fn remove(&self) -> Result<bool, AuthError> {
        let held = self.find_token()?;
        let had_token = held.is_some();
        if let Some(mut stored) = held {
            stored.token.zeroize_now();
            debug_assert!(stored.token.is_empty());
            drop(stored);
        }

        let mut file = self.load_credential_file()?;
        if file.tokens.remove(&self.origin).is_some() {
            self.save_credential_file(&file)?;
        }

        if let Ok(mut config) = self.load() {
            let key = config
                .default_profile
                .clone()
                .unwrap_or_else(|| "default".to_string());
            if let Some(profile) = config.profiles.get_mut(&key) {
                if let Some(mut token) = profile.token.take() {
                    token.zeroize();
                }
                let _ = self.save(&config);
            }
        }
        Ok(had_token)
    }

    pub fn clear_token(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.remove()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// device authorization
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuthorization {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i64,
    pub interval: u64,
    /// The scopes this authorization was opened for, as the server settled
    /// them.
    ///
    /// The server's answer, not the request: `--scope` asks, and the
    /// deployment decides — an unknown scope is refused outright, and asking
    /// for nothing takes the server's own default set. Carried here and
    /// reported so `--scope` has a visible effect. Without it the flag reached
    /// the wire and nothing a reader could see ever changed, which is
    /// indistinguishable from the flag not being read at all.
    #[serde(default)]
    pub scope: Option<String>,
}

/// One poll of the token endpoint.
#[derive(Debug)]
pub enum DevicePoll {
    Granted,
    Pending,
    SlowDown,
}

/// One poll of a GitHub connect authorization.
#[derive(Debug)]
pub enum ConnectPoll {
    /// The server accepted the approval and kept the credential: the answer
    /// names the GitHub account, and carries no token to store.
    Connected(String),
    Pending,
    SlowDown,
}

pub struct DeviceClient {
    origin: String,
    http: reqwest::Client,
}

impl DeviceClient {
    pub fn new(origin: &str) -> Self {
        Self {
            origin: origin.trim_end_matches('/').to_string(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Ask the server to open an authorization. The server decides the default
    /// scope set; naming scopes here names exactly what the approval page shows.
    pub async fn start(&self, scopes: &[String]) -> Result<DeviceAuthorization, AuthError> {
        let url = format!("{}/api/v1/device/authorizations", self.origin);
        let body = device_authorization_body(scopes, local_device_name());
        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|error| AuthError::new(format!("could not reach {}: {error}", self.origin)))?;
        let status = response.status();
        let value: serde_json::Value = response.json().await.map_err(|error| {
            AuthError::new(format!(
                "could not read the authorization response: {error}"
            ))
        })?;
        if status.as_u16() != 201 {
            return Err(AuthError::new(format!(
                "{} could not start CLI authorization ({}{})",
                self.origin,
                status.as_u16(),
                api_error_detail(&value)
            )));
        }
        serde_json::from_value(value).map_err(|error| {
            AuthError::new(format!(
                "the device authorization response did not match the API contract: {error}"
            ))
        })
    }

    /// Ask the server to open a GitHub connect authorization. Same endpoint
    /// and contract as [`DeviceClient::start`], with `kind: "github_connect"`
    /// selecting the server's repository-scope flow: the approval page grants
    /// `repo, read:org` to the signed-in account, and the credential stays
    /// server-side.
    pub async fn start_github_connect(&self) -> Result<DeviceAuthorization, AuthError> {
        let url = format!("{}/api/v1/device/authorizations", self.origin);
        let mut body = device_authorization_body(&[], local_device_name());
        if let serde_json::Value::Object(ref mut map) = body {
            map.insert(
                "kind".to_string(),
                serde_json::Value::String("github_connect".to_string()),
            );
        }
        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|error| AuthError::new(format!("could not reach {}: {error}", self.origin)))?;
        let status = response.status();
        let value: serde_json::Value = response.json().await.map_err(|error| {
            AuthError::new(format!(
                "could not read the authorization response: {error}"
            ))
        })?;
        if status.as_u16() != 201 {
            return Err(AuthError::new(format!(
                "{} could not start the GitHub connect authorization ({}{})",
                self.origin,
                status.as_u16(),
                api_error_detail(&value)
            )));
        }
        serde_json::from_value(value).map_err(|error| {
            AuthError::new(format!(
                "the GitHub connect authorization response did not match the API contract: {error}"
            ))
        })
    }

    /// One poll. Distinguishes "not yet approved" from "denied", because the
    /// first is a reason to wait and the second is a reason to stop.
    pub async fn poll(
        &self,
        device_code: &str,
        into: &mut Option<Secret>,
    ) -> Result<DevicePoll, AuthError> {
        let url = format!("{}/api/v1/device/authorizations/token", self.origin);
        let response = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "device_code": device_code }))
            .send()
            .await
            .map_err(|error| AuthError::new(format!("could not reach {}: {error}", self.origin)))?;
        let status = response.status().as_u16();
        let value: serde_json::Value = response.json().await.map_err(|error| {
            AuthError::new(format!(
                "could not read the authorization response: {error}"
            ))
        })?;
        if status == 200 {
            let access = value
                .get("access_token")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AuthError::new(
                        "the device token response did not match the API contract: no access_token",
                    )
                })?;
            *into = Some(Secret::new(access));
            return Ok(DevicePoll::Granted);
        }
        let code = value.get("code").and_then(|v| v.as_str()).unwrap_or("");
        match (status, code) {
            (428, "authorization_pending") => Ok(DevicePoll::Pending),
            (429, "slow_down") => Ok(DevicePoll::SlowDown),
            _ => Err(AuthError::new(format!(
                "CLI authorization was denied, expired, or already claimed ({status}{})",
                api_error_detail(&value)
            ))),
        }
    }

    /// Poll until the request is approved, denied, or expires.
    pub async fn wait(&self, authorization: &DeviceAuthorization) -> Result<Secret, AuthError> {
        let mut interval = authorization.interval.max(1);
        let deadline =
            SystemTime::now() + Duration::from_secs(authorization.expires_in.max(1) as u64);
        loop {
            let mut received: Option<Secret> = None;
            match self.poll(&authorization.device_code, &mut received).await? {
                DevicePoll::Granted => {
                    return received.ok_or_else(|| {
                        AuthError::new("the device token response carried no token")
                    });
                }
                DevicePoll::Pending => {}
                DevicePoll::SlowDown => interval += 5,
            }
            if SystemTime::now() >= deadline {
                return Err(AuthError::new(
                    "CLI authorization expired before it was approved",
                ));
            }
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }

    /// One poll of a GitHub connect authorization. The approved answer is
    /// `200` with `status: "connected"` and a `github_login` — deliberately
    /// unlike the token poll, so a server bug that handed back an access
    /// token here cannot be mistaken for a connect answer.
    pub async fn poll_connect(&self, device_code: &str) -> Result<ConnectPoll, AuthError> {
        let url = format!("{}/api/v1/device/authorizations/token", self.origin);
        let response = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "device_code": device_code }))
            .send()
            .await
            .map_err(|error| AuthError::new(format!("could not reach {}: {error}", self.origin)))?;
        let status = response.status().as_u16();
        let value: serde_json::Value = response.json().await.map_err(|error| {
            AuthError::new(format!(
                "could not read the authorization response: {error}"
            ))
        })?;
        if status == 200 {
            if value.get("status").and_then(|v| v.as_str()) == Some("connected") {
                let login = value
                    .get("github_login")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                return Ok(ConnectPoll::Connected(login));
            }
            return Err(AuthError::new(
                "the GitHub connect response did not match the API contract: \
                 a 200 without status \"connected\"",
            ));
        }
        let code = value.get("code").and_then(|v| v.as_str()).unwrap_or("");
        match (status, code) {
            (428, "authorization_pending") => Ok(ConnectPoll::Pending),
            (429, "slow_down") => Ok(ConnectPoll::SlowDown),
            _ => Err(AuthError::new(format!(
                "the GitHub connect authorization was denied, expired, or already \
claimed ({status}{})",
                api_error_detail(&value)
            ))),
        }
    }

    /// Poll until the GitHub connect request is approved, denied, or expires.
    pub async fn wait_for_connect(
        &self,
        authorization: &DeviceAuthorization,
    ) -> Result<String, AuthError> {
        let mut interval = authorization.interval.max(1);
        let deadline =
            SystemTime::now() + Duration::from_secs(authorization.expires_in.max(1) as u64);
        loop {
            match self.poll_connect(&authorization.device_code).await? {
                ConnectPoll::Connected(login) => return Ok(login),
                ConnectPoll::Pending => {}
                ConnectPoll::SlowDown => interval += 5,
            }
            if SystemTime::now() >= deadline {
                return Err(AuthError::new(
                    "the GitHub connect authorization expired before it was approved",
                ));
            }
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }
}

/// Build the anonymous authorization request without putting local metadata in
/// authority-bearing fields. The computer name is shown to the approver; it
/// does not change the scopes or token the server may grant.
fn device_authorization_body(scopes: &[String], device_name: Option<String>) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    if !scopes.is_empty() {
        body.insert("scope".to_string(), scopes.join(" ").into());
    }
    if let Some(device_name) = device_name.and_then(normalize_device_name) {
        body.insert("device_name".to_string(), device_name.into());
    }
    serde_json::Value::Object(body)
}

fn normalize_device_name(name: String) -> Option<String> {
    let name = name
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let name = name.trim().chars().take(80).collect::<String>();
    (!name.is_empty()).then_some(name)
}

#[cfg(unix)]
fn local_device_name() -> Option<String> {
    let mut bytes = [0_u8; 256];
    // SAFETY: `bytes` is a writable buffer for exactly the length supplied,
    // and it lives until `gethostname` returns.
    if unsafe { libc::gethostname(bytes.as_mut_ptr().cast(), bytes.len()) } != 0 {
        return None;
    }
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    normalize_device_name(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

#[cfg(windows)]
fn local_device_name() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .ok()
        .and_then(normalize_device_name)
}

#[cfg(not(any(unix, windows)))]
fn local_device_name() -> Option<String> {
    None
}

/// The parenthetical the CLI appends to an API refusal: the server's own code
/// and request id when it sent them, and nothing invented when it did not.
pub fn api_error_detail(value: &serde_json::Value) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(code) = value.get("code").and_then(|v| v.as_str()) {
        parts.push(code.to_string());
    }
    if let Some(message) = value
        .get("message")
        .and_then(|v| v.as_str())
        .filter(|m| !m.is_empty())
    {
        parts.push(message.to_string());
    }
    if let Some(request_id) = value.get("request_id").and_then(|v| v.as_str()) {
        parts.push(format!("request {request_id}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(": {}", parts.join("; "))
    }
}

// ---------------------------------------------------------------------------
// pending device authorizations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDeviceAuthorization {
    pub origin: String,
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_at_ms: i64,
    pub interval: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingFile {
    version: u8,
    #[serde(default)]
    authorizations: std::collections::BTreeMap<String, PendingDeviceAuthorization>,
}

/// The half-finished login. `oa auth login --headless` writes one; `--resume`
/// reads it. It carries no token, only the code the person is approving.
pub struct PendingStore {
    path: PathBuf,
}

impl PendingStore {
    pub fn new() -> Self {
        Self {
            path: device_authorizations_path(),
        }
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    fn load(&self) -> Result<PendingFile, AuthError> {
        if !self.path.exists() {
            return Ok(PendingFile {
                version: 1,
                authorizations: Default::default(),
            });
        }
        let text = fs::read_to_string(&self.path).map_err(|error| {
            AuthError::new(format!("could not read {}: {error}", self.path.display()))
        })?;
        if text.len() > 65_536 {
            return Err(AuthError::new(format!(
                "{} is larger than a pending authorization file can be",
                self.path.display()
            )));
        }
        serde_json::from_str(&text).map_err(|error| {
            AuthError::new(format!("could not decode {}: {error}", self.path.display()))
        })
    }

    fn save(&self, file: &PendingFile) -> Result<(), AuthError> {
        if file.authorizations.is_empty() {
            if self.path.exists() {
                fs::remove_file(&self.path).map_err(|error| {
                    AuthError::new(format!("could not remove {}: {error}", self.path.display()))
                })?;
            }
            return Ok(());
        }
        let encoded = serde_json::to_string(file).map_err(|error| {
            AuthError::new(format!("could not encode pending authorizations: {error}"))
        })?;
        write_private_file(&self.path, &encoded)
    }

    pub fn get(&self, origin: &str) -> Result<Option<PendingDeviceAuthorization>, AuthError> {
        Ok(self.load()?.authorizations.get(origin).cloned())
    }

    pub fn set(&self, authorization: &PendingDeviceAuthorization) -> Result<(), AuthError> {
        let mut file = self.load()?;
        file.authorizations
            .insert(authorization.origin.clone(), authorization.clone());
        self.save(&file)
    }

    pub fn remove(&self, origin: &str) -> Result<(), AuthError> {
        let mut file = self.load()?;
        if file.authorizations.remove(origin).is_some() {
            self.save(&file)?;
        }
        Ok(())
    }
}

impl Default for PendingStore {
    fn default() -> Self {
        Self::new()
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// browser
// ---------------------------------------------------------------------------

/// Try to open the approval page. The URL is printed either way, so a machine
/// with no browser is not a machine that cannot sign in.
pub fn open_browser(url: &str) -> bool {
    let launcher: Option<(&str, Vec<&str>)> = if cfg!(target_os = "macos") {
        Some(("open", vec![url]))
    } else if cfg!(target_os = "linux") {
        Some(("xdg-open", vec![url]))
    } else if cfg!(target_os = "windows") {
        Some(("cmd", vec!["/C", "start", "", url]))
    } else {
        None
    };
    match launcher {
        Some((program, args)) => Command::new(program)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_never_renders_its_value() {
        let secret = Secret::new("oa_pat_supersecretvalue");
        let rendered = format!("{secret:?}");
        // Asserting on the redaction marker alone would be satisfied by a
        // prefix swap that leaves the value in the tail, so assert absence.
        assert!(!rendered.contains("supersecretvalue"), "{rendered}");
        assert!(
            !format!(
                "{:?}",
                StoredToken {
                    token: secret,
                    source: TokenSource::File
                }
            )
            .contains("supersecretvalue")
        );
    }

    #[test]
    fn zeroize_now_empties_the_buffer() {
        let mut secret = Secret::new("oa_pat_supersecretvalue");
        secret.zeroize_now();
        assert!(secret.is_empty());
        assert_eq!(secret.expose(), "");
    }

    #[test]
    fn origins_must_be_bare_and_https() {
        assert_eq!(
            normalize_api_origin("https://openagents.com/").unwrap(),
            "https://openagents.com"
        );
        assert_eq!(
            normalize_api_origin("http://localhost:4000").unwrap(),
            "http://localhost:4000"
        );
        assert!(normalize_api_origin("http://example.com").is_err());
        assert!(normalize_api_origin("https://user:pw@openagents.com").is_err());
        assert!(normalize_api_origin("https://openagents.com/api/v1").is_err());
        assert!(normalize_api_origin("").is_err());
    }

    #[test]
    fn a_device_authorization_names_the_computer_without_widening_its_scopes() {
        let scopes = vec!["chat:account".to_string(), "forge:write".to_string()];
        let body =
            device_authorization_body(&scopes, Some(format!("  MacBook\nPro {}", "x".repeat(100))));

        assert_eq!(body["scope"], "chat:account forge:write");
        let name = body["device_name"].as_str().expect("a computer name");
        assert!(name.starts_with("MacBook Pro "), "{name}");
        assert!(!name.contains('\n'), "{name}");
        assert_eq!(name.chars().count(), 80);
    }

    #[test]
    fn an_empty_computer_name_is_not_sent() {
        let body = device_authorization_body(&[], Some(" \n ".to_string()));
        assert_eq!(body, serde_json::json!({}));
    }

    #[test]
    fn credentials_have_one_cross_platform_location() {
        let home = Path::new("/home/reader");
        assert_eq!(
            credentials_path_for_home(home),
            home.join(".openagents").join("credentials.json")
        );
    }

    #[test]
    fn the_previous_file_store_moves_to_the_cross_platform_location() {
        let directory = tempfile::tempdir().unwrap();
        let previous = directory.path().join("old").join("cli-credentials.json");
        let current = directory.path().join("new").join("credentials.json");
        std::fs::create_dir_all(previous.parent().unwrap()).unwrap();
        std::fs::write(
            &previous,
            r#"{"version":1,"tokens":{"https://openagents.com":"oa_pat_migrated"}}"#,
        )
        .unwrap();

        migrate_credential_file(&previous, &current).unwrap();

        assert!(!previous.exists());
        assert_eq!(
            std::fs::read_to_string(&current).unwrap(),
            r#"{"version":1,"tokens":{"https://openagents.com":"oa_pat_migrated"}}"#
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&current).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
