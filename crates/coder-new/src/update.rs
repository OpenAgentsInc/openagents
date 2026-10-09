//! Automatic updates for an installed Coder (#11128).
//!
//! The design is `docs/release/terminal-auto-update.md`. Coder reads the
//! release contract `scripts/release/coder.sh` publishes (`coder.<channel>`,
//! `SHA256SUMS-coder-<v>`, `coder-<v>-<platform>.tar.gz|.zip`), at most once
//! a day from the TUI, never blocking startup. On a standalone install it
//! downloads and verifies the newer archive in the background and installs
//! every bundled command in one step when Coder quits, keeping the previous
//! set for `coder update --rollback`. Other installs get a one-line notice.

mod archive;
#[cfg(test)]
mod tests;

use std::{
    cmp::Ordering,
    fmt, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use archive::unpack;

/// The public release prefix the installers read too.
pub const DEFAULT_BASE_URL: &str =
    "https://storage.googleapis.com/openagentsgemini-cli-releases/coder";
/// At most one automatic check a day.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const POINTER_TIMEOUT: Duration = Duration::from_secs(5);
const SUMS_TIMEOUT: Duration = Duration::from_secs(30);
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(600);
const MAX_ARCHIVE: u64 = 1 << 30;
const LOCK_STALE: Duration = Duration::from_secs(600);
const VERSION_TIMEOUT: Duration = Duration::from_secs(30);
/// The folder in the install directory that holds the replaced commands.
pub const BACKUP_DIR: &str = ".coder-previous";
const LOCK_FILE: &str = ".coder-update.lock";

/// `X.Y.Z` or `X.Y.Z-rc.N`, the forms the release script publishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub rc: Option<u64>,
}

impl Version {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (release, rc) = match text.split_once('-') {
            Some((release, suffix)) => {
                let number = suffix.strip_prefix("rc.")?;
                if number.is_empty()
                    || !number.bytes().all(|b| b.is_ascii_digit())
                    || (number.len() > 1 && number.starts_with('0'))
                {
                    return None;
                }
                (release, Some(number.parse().ok()?))
            }
            None => (text, None),
        };
        let mut parts = release.split('.');
        let mut part = || -> Option<u64> {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            part.parse().ok()
        };
        let version = Self {
            major: part()?,
            minor: part()?,
            patch: part()?,
            rc,
        };
        parts.next().is_none().then_some(version)
    }

    /// The version this binary was built as.
    #[must_use]
    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Self {
            major: 0,
            minor: 0,
            patch: 0,
            rc: Some(0),
        })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.rc, other.rc) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(&b),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(rc) = self.rc {
            write!(f, "-rc.{rc}")?;
        }
        Ok(())
    }
}

/// A channel pointer's body: one version, surrounding whitespace allowed.
#[must_use]
pub fn parse_pointer(body: &str) -> Option<Version> {
    let body = body.trim();
    (body.len() <= 64).then(|| Version::parse(body)).flatten()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Rc,
}

impl Channel {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "stable" => Some(Self::Stable),
            "rc" => Some(Self::Rc),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Rc => "rc",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Download, verify, and install on its own (the default).
    Auto,
    /// Show the line; installing is `coder update`.
    Notify,
    /// Never contact the release bucket.
    Off,
}

impl Mode {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "auto" => Some(Self::Auto),
            "notify" => Some(Self::Notify),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Notify => "notify",
            Self::Off => "off",
        }
    }
}

/// `update-settings.json`, written by `coder update --mode/--channel`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<Channel>,
}

/// The settings in force: environment over the settings file over defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub mode: Mode,
    pub channel: Channel,
    pub base_url: String,
    /// `CODER_BASE_URL` names a channel other than the public one.
    pub base_url_overridden: bool,
}

