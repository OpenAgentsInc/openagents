//! `oa update` — replace this binary with the one the channel names.
//!
//! This is the installer's contract read from the other end. The script at
//! <https://openagents.com/install.sh> resolves a channel pointer to a version,
//! downloads `openagents-<version>-<platform>`, fetches
//! `SHA256SUMS-<version>` over a separate request, refuses when the sums file
//! is missing or names no entry for the artifact, refuses when the digest
//! disagrees, and only then makes the bytes executable. Every one of those
//! refusals is repeated here, because an update path that verifies less than
//! the install path would mean the second binary a reader receives is held to
//! a lower standard than the first.
//!
//! Two things this knows that the shell script has to work out at runtime. The
//! platform is decided at compile time: a binary knows its own architecture and
//! its own C library, so there is no libc probe here and no way for one to be
//! wrong. And the path to replace comes from the running process rather than
//! from a convention about where the installer puts things, so an `oa` invoked
//! through the symlinks the installer leaves in `~/.openagents/bin` updates the
//! file those symlinks point at and they keep pointing at it.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Where releases are published. Overridable so the flow can be exercised
/// against a fixture server without pointing a test at the real one.
pub const DEFAULT_BASE_URL: &str = "https://openagents.com/releases";

/// The channel a reader who names none is asking for.
pub const DEFAULT_CHANNEL: &str = "stable";

#[derive(Debug)]
pub enum UpdateError {
    /// This build has no published artifact to update to.
    UnsupportedPlatform { os: String, arch: String },
    /// The channel pointer could not be read.
    ChannelUnreadable { channel: String, detail: String },
    /// The channel resolved to something that is not a version.
    ChannelNotAVersion { channel: String, body: String },
    /// A version was named that the release naming grammar does not admit.
    InvalidVersion(String),
    /// The artifact itself could not be fetched.
    ArtifactUnavailable { name: String, detail: String },
    /// The sums file could not be fetched. Nothing is installed unverified.
    SumsUnavailable { version: String, detail: String },
    /// The sums file exists but names no entry for this artifact.
    SumsMissingEntry { version: String, name: String },
    /// The bytes that arrived are not the bytes the release published.
    DigestMismatch {
        name: String,
        expected: String,
        actual: String,
    },
    /// The binary could not be replaced.
    ReplaceFailed { path: PathBuf, detail: String },
    /// The named version is older than the one already installed.
    WouldDowngrade { current: String, requested: String },
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform { os, arch } => write!(
                formatter,
                "no release is published for {os}/{arch}, so there is nothing to update to"
            ),
            Self::ChannelUnreadable { channel, detail } => write!(
                formatter,
                "could not resolve the '{channel}' channel: {detail}"
            ),
            Self::ChannelNotAVersion { channel, body } => write!(
                formatter,
                "the '{channel}' channel returned something that is not a version: {body}"
            ),
            Self::InvalidVersion(version) => write!(
                formatter,
                "invalid version: {version} (expected X.Y.Z or X.Y.Z-suffix)"
            ),
            Self::ArtifactUnavailable { name, detail } => {
                write!(formatter, "could not download {name}: {detail}")
            }
            Self::SumsUnavailable { version, detail } => write!(
                formatter,
                "could not download SHA256SUMS-{version} ({detail}); \
                 refusing to install unverified bytes"
            ),
            Self::SumsMissingEntry { version, name } => write!(
                formatter,
                "SHA256SUMS-{version} names no entry for {name}; refusing to install"
            ),
            Self::DigestMismatch {
                name,
                expected,
                actual,
            } => write!(
                formatter,
                "checksum mismatch for {name}\n  expected {expected}\n  actual   {actual}"
            ),
            Self::ReplaceFailed { path, detail } => {
                write!(formatter, "could not replace {}: {detail}", path.display())
            }
            Self::WouldDowngrade { current, requested } => write!(
                formatter,
                "{requested} is older than the installed {current}. Pass --force to install it anyway"
            ),
        }
    }
}

impl std::error::Error for UpdateError {}

/// The platform segment of the artifact name for the build this code is
/// compiled into.
///
/// The installer works this out at runtime from `uname` and a search for the
/// glibc loader. A binary does not have to: it was built for exactly one
/// target, and `target_env = "musl"` is settled by the toolchain that produced
/// it. `None` means this build has no published counterpart, which is a
/// clearer thing to say than a request for a URL that will 404.
pub fn platform() -> Option<String> {
    let os = match std::env::consts::OS {
        "macos" => "macos",
        "linux" => "linux",
        "windows" => "windows",
        _ => return None,
    };

    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => return None,
    };

    // Only Linux is published in two libc flavors. The glibc artifact keeps
    // the unsuffixed name it has always had; musl is the one that carries a
    // suffix, exactly as the installer asks for it.
    let libc = if os == "linux" && cfg!(target_env = "musl") {
        "-musl"
    } else {
        ""
    };

    Some(format!("{os}-{arch}{libc}"))
}

