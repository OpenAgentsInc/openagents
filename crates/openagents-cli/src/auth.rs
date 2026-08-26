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
    /// `oa auth logout` calls this before it asks the OS store to delete the
    /// record, so the plaintext is gone from this process even if the deletion
    /// itself fails and the command exits by the error path.
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
            ))
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

/// The file adapter's path.
///
/// Deliberately *not* `credentials.json` in that directory: that name is
/// already taken by the agent-key store (`{"agents": …, "default": …}`), and
/// writing this store's shape over it would destroy an unrelated set of keys.
/// The OS keychain remains the primary store; this file is what a machine
/// without one falls back to.
pub fn credentials_path() -> PathBuf {
    config_directory().join("cli-credentials.json")
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

/// Write a file 0600 through a temporary file in the same directory.
fn write_private_file(path: &Path, contents: &str) -> Result<(), AuthError> {
    let parent = path
        .parent()
        .ok_or_else(|| AuthError::new(format!("{} has no parent directory", path.display())))?;
    ensure_private_directory(parent)?;
    let temporary = path.with_extension("tmp");
    {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|error| {
            AuthError::new(format!("could not write {}: {error}", temporary.display()))
        })?;
        file.write_all(contents.as_bytes()).map_err(|error| {
            AuthError::new(format!("could not write {}: {error}", temporary.display()))
        })?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|error| {
            AuthError::new(format!(
                "could not restrict {} to 0600: {error}",
                temporary.display()
            ))
        })?;
    }
    fs::rename(&temporary, path)
        .map_err(|error| AuthError::new(format!("could not write {}: {error}", path.display())))
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
    Store,
    File,
    LegacyConfig,
}

impl TokenSource {
    pub fn label(self) -> &'static str {
        match self {
            TokenSource::Environment => "environment",
            TokenSource::Store => "store",
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

/// The service name the OS credential store files the record under. The
/// TypeScript CLI uses the same one.
const KEYCHAIN_SERVICE: &str = "openagents-cli";

/// Longest token the store will hand back. A record longer than this is not a
/// token the API issues, so it is a corrupt or hostile record, not a credential.
const MAX_TOKEN_LENGTH: usize = 160;

fn admitted_token(value: &str) -> bool {
    (value.starts_with("oa_pat_") || value.starts_with("smct_")) && value.len() < MAX_TOKEN_LENGTH
}

pub struct CredentialStore {
    origin: String,
    credentials_path: PathBuf,
    legacy_config_path: PathBuf,
    use_os_store: bool,
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
            legacy_config_path: path.unwrap_or_else(Self::default_path),
            use_os_store: true,
        }
    }

    pub fn for_origin(origin: &str) -> Self {
        Self {
            origin: origin.to_string(),
            credentials_path: credentials_path(),
            legacy_config_path: Self::default_path(),
            use_os_store: true,
        }
    }