impl Config {
    #[must_use]
    pub fn resolve(
        settings: &Settings,
        env: &dyn Fn(&str) -> Option<String>,
        current: Version,
    ) -> Self {
        let mode = env("CODER_UPDATE")
            .and_then(|value| Mode::parse(&value))
            .or(settings.mode)
            .unwrap_or(Mode::Auto);
        let channel = env("CODER_CHANNEL")
            .and_then(|value| Channel::parse(&value))
            .or(settings.channel)
            .unwrap_or(if current.rc.is_some() {
                Channel::Rc
            } else {
                Channel::Stable
            });
        let overridden = env("CODER_BASE_URL").filter(|url| !url.trim().is_empty());
        Self {
            mode,
            channel,
            base_url: overridden
                .clone()
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned())
                .trim()
                .trim_end_matches('/')
                .to_owned(),
            base_url_overridden: overridden.is_some(),
        }
    }

    /// Whether the TUI may check on its own: not `off`, not under CI, and
    /// not a debug build unless a test channel is named.
    #[must_use]
    pub fn automatic(&self, env: &dyn Fn(&str) -> Option<String>, debug_build: bool) -> bool {
        let ci = env("CI").is_some_and(|value| {
            let value = value.trim().to_ascii_lowercase();
            !value.is_empty() && value != "0" && value != "false"
        });
        self.mode != Mode::Off && !ci && (!debug_build || self.base_url_overridden)
    }
}

/// What `update.json` remembers between runs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Unix seconds of the last check that reached the channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<u64>,
    /// The newest version the channel named, if newer than the binary then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    /// A verified download waiting to be installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staged: Option<Staged>,
    /// A version `coder update --rollback` left; automatic updates skip it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Staged {
    pub version: String,
    pub archive: PathBuf,
    pub sha256: String,
}

impl State {
    #[must_use]
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_atomic(
            path,
            &serde_json::to_vec_pretty(self).map_err(io::Error::other)?,
        )
    }

    /// Whether a check is due at `now`: never checked, a day passed, or the
    /// recorded time is in the future (a clock that moved back).
    #[must_use]
    pub fn due(&self, now: u64) -> bool {
        match self.checked_at {
            None => true,
            Some(at) => at > now || now - at >= CHECK_INTERVAL.as_secs(),
        }
    }

    fn latest_version(&self) -> Option<Version> {
        self.latest.as_deref().and_then(Version::parse)
    }

    fn held_version(&self) -> Option<Version> {
        self.held.as_deref().and_then(Version::parse)
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&temp, bytes)?;
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

#[must_use]
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// How this copy of Coder was installed, from its resolved path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// The installers' layout in a writable directory: Coder replaces itself.
    Standalone(PathBuf),
    /// Inside an app bundle; the OpenAgents app's own update carries it.
    Desktop,
    /// Homebrew or npm.
    PackageManager,
    /// A Cargo build or `scripts/install-coder.sh`; never checked.
    Source,
    /// The installers' layout in a directory Coder cannot write.
    Unwritable(PathBuf),
}

impl InstallKind {
    #[must_use]
    pub fn detect(exe: &Path, openagents_home: Option<&Path>) -> Self {
        let parts: Vec<String> = exe
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        let cargo_build = parts
            .windows(2)
            .any(|pair| pair[0] == "target" && matches!(pair[1].as_str(), "debug" | "release"))
            || parts.windows(3).any(|triple| {
                triple[0] == "target" && matches!(triple[2].as_str(), "debug" | "release")
            });
        let source_install =
            openagents_home.is_some_and(|home| exe.starts_with(home.join("versions")));
        if cargo_build || source_install {
            return Self::Source;
        }
        if parts
            .windows(2)
            .any(|pair| pair[0].ends_with(".app") && pair[1] == "Contents")
        {
            return Self::Desktop;
        }
        if parts.iter().any(|part| {
            matches!(
                part.as_str(),
                "Cellar" | "homebrew" | "linuxbrew" | "node_modules"
            )
        }) {
            return Self::PackageManager;
        }
        let Some(dir) = exe.parent() else {
            return Self::Source;
        };
        if writable(dir) {
            Self::Standalone(dir.to_owned())
        } else {
            Self::Unwritable(dir.to_owned())
        }
    }
}

fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".coder-write-test.{}", std::process::id()));
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// The line the TUI shows for a newer version, if any.
#[must_use]
pub fn notice(kind: &InstallKind, mode: Mode, latest: Version, staged: bool) -> Option<String> {
    Some(match kind {
        InstallKind::Source => return None,
        InstallKind::Standalone(_) if staged && mode == Mode::Auto => {
            format!("Coder {latest} is ready. Restart Coder to use it.")
        }
        InstallKind::Standalone(_) => {
            format!("Coder {latest} is available. Run coder update.")
        }
        InstallKind::Desktop => {
            format!("Coder {latest} is available. Update OpenAgents Desktop to get it.")
        }
        InstallKind::PackageManager => {
            format!("Coder {latest} is available. Update it with your package manager.")
        }
        InstallKind::Unwritable(_) => {
            format!("Coder {latest} is available. Run: {}", installer_command())
        }
    })
}

fn installer_command() -> &'static str {
    if cfg!(windows) {
        "irm https://openagents.com/cli/install.ps1 | iex"
    } else {
        "curl -fsSL https://openagents.com/cli/install.sh | bash"
    }
}

/// The release platform this binary was built for.
#[must_use]
pub fn platform() -> &'static str {
    if cfg!(windows) {
        "windows-x86_64"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos-aarch64"
    } else if cfg!(target_os = "macos") {
        "macos-x86_64"
    } else if cfg!(all(target_arch = "aarch64", target_env = "musl")) {
        "linux-aarch64-musl"
    } else if cfg!(target_arch = "aarch64") {
        "linux-aarch64"
    } else if cfg!(target_env = "musl") {
        "linux-x86_64-musl"
    } else {
        "linux-x86_64"
    }
}

#[must_use]
pub fn archive_name(version: Version, platform: &str) -> String {
    let extension = if platform.starts_with("windows-") {
        "zip"
    } else {
        "tar.gz"
    };
    format!("coder-{version}-{platform}.{extension}")
}

#[must_use]
pub fn sums_name(version: Version) -> String {
    format!("SHA256SUMS-coder-{version}")
}

/// The commands one archive installs, by installed file name.
#[must_use]
pub fn commands(platform: &str) -> Vec<String> {
    if platform.starts_with("windows-") {
        ["coder", "openagents", "microcoder", "coder-boundary"]
            .iter()
            .map(|name| format!("{name}.exe"))
            .collect()
    } else {
        ["coder", "openagents", "microcoder"]
            .iter()
            .map(|name| (*name).to_owned())
            .collect()
    }
}

/// The digest the sums file names for `name`, exactly once.
pub fn sums_entry(sums: &str, name: &str) -> Result<String, String> {
    let mut found = Vec::new();
    for line in sums.lines() {
        let line = line.trim_end_matches('\r');
        let Some((hash, rest)) = line.split_once([' ', '\t']) else {
            continue;
        };
        let file = rest.trim_start_matches([' ', '\t']);
        let file = file.strip_prefix('*').unwrap_or(file);
        if file == name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            found.push(hash.to_ascii_lowercase());
        }
    }
    match found.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(format!("The release has no verified {name}.")),
        _ => Err(format!("The release names {name} more than once.")),
    }
}

pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Everything one run of the updater works with.
#[derive(Clone, Debug)]
pub struct Context {
    /// `~/.openagents/coder-new`: the cache, settings, and downloads.
    pub dir: PathBuf,
    pub kind: InstallKind,
    pub config: Config,
    pub current: Version,
    pub platform: String,
    /// Run each new command's `--version` before installing it.
    pub run_version_checks: bool,
    /// On macOS, the Developer ID team new binaries must be signed by.
    pub signing_team: Option<String>,
}

impl Context {
    /// The running binary's context, reading the environment.
    pub fn from_env(dir: &Path) -> Result<Self, String> {
        let exe = std::env::current_exe()
            .and_then(fs::canonicalize)
            .map_err(|error| format!("Cannot find the running Coder: {error}."))?;
        let home = model_access::store::openagents_dir()
            .map(|home| fs::canonicalize(&home).unwrap_or(home));
        let current = Version::current();
        let settings = load_settings(dir);
        let env = |name: &str| std::env::var(name).ok();
        Ok(Self {
            dir: dir.to_owned(),
            kind: InstallKind::detect(&exe, home.as_deref()),
            config: Config::resolve(&settings, &env, current),
            current,
            platform: platform().to_owned(),
            run_version_checks: true,
            signing_team: signing_team(&exe),
        })
    }