/// The grammar `ops/release-cli.sh` and the installer both apply. A version
/// one of them accepts and another rejects is a release nobody can ask for.
/// Compare two versions the release grammar admits.
///
/// Core `X.Y.Z` numbers are compared first. A release without a suffix sorts
/// ahead of the same core with a suffix (`0.2.0` is newer than `0.2.0-rc.13`).
/// Suffix identifiers split on `.`; a numeric identifier compares as a number,
/// otherwise as bytes. `None` means a side is not a version.
pub fn cmp_release_versions(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let left = parse_release_version(left)?;
    let right = parse_release_version(right)?;
    Some(compare_parsed(left, right))
}

fn compare_parsed(
    left: (u64, u64, u64, Option<Vec<PrereleaseIdent>>),
    right: (u64, u64, u64, Option<Vec<PrereleaseIdent>>),
) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (left.0, left.1, left.2).cmp(&(right.0, right.1, right.2)) {
        Ordering::Equal => match (&left.3, &right.3) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left_pre), Some(right_pre)) => left_pre.cmp(right_pre),
        },
        other => other,
    }
}

fn parse_release_version(value: &str) -> Option<(u64, u64, u64, Option<Vec<PrereleaseIdent>>)> {
    if !valid_version(value) {
        return None;
    }

    let (core, suffix) = match value.split_once('-') {
        Some((core, suffix)) => (core, Some(suffix)),
        None => (value, None),
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    let prerelease = suffix.map(|suffix| {
        suffix
            .split('.')
            .map(|part| {
                if !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()) {
                    PrereleaseIdent::Numeric(part.parse().unwrap_or(0))
                } else {
                    PrereleaseIdent::Text(part.to_string())
                }
            })
            .collect()
    });
    Some((major, minor, patch, prerelease))
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PrereleaseIdent {
    Numeric(u64),
    Text(String),
}

pub fn valid_version(value: &str) -> bool {
    let (core, suffix) = match value.split_once('-') {
        Some((core, suffix)) => (core, Some(suffix)),
        None => (value, None),
    };

    let mut parts = core.split('.');
    let numeric = |part: Option<&str>| {
        part.is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    };

    if !numeric(parts.next()) || !numeric(parts.next()) || !numeric(parts.next()) {
        return false;
    }

    if parts.next().is_some() {
        return false;
    }

    match suffix {
        None => true,
        Some(suffix) => {
            !suffix.is_empty()
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_')
        }
    }
}

/// Read one digest out of a `SHA256SUMS` file.
///
/// The installer's lookup is `awk '$2 == name || $2 == "*" name'`, and the
/// leading `*` is the binary-mode marker `sha256sum` writes. This is that
/// lookup, so an entry either tool accepts is an entry both accept.
pub fn digest_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let digest = fields.next()?;
        let entry = fields.next()?;

        if entry == name || entry.strip_prefix('*') == Some(name) {
            Some(digest.to_string())
        } else {
            None
        }
    })
}

/// The published object name for a version and platform.
///
/// The artifact URL never carries a file extension, on any platform. The
/// `SHA256SUMS` entry for Windows *does*, because the installer appends `.exe`
/// to the name it searches for after downloading a URL without one. The two
/// disagree by design and `ops/release-cli.sh` publishes them that way, so
/// both spellings live here rather than being guessed at a call site.
pub fn artifact_name(version: &str, platform: &str) -> String {
    format!("openagents-{version}-{platform}")
}

pub fn sums_entry_name(version: &str, platform: &str) -> String {
    let name = artifact_name(version, platform);

    if platform.starts_with("windows-") {
        format!("{name}.exe")
    } else {
        name
    }
}

pub fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub struct Updater {
    pub base_url: String,
    pub channel: String,
    http: reqwest::Client,
}

/// What an update run decided, so a caller can report it without inferring it
/// from printed text.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    AlreadyCurrent {
        version: String,
    },
    Available {
        version: String,
    },
    Replaced {
        from: String,
        to: String,
        path: PathBuf,
    },
    /// The channel or requested version is older than what is installed.
    Older {
        version: String,
        installed: String,
    },
}