    /// A store confined to a directory, with the OS keychain switched off.
    /// Tests use it so they exercise the real read, write, and delete paths
    /// without touching the developer's own credentials.
    pub fn isolated(origin: &str, directory: &Path) -> Self {
        Self {
            origin: origin.to_string(),
            credentials_path: directory.join("credentials.json"),
            legacy_config_path: directory.join("config.json"),
            use_os_store: false,
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

    // -- OS credential store -------------------------------------------------

    /// Read the OS store.
    ///
    /// `Ok(None)` means the store answered and holds nothing. An error means
    /// the store could not be consulted, and that is never reported as "not
    /// signed in": the caller would then send an unauthenticated request that
    /// fails somewhere far away from the actual problem.
    fn os_get(&self) -> Result<Option<Secret>, AuthError> {
        if !self.use_os_store {
            return Ok(None);
        }
        let output = match os_store_command_get(&self.origin) {
            Some(mut command) => match command.output() {
                Ok(output) => output,
                // No `security` / `secret-tool` on this machine. That is not a
                // failure to read: this platform simply has no OS store, and
                // the file adapter below is the whole store.
                Err(_) => return Ok(None),
            },
            None => return Ok(None),
        };
        if !output.status.success() {
            return Ok(None);
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if value.is_empty() {
            return Ok(None);
        }
        if !admitted_token(&value) {
            return Err(AuthError::new(format!(
                "the OS credential store holds a record for {} that is not an OpenAgents token. \
                 Run oa auth logout, then oa auth login",
                self.origin
            )));
        }
        Ok(Some(Secret::new(value)))
    }

    fn os_set(&self, token: &Secret) -> Result<bool, AuthError> {
        if !self.use_os_store {
            return Ok(false);
        }
        let Some((mut command, stdin_input)) = os_store_command_set(&self.origin, token) else {
            return Ok(false);
        };
        let output = match stdin_input {
            None => match command.output() {
                Ok(output) => output,
                Err(_) => return Ok(false),
            },
            Some(input) => {
                command.stdin(Stdio::piped()).stdout(Stdio::piped());
                let mut child = match command.spawn() {
                    Ok(child) => child,
                    Err(_) => return Ok(false),
                };
                if let Some(mut pipe) = child.stdin.take() {
                    let _ = pipe.write_all(input.expose().as_bytes());
                    let _ = pipe.write_all(b"\n");
                }
                child.wait_with_output().map_err(|error| {
                    AuthError::new(format!("the OS credential store failed: {error}"))
                })?
            }
        };
        if !output.status.success() {
            return Err(AuthError::new(format!(
                "the OS credential store refused to store a token for {} (exit {})",
                self.origin,
                output.status.code().unwrap_or(-1)
            )));
        }
        // Read back. A store that reported success but did not keep the value
        // would leave the next command sending the previous token.
        match self.os_get()? {
            Some(stored) if stored.expose() == token.expose() => Ok(true),
            _ => Err(AuthError::new(
                "the OS credential store did not return the token that was just written",
            )),
        }
    }

    fn os_remove(&self) {
        if !self.use_os_store {
            return;
        }
        if let Some(mut command) = os_store_command_remove(&self.origin) {
            let _ = command.output();
        }
    }

    // -- public API ----------------------------------------------------------

    /// Find the token for this endpoint, refusing when a store could not be read.
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
        if let Some(token) = self.os_get()? {
            return Ok(Some(StoredToken {
                token,
                source: TokenSource::Store,
            }));
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
    /// [`CredentialStore::find_token`] instead, so a store that cannot be read
    /// is refused rather than reported as "not signed in".
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
        if self.os_set(token)? {
            return Ok(TokenSource::Store);
        }
        let mut file = self.load_credential_file()?;
        file.tokens
            .insert(self.origin.clone(), token.expose().to_string());
        self.save_credential_file(&file)?;
        Ok(TokenSource::File)
    }

    pub fn set_token(&self, token: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.store(&Secret::new(token))?;
        Ok(())
    }

    /// Remove the token for this endpoint.
    ///
    /// The in-memory copy is wiped before any deletion is attempted, so the
    /// plaintext is gone from this process even if a store then refuses to
    /// delete its record and the command exits by the error path.
    pub fn remove(&self) -> Result<bool, AuthError> {
        let held = self.find_token()?;
        let had_token = held.is_some();
        if let Some(mut stored) = held {
            stored.token.zeroize_now();
            debug_assert!(stored.token.is_empty());
            drop(stored);
        }

        self.os_remove();

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

fn os_store_command_get(origin: &str) -> Option<Command> {
    if cfg!(target_os = "macos") {
        let mut command = Command::new("security");
        command.args([
            "find-generic-password",
            "-a",
            origin,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
        ]);
        command.stderr(Stdio::null());
        Some(command)
    } else if cfg!(target_os = "linux") {
        let mut command = Command::new("secret-tool");
        command.args(["lookup", "service", KEYCHAIN_SERVICE, "origin", origin]);
        command.stderr(Stdio::null());
        Some(command)
    } else {
        None
    }
}

fn os_store_command_set(origin: &str, token: &Secret) -> Option<(Command, Option<Secret>)> {
    if cfg!(target_os = "macos") {
        let mut command = Command::new("security");
        command.args([
            "add-generic-password",
            "-U",
            "-a",
            origin,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
            token.expose(),
        ]);
        command.stderr(Stdio::null());
        Some((command, None))
    } else if cfg!(target_os = "linux") {
        let mut command = Command::new("secret-tool");
        command.args([
            "store",
            "--label=OpenAgents CLI",
            "service",
            KEYCHAIN_SERVICE,
            "origin",
            origin,
        ]);
        command.stderr(Stdio::null());
        Some((command, Some(token.clone())))
    } else {
        None
    }
}

fn os_store_command_remove(origin: &str) -> Option<Command> {
    if cfg!(target_os = "macos") {
        let mut command = Command::new("security");
        command.args([
            "delete-generic-password",
            "-a",
            origin,
            "-s",
            KEYCHAIN_SERVICE,
        ]);
        command.stderr(Stdio::null());
        command.stdout(Stdio::null());
        Some(command)
    } else if cfg!(target_os = "linux") {
        let mut command = Command::new("secret-tool");
        command.args(["clear", "service", KEYCHAIN_SERVICE, "origin", origin]);
        command.stderr(Stdio::null());
        Some(command)
    } else {
        None
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
        let body = if scopes.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::json!({ "scope": scopes.join(" ") })
        };
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
                    })
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
        assert!(!format!(
            "{:?}",
            StoredToken {
                token: secret,
                source: TokenSource::Store
            }
        )
        .contains("supersecretvalue"));
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
}