    #[must_use]
    pub fn state_path(&self) -> PathBuf {
        self.dir.join("update.json")
    }

    #[must_use]
    pub fn settings_path(&self) -> PathBuf {
        self.dir.join("update-settings.json")
    }

    fn downloads(&self) -> PathBuf {
        self.dir.join("updates")
    }

    /// The line to show from what the cache already knows, offline too.
    #[must_use]
    pub fn cached_notice(&self) -> Option<String> {
        let state = State::load(&self.state_path());
        let latest = state
            .latest_version()
            .filter(|latest| *latest > self.current)?;
        let staged = state
            .staged
            .as_ref()
            .is_some_and(|staged| Version::parse(&staged.version) == Some(latest));
        notice(&self.kind, self.config.mode, latest, staged)
    }
}

#[must_use]
pub fn load_settings(dir: &Path) -> Settings {
    fs::read(dir.join("update-settings.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// What a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The channel's version when it is newer than this binary.
    pub newer: Option<Version>,
    /// That version is downloaded, verified, and waiting to install.
    pub staged: bool,
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .user_agent(concat!("coder/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("Cannot start the update download: {error}."))
}

async fn get_text(
    client: &reqwest::Client,
    url: &str,
    timeout: Duration,
) -> Result<String, String> {
    let response = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| format!("Cannot read {url}."))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|_| format!("Cannot read {url}."))?;
    if bytes.len() > 1 << 20 {
        return Err(format!("{url} is too large."));
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| format!("{url} is not text."))
}

async fn download(client: &reqwest::Client, url: &str, dest: &Path) -> Result<(), String> {
    let mut response = client
        .get(url)
        .timeout(ARCHIVE_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| format!("Cannot download {url}."))?;
    let temp = dest.with_extension(format!("part.{}", std::process::id()));
    let result = async {
        let mut file =
            fs::File::create(&temp).map_err(|error| format!("Cannot save the update: {error}."))?;
        let mut total = 0u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| format!("Cannot download {url}."))?
        {
            total += chunk.len() as u64;
            if total > MAX_ARCHIVE {
                return Err(format!("{url} is too large."));
            }
            file.write_all(&chunk)
                .map_err(|error| format!("Cannot save the update: {error}."))?;
        }
        file.flush()
            .map_err(|error| format!("Cannot save the update: {error}."))?;
        fs::rename(&temp, dest).map_err(|error| format!("Cannot save the update: {error}."))
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Reads the channel and, when `stage` is set, downloads and verifies the
/// newer archive. Records what it learned in the cache.
pub async fn check(context: &Context, stage: bool, honor_hold: bool) -> Result<Found, String> {
    let client = http()?;
    let state_path = context.state_path();
    let mut state = State::load(&state_path);
    let pointer_url = format!(
        "{}/coder.{}",
        context.config.base_url,
        context.config.channel.as_str()
    );
    let body = get_text(&client, &pointer_url, POINTER_TIMEOUT).await?;
    let remote = parse_pointer(&body).ok_or_else(|| {
        format!(
            "coder.{} does not name a version.",
            context.config.channel.as_str()
        )
    })?;
    state.checked_at = Some(now());
    let held = honor_hold && state.held_version() == Some(remote);
    if remote <= context.current || held {
        state.latest = None;
        if state
            .staged
            .as_ref()
            .and_then(|staged| Version::parse(&staged.version))
            .is_none_or(|staged| staged <= context.current || held)
        {
            discard_staged(&mut state);
        }
        let _ = state.save(&state_path);
        return Ok(Found {
            newer: None,
            staged: false,
        });
    }
    state.latest = Some(remote.to_string());
    let already = state.staged.as_ref().is_some_and(|staged| {
        staged.version == remote.to_string()
            && sha256_file(&staged.archive).is_ok_and(|digest| digest == staged.sha256)
    });
    if !stage || already {
        let _ = state.save(&state_path);
        return Ok(Found {
            newer: Some(remote),
            staged: already,
        });
    }
    discard_staged(&mut state);
    let _ = state.save(&state_path);
    let staged = fetch_verified(context, &client, remote).await?;
    state.staged = Some(staged);
    let _ = state.save(&state_path);
    Ok(Found {
        newer: Some(remote),
        staged: true,
    })
}

fn discard_staged(state: &mut State) {
    if let Some(staged) = state.staged.take() {
        let _ = fs::remove_file(&staged.archive);
    }
}

/// Downloads `version`'s sums and this platform's archive, and keeps the
/// archive only if its digest is the one the sums file names.
async fn fetch_verified(
    context: &Context,
    client: &reqwest::Client,
    version: Version,
) -> Result<Staged, String> {
    let base = &context.config.base_url;
    let sums = get_text(
        client,
        &format!("{base}/{}", sums_name(version)),
        SUMS_TIMEOUT,
    )
    .await?;
    let name = archive_name(version, &context.platform);
    let expected = sums_entry(&sums, &name)?;
    let downloads = context.downloads();
    fs::create_dir_all(&downloads).map_err(|error| format!("Cannot save the update: {error}."))?;
    let dest = downloads.join(&name);
    download(client, &format!("{base}/{name}"), &dest).await?;
    let actual = sha256_file(&dest).map_err(|error| format!("Cannot read the update: {error}."))?;
    if actual != expected {
        let _ = fs::remove_file(&dest);
        return Err(format!("Checksum mismatch for {name}. Coder is unchanged."));
    }
    Ok(Staged {
        version: version.to_string(),
        archive: dest,
        sha256: expected,
    })
}

/// Installs the staged version, if there is one for a standalone install.
/// Returns the installed version.
pub fn install_staged(context: &Context) -> Result<Option<Version>, String> {
    let InstallKind::Standalone(bin) = &context.kind else {
        return Ok(None);
    };
    let state_path = context.state_path();
    let mut state = State::load(&state_path);
    let Some(staged) = state.staged.clone() else {
        return Ok(None);
    };
    let Some(version) = Version::parse(&staged.version).filter(|v| *v > context.current) else {
        discard_staged(&mut state);
        let _ = state.save(&state_path);
        return Ok(None);
    };
    let result = install_archive(
        context,
        bin,
        &staged.archive,
        &staged.sha256,
        version,
        &|_| Ok(()),
    );
    match &result {
        Err(Refusal::Busy) => return Ok(None),
        _ => {
            discard_staged(&mut state);
            if result.is_ok() {
                state.latest = None;
            }
            let _ = state.save(&state_path);
        }
    }
    result
        .map(|()| Some(version))
        .map_err(|refusal| refusal.to_string())
}

/// Why an install did not happen.
#[derive(Debug)]
pub enum Refusal {
    /// Another Coder is installing.
    Busy,
    Failed(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => {
                f.write_str("Another Coder is installing an update. Try again in a minute.")
            }
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl From<String> for Refusal {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

struct Lock(PathBuf);

impl Lock {
    fn take(bin: &Path) -> Result<Self, Refusal> {
        let path = bin.join(LOCK_FILE);
        for _ in 0..2 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let _ = write!(file, "{}", std::process::id());
                    return Ok(Self(path));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .and_then(|at| at.elapsed().ok())
                        .is_some_and(|age| age > LOCK_STALE);
                    if !stale {
                        return Err(Refusal::Busy);
                    }
                    let _ = fs::remove_file(&path);
                }
                Err(error) => {
                    return Err(Refusal::Failed(format!(
                        "Cannot write to {}: {error}.",
                        bin.display()
                    )));
                }
            }
        }
        Err(Refusal::Busy)
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Verifies `archive` against `sha256`, unpacks it next to the installed
/// commands, checks every new command, and swaps them all in. `fault` runs
/// before each command is moved into place; tests use it to fail midway.
pub fn install_archive(
    context: &Context,
    bin: &Path,
    archive: &Path,
    sha256: &str,
    version: Version,
    fault: &dyn Fn(usize) -> io::Result<()>,
) -> Result<(), Refusal> {
    let _lock = Lock::take(bin)?;
    let name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let actual =
        sha256_file(archive).map_err(|error| format!("Cannot read the update: {error}."))?;
    if actual != sha256 {
        return Err(format!("Checksum mismatch for {name}. Coder is unchanged.").into());
    }
    let stage = bin.join(format!(".coder-update.{}", std::process::id()));
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir(&stage).map_err(|error| format!("Cannot unpack the update: {error}."))?;
    let result = (|| {
        let names = commands(&context.platform);
        unpack(archive, &names, &stage)
            .map_err(|message| format!("{message} Coder is unchanged."))?;
        for command in &names {
            make_executable(&stage.join(command))?;
        }
        if context.run_version_checks {
            for command in names
                .iter()
                .filter(|name| !name.starts_with("coder-boundary"))
            {
                check_version(&stage.join(command), command, version)?;
            }
        }
        if let Some(team) = &context.signing_team {
            for command in &names {
                check_signature(&stage.join(command), team)?;
            }
        }
        swap(bin, &stage, &names, fault)
    })();
    let _ = fs::remove_dir_all(&stage);
    result.map_err(Refusal::from)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("Cannot prepare the update: {error}."))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// `<command> --version` must print `<name> <version>` on its first line.
fn check_version(path: &Path, command: &str, version: Version) -> Result<(), String> {
    let name = command.strip_suffix(".exe").unwrap_or(command);
    let refused = || format!("The new {name} does not run on this computer. Coder is unchanged.");
    let mut child = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| refused())?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < VERSION_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(refused());
            }
        }
    };
    let mut output = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_string(&mut output);
    }
    let mut words = output.lines().next().unwrap_or_default().split_whitespace();
    if !status.success()
        || words.next() != Some(name)
        || words.next() != Some(version.to_string().as_str())
    {
        return Err(format!(
            "The new {name} does not report version {version}. Coder is unchanged."
        ));
    }
    Ok(())
}