impl Outcome {
    /// The `--json` document. `Outcome` exists so a caller can report the
    /// decision without inferring it from printed text; this is that report.
    pub fn document(&self) -> serde_json::Value {
        match self {
            Self::AlreadyCurrent { version } => serde_json::json!({
                "schema": "openagents.cli_update.v1",
                "outcome": "already_current",
                "version": version,
            }),
            Self::Available { version } => serde_json::json!({
                "schema": "openagents.cli_update.v1",
                "outcome": "available",
                "version": version,
                "installed": crate::VERSION,
            }),
            Self::Replaced { from, to, path } => serde_json::json!({
                "schema": "openagents.cli_update.v1",
                "outcome": "replaced",
                "from": from,
                "to": to,
                "path": path.to_string_lossy(),
            }),
            Self::Older { version, installed } => serde_json::json!({
                "schema": "openagents.cli_update.v1",
                "outcome": "older",
                "version": version,
                "installed": installed,
            }),
        }
    }
}

impl Updater {
    pub fn new(base_url: Option<String>, channel: Option<String>) -> Self {
        let base_url = base_url
            .or_else(|| std::env::var("OPENAGENTS_RELEASES_BASE_URL").ok())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let channel = channel
            .or_else(|| std::env::var("OPENAGENTS_CHANNEL").ok())
            .unwrap_or_else(|| DEFAULT_CHANNEL.to_string());

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            channel,
            http: reqwest::Client::new(),
        }
    }

    /// Resolve the channel pointer to the version it currently names.
    pub async fn resolve_channel(&self) -> Result<String, UpdateError> {
        let url = format!("{}/{}", self.base_url, self.channel);

        let response =
            self.http
                .get(&url)
                .send()
                .await
                .map_err(|error| UpdateError::ChannelUnreadable {
                    channel: self.channel.clone(),
                    detail: error.to_string(),
                })?;

        if !response.status().is_success() {
            return Err(UpdateError::ChannelUnreadable {
                channel: self.channel.clone(),
                detail: format!("{} answered {}", url, response.status()),
            });
        }

        let body = response
            .text()
            .await
            .map_err(|error| UpdateError::ChannelUnreadable {
                channel: self.channel.clone(),
                detail: error.to_string(),
            })?;

        let version = body.trim().to_string();

        if !valid_version(&version) {
            return Err(UpdateError::ChannelNotAVersion {
                channel: self.channel.clone(),
                body: version,
            });
        }

        Ok(version)
    }

    /// Download the artifact and prove it is the one the release published.
    ///
    /// The sums file is fetched over its own request rather than alongside the
    /// artifact. A digest that arrived on the same connection as the bytes it
    /// describes proves only that they travelled together.
    pub async fn fetch_verified(
        &self,
        version: &str,
        platform: &str,
    ) -> Result<Vec<u8>, UpdateError> {
        if !valid_version(version) {
            return Err(UpdateError::InvalidVersion(version.to_string()));
        }

        let name = artifact_name(version, platform);
        let url = format!("{}/{}", self.base_url, name);

        let response =
            self.http
                .get(&url)
                .send()
                .await
                .map_err(|error| UpdateError::ArtifactUnavailable {
                    name: name.clone(),
                    detail: error.to_string(),
                })?;

        if !response.status().is_success() {
            return Err(UpdateError::ArtifactUnavailable {
                name: name.clone(),
                detail: format!("{} answered {}", url, response.status()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|error| UpdateError::ArtifactUnavailable {
                name: name.clone(),
                detail: error.to_string(),
            })?
            .to_vec();

        let sums_url = format!("{}/SHA256SUMS-{}", self.base_url, version);

        let sums_response = self.http.get(&sums_url).send().await.map_err(|error| {
            UpdateError::SumsUnavailable {
                version: version.to_string(),
                detail: error.to_string(),
            }
        })?;

        if !sums_response.status().is_success() {
            return Err(UpdateError::SumsUnavailable {
                version: version.to_string(),
                detail: format!("{} answered {}", sums_url, sums_response.status()),
            });
        }

        let sums = sums_response
            .text()
            .await
            .map_err(|error| UpdateError::SumsUnavailable {
                version: version.to_string(),
                detail: error.to_string(),
            })?;

        let entry = sums_entry_name(version, platform);

        let expected = digest_for(&sums, &entry).ok_or_else(|| UpdateError::SumsMissingEntry {
            version: version.to_string(),
            name: entry.clone(),
        })?;

        let actual = hex_digest(&bytes);

        if !actual.eq_ignore_ascii_case(&expected) {
            return Err(UpdateError::DigestMismatch {
                name: entry,
                expected,
                actual,
            });
        }

        Ok(bytes)
    }
}

/// The file this process is running from, with symlinks resolved.
///
/// The installer links `~/.openagents/bin/oa` and `~/.openagents/bin/openagents`
/// at a single file under `~/.openagents/downloads`. Replacing the link target
/// is what keeps both names working; replacing a link would leave the other
/// name pointing at the old binary.
pub fn running_binary() -> Result<PathBuf, UpdateError> {
    let path = std::env::current_exe().map_err(|error| UpdateError::ReplaceFailed {
        path: PathBuf::from("<unknown>"),
        detail: format!("could not locate the running binary: {error}"),
    })?;

    Ok(path.canonicalize().unwrap_or(path))
}

/// Write `bytes` over `target` without ever leaving a partial binary there.
///
/// The new file is written beside the target so the final step is a rename
/// within one filesystem, which is atomic: a reader who runs `oa` during an
/// update gets the old binary or the new one and never half of either. On Unix
/// the rename also works while the old binary is executing, because the
/// running process holds the inode rather than the name.
pub fn replace_binary(target: &Path, bytes: &[u8]) -> Result<(), UpdateError> {
    let directory = target.parent().ok_or_else(|| UpdateError::ReplaceFailed {
        path: target.to_path_buf(),
        detail: "the running binary has no parent directory".to_string(),
    })?;

    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "oa".to_string());

    let staged = directory.join(format!(".{}.update.{}", file_name, std::process::id()));

    let fail = |detail: String| UpdateError::ReplaceFailed {
        path: target.to_path_buf(),
        detail,
    };

    std::fs::write(&staged, bytes).map_err(|error| fail(error.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| fail(error.to_string()))?;
    }

    // Windows refuses to rename over a running executable, so the old name is
    // moved aside first. The installer does the same thing with the same
    // `.old` suffix, and Windows will delete it on the next boot or the next
    // update, whichever a reader reaches first.
    #[cfg(windows)]
    let displaced = {
        let displaced = directory.join(format!("{file_name}.old"));
        let _ = std::fs::remove_file(&displaced);

        if target.exists() {
            std::fs::rename(target, &displaced).map_err(|error| fail(error.to_string()))?;
        }

        Some(displaced)
    };

    if let Err(error) = std::fs::rename(&staged, target) {
        let _ = std::fs::remove_file(&staged);

        #[cfg(windows)]
        if let Some(displaced) = displaced {
            let _ = std::fs::rename(&displaced, target);
        }

        return Err(fail(error.to_string()));
    }

    Ok(())
}

/// Run the update.
///
/// `check` stops after resolving the channel: nothing is downloaded and
/// nothing is written, which is what a script that wants to know whether an
/// update exists should call.
pub async fn run(
    channel: Option<String>,
    requested: Option<String>,
    check: bool,
    force: bool,
    json: bool,
) -> Result<Outcome, Box<dyn std::error::Error>> {
    // Progress is a diagnostic, not the answer. Under `--json` the answer is
    // the one document the caller printed at the end, so these lines move to
    // stderr rather than interleaving with it on stdout.
    let say = |line: String| {
        if json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    let platform = platform().ok_or_else(|| UpdateError::UnsupportedPlatform {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    })?;

    let updater = Updater::new(None, channel);
    let current = crate::VERSION;

    let version = match requested {
        Some(version) => {
            if !valid_version(&version) {
                return Err(Box::new(UpdateError::InvalidVersion(version)));
            }

            version
        }
        None => {
            let resolved = updater.resolve_channel().await?;

            say(format!(
                "Channel '{}' names {} ({} is installed).",
                updater.channel, resolved, current
            ));

            resolved
        }
    };

    if version == current && !force {
        say(format!("Already running {current}. Nothing to do."));

        return Ok(Outcome::AlreadyCurrent {
            version: version.clone(),
        });
    }

    let target_is_older = cmp_release_versions(&version, current) == Some(std::cmp::Ordering::Less);

    if target_is_older && !force {
        say(format!(
            "{version} is older than the installed {current}. Pass --force to install it anyway."
        ));

        if check {
            return Ok(Outcome::Older {
                version,
                installed: current.to_string(),
            });
        }

        return Err(Box::new(UpdateError::WouldDowngrade {
            current: current.to_string(),
            requested: version,
        }));
    }

    if check {
        if target_is_older {
            say(format!(
                "Would install older {version} over {current} (--force)."
            ));
        } else {
            say(format!("Update available: {current} -> {version}"));
        }

        return Ok(Outcome::Available { version });
    }

    let target = running_binary()?;

    say(format!(
        "Downloading {} ({platform})...",
        artifact_name(&version, &platform)
    ));

    let bytes = updater.fetch_verified(&version, &platform).await?;

    say(format!("  Verified sha256 {}.", hex_digest(&bytes)));

    replace_binary(&target, &bytes)?;

    say(format!("Replaced {}.", target.display()));
    say(format!("OpenAgents CLI is now {version}."));

    Ok(Outcome::Replaced {
        from: current.to_string(),
        to: version,
        path: target,
    })
}