/// The Developer ID team that signed `path`, on macOS.
#[must_use]
pub fn signing_team(path: &Path) -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let output = Command::new("/usr/bin/codesign")
        .args(["-dv", "--verbose=2"])
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stderr);
    text.lines()
        .find_map(|line| line.strip_prefix("TeamIdentifier="))
        .map(str::trim)
        .filter(|team| !team.is_empty() && *team != "not set")
        .map(str::to_owned)
}

fn check_signature(path: &Path, team: &str) -> Result<(), String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let verified = Command::new("/usr/bin/codesign")
        .args(["--verify", "--strict"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !verified || signing_team(path).as_deref() != Some(team) {
        return Err(format!(
            "The new {name} is not signed by OpenAgents. Coder is unchanged."
        ));
    }
    Ok(())
}

/// Moves each installed command in `names` into a new backup set, then each
/// staged one into place. Any failure puts every moved command back. On
/// success the new backup set replaces `.coder-previous`.
pub fn swap(
    bin: &Path,
    stage: &Path,
    names: &[String],
    fault: &dyn Fn(usize) -> io::Result<()>,
) -> Result<(), String> {
    let pid = std::process::id();
    let backup = bin.join(format!("{BACKUP_DIR}.{pid}"));
    let _ = fs::remove_dir_all(&backup);
    fs::create_dir(&backup).map_err(|error| format!("Cannot back up Coder: {error}."))?;
    let mut moved_out = Vec::new();
    let mut moved_in = Vec::new();
    let undo = |moved_out: &[&String], moved_in: &[&String]| {
        for name in moved_in {
            let _ = fs::rename(bin.join(name), stage.join(name));
        }
        for name in moved_out {
            let _ = fs::rename(backup.join(name), bin.join(name));
        }
        let _ = fs::remove_dir_all(&backup);
    };
    for name in names {
        let installed = bin.join(name);
        match fs::symlink_metadata(&installed) {
            Ok(meta) if meta.is_dir() => {
                undo(&moved_out, &moved_in);
                return Err(format!(
                    "{} is a folder. Coder is unchanged.",
                    installed.display()
                ));
            }
            Ok(_) => {
                if let Err(error) = fs::rename(&installed, backup.join(name)) {
                    undo(&moved_out, &moved_in);
                    return Err(format!(
                        "Cannot replace {name}: {error}. Coder is unchanged."
                    ));
                }
                moved_out.push(name);
            }
            Err(_) => {}
        }
    }
    for (at, name) in names.iter().enumerate() {
        if let Err(error) = fault(at).and_then(|()| fs::rename(stage.join(name), bin.join(name))) {
            undo(&moved_out, &moved_in);
            return Err(format!(
                "Cannot install {name}: {error}. Coder is unchanged."
            ));
        }
        moved_in.push(name);
    }
    let previous = bin.join(BACKUP_DIR);
    if fs::remove_dir_all(&previous).is_err() && previous.exists() {
        // Windows keeps a running image's file; move the old set aside.
        let _ = fs::rename(&previous, bin.join(format!("{BACKUP_DIR}.stale.{pid}")));
    }
    let _ = fs::rename(&backup, &previous);
    Ok(())
}

/// Puts the commands the last update replaced back, keeping the ones it
/// removes as the new backup, so a second rollback undoes the first.
pub fn rollback(context: &Context) -> Result<Version, String> {
    let InstallKind::Standalone(bin) = &context.kind else {
        return Err(not_standalone(&context.kind));
    };
    let lock = Lock::take(bin).map_err(|refusal| refusal.to_string())?;
    let previous = bin.join(BACKUP_DIR);
    let coder = commands(&context.platform)
        .into_iter()
        .next()
        .unwrap_or_else(|| "coder".into());
    let Some(version) = version_of(&previous.join(&coder)) else {
        return Err("There is no earlier Coder to go back to.".into());
    };
    let stage = bin.join(format!(".coder-update.{}", std::process::id()));
    let _ = fs::remove_dir_all(&stage);
    fs::rename(&previous, &stage).map_err(|error| format!("Cannot go back: {error}."))?;
    let names: Vec<String> = commands(&context.platform)
        .into_iter()
        .filter(|name| stage.join(name).is_file())
        .collect();
    let result = swap(bin, &stage, &names, &|_| Ok(()));
    if result.is_err() {
        let _ = fs::rename(&stage, &previous);
    } else {
        let _ = fs::remove_dir_all(&stage);
    }
    drop(lock);
    result?;
    let state_path = context.state_path();
    let mut state = State::load(&state_path);
    state.held = Some(context.current.to_string());
    state.latest = None;
    discard_staged(&mut state);
    let _ = state.save(&state_path);
    Ok(version)
}

fn version_of(path: &Path) -> Option<Version> {
    let output = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    Version::parse(text.split_whitespace().nth(1)?)
}

fn not_standalone(kind: &InstallKind) -> String {
    match kind {
        InstallKind::Desktop => {
            "This Coder comes with OpenAgents Desktop. Update the app to update Coder.".into()
        }
        InstallKind::PackageManager => {
            "This Coder was installed by a package manager. Update it there.".into()
        }
        InstallKind::Source => {
            "This Coder is a source build. Rebuild it, or install a release: ".to_owned()
                + installer_command()
        }
        InstallKind::Unwritable(dir) => format!(
            "Coder cannot write to {}. Run: {}",
            dir.display(),
            installer_command()
        ),
        InstallKind::Standalone(_) => String::new(),
    }
}

/// A background check's result for the TUI.
pub enum Event {
    Notice(String),
}

/// Starts the TUI's daily check on a background thread when it is due.
/// Returns `None` when no check runs.
#[must_use]
pub fn spawn_check(context: Context) -> Option<std::sync::mpsc::Receiver<Event>> {
    if context.kind == InstallKind::Source || !State::load(&context.state_path()).due(now()) {
        return None;
    }
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        let stage =
            context.config.mode == Mode::Auto && matches!(context.kind, InstallKind::Standalone(_));
        if let Ok(Found {
            newer: Some(version),
            staged,
        }) = runtime.block_on(check(&context, stage, true))
            && let Some(line) = notice(&context.kind, context.config.mode, version, staged)
        {
            let _ = send.send(Event::Notice(line));
        }
    });
    Some(receive)
}

pub const USAGE: &str = "Usage:
  coder update                 Install the newest Coder on your channel now.
  coder update --check         Say whether a newer Coder is published.
  coder update --rollback      Go back to the Coder the last update replaced.
  coder update --mode MODE     auto: download and install updates on their own (default).
                               notify: only say when one is ready. off: never check.
  coder update --channel NAME  stable (default) or rc.

Coder checks once a day while it is open. CODER_UPDATE=auto|notify|off and
CODER_CHANNEL=stable|rc override the saved choice; checks never run under CI.";

/// `coder update …`. Writes what happened to `out`.
pub fn command(args: &[String], dir: &Path, out: &mut dyn Write) -> Result<(), String> {
    let write = |out: &mut dyn Write, line: String| {
        let _ = writeln!(out, "{line}");
    };
    match args {
        [flag] if flag == "--help" || flag == "-h" => {
            write(out, USAGE.into());
            return Ok(());
        }
        [flag, value] if flag == "--mode" || flag == "--channel" => {
            let mut settings = load_settings(dir);
            if flag == "--mode" {
                settings.mode =
                    Some(Mode::parse(value).ok_or("Choose --mode auto, notify, or off.")?);
            } else {
                settings.channel =
                    Some(Channel::parse(value).ok_or("Choose --channel stable or rc.")?);
            }
            write_atomic(
                &dir.join("update-settings.json"),
                &serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?,
            )
            .map_err(|error| format!("Cannot save the setting: {error}."))?;
            let line = if flag == "--mode" {
                match settings.mode {
                    Some(Mode::Auto) => "Coder will download and install updates on its own.",
                    Some(Mode::Notify) => {
                        "Coder will say when an update is ready; run coder update to install it."
                    }
                    _ => "Coder will not check for updates. Run coder update to update.",
                }
                .to_owned()
            } else {
                format!("Coder follows the {value} channel.")
            };
            write(out, line);
            return Ok(());
        }
        [] => {}
        [flag] if flag == "--check" || flag == "--rollback" => {}
        _ => return Err(format!("Unknown option. {USAGE}")),
    }
    let context = Context::from_env(dir)?;
    run(&context, args.first().map(String::as_str), out)
}

/// The network part of `coder update`, split out so tests can name the
/// context.
pub fn run(context: &Context, flag: Option<&str>, out: &mut dyn Write) -> Result<(), String> {
    let mut write = |line: String| {
        let _ = writeln!(out, "{line}");
    };
    if flag == Some("--rollback") {
        let version = rollback(context)?;
        write(format!(
            "Went back to Coder {version}. Automatic updates skip {} until you run coder update.",
            context.current
        ));
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Cannot start the update.".to_owned())?;
    let channel = context.config.channel.as_str();
    if flag == Some("--check") {
        let found = runtime.block_on(check(context, false, false))?;
        match found.newer {
            Some(version) => write(
                notice(&context.kind, Mode::Notify, version, false)
                    .unwrap_or_else(|| format!("Coder {version} is available.")),
            ),
            None => write(format!(
                "Coder {} is the newest on the {channel} channel.",
                context.current
            )),
        }
        return Ok(());
    }
    if !matches!(context.kind, InstallKind::Standalone(_)) {
        return Err(not_standalone(&context.kind));
    }
    write(format!("Checking the {channel} channel..."));
    let found = runtime.block_on(check(context, true, false))?;
    let Some(version) = found.newer else {
        write(format!(
            "Coder {} is the newest on the {channel} channel.",
            context.current
        ));
        let mut state = State::load(&context.state_path());
        state.held = None;
        let _ = state.save(&context.state_path());
        return Ok(());
    };
    write(format!("Downloaded and verified Coder {version}."));
    match install_staged(context)? {
        Some(installed) => {
            let mut state = State::load(&context.state_path());
            state.held = None;
            let _ = state.save(&context.state_path());
            write(format!("Updated Coder to {installed}."));
            Ok(())
        }
        None => Err(Refusal::Busy.to_string()),
    }
}
