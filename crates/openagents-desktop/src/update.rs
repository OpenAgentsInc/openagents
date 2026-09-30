//! Auto-update for OpenAgents for Mac.
//!
//! The updater is Rust, not Sparkle. A release is described by a signed
//! manifest at [`MANIFEST_URL`]: an envelope that carries the manifest's
//! exact bytes, the ID of the key that signed them, and an Ed25519
//! signature. The app trusts only the public keys compiled into
//! [`TRUSTED_KEYS`]; `scripts/desktop/sign-manifest.sh` makes the envelope
//! with the matching private key, which never leaves the release machine.
//!
//! An update runs in four steps, and each step refuses rather than guesses:
//!
//! 1. [`Updater::check`] fetches the envelope, verifies the signature over
//!    the payload bytes before parsing them, and compares versions. A
//!    manifest older than the running app is refused as a downgrade, so a
//!    replayed old manifest cannot roll the app back.
//! 2. [`Updater::fetch`] downloads the archive into the cache. A cut
//!    connection leaves a `.part` file that the next attempt resumes with an
//!    HTTP range request. The archive's size and SHA-256 must match the
//!    signed manifest, or the file is deleted.
//! 3. The archive is extracted with `ditto`, and the new bundle must pass
//!    `codesign --verify --deep --strict`, Gatekeeper's notarization check
//!    (`spctl --assess`), and carry the manifest's Team ID, the running
//!    app's Team ID, the bundle ID, and the manifest's version.
//! 4. [`Updater::install`] copies the new bundle next to the running one,
//!    checks it again there, swaps the two with renames (restoring the old
//!    bundle if the second rename fails), and restarts the host agent.
//!    [`relaunch_after_exit`] then opens the new app once this process
//!    exits.
//!
//! Pairings survive an update: the host's keys live in the login keychain
//! and its grants in its state directory, never inside the bundle, and the
//! new bundle is signed by the same Team ID, so the keychain items stay
//! readable.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use ring::digest::{Context, SHA256};
use ring::signature::{ED25519, UnparsedPublicKey};
use semver::Version;
use serde::{Deserialize, Serialize};

/// Where the signed manifest for the macOS app is published: the public-read
/// bucket `openagentsgemini-oa-updates`, beside the builds it names
/// (`desktop/macos/VERSION/`).
pub const MANIFEST_URL: &str =
    "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/manifest.json";

/// The manifest payload's schema name.
pub const MANIFEST_SCHEMA: &str = "openagents.desktop.update.v1";

/// The only bundle ID an update may carry.
pub const BUNDLE_ID: &str = "com.openagents.desktop";

/// The OpenAgents Developer ID team. A manifest or bundle naming any other
/// team is refused, whatever key signed it.
pub const TEAM_ID: &str = "HQWSG26L43";

/// The launchd label of the host agent the app registers with
/// `SMAppService`, restarted after a swap so it runs the new `coder`.
pub const HOST_AGENT_LABEL: &str = "com.openagents.desktop.host";

/// How often a running app checks for an update.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// The largest envelope the updater reads. A real one is about 1 KiB.
const MAX_ENVELOPE_BYTES: u64 = 64 * 1024;

/// A public key the app accepts manifests from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrustedKey {
    /// The ID the envelope names in `key`.
    pub id: &'static str,
    /// The raw 32-byte Ed25519 public key.
    pub public: [u8; 32],
}

/// The keys compiled into the app. To rotate, add the new key here, ship a
/// release signed by the old key, then sign with the new key and remove the
/// old one in a later release. The private half is kept outside the
/// repository; `scripts/desktop/sign-manifest.sh` says where.
pub const TRUSTED_KEYS: &[TrustedKey] = &[TrustedKey {
    id: "desktop-update-2026-09",
    public: [
        0xb9, 0xc6, 0x88, 0xe6, 0xf3, 0x3b, 0x77, 0xf6, 0x3f, 0x61, 0xce, 0x46, 0xd1, 0xb5, 0x20,
        0xb2, 0xc5, 0x58, 0x8b, 0x79, 0xea, 0x4c, 0xcc, 0xaf, 0xc3, 0x0b, 0x12, 0x2f, 0x21, 0x1a,
        0x8b, 0x84,
    ],
}];

/// Why an update was refused or could not proceed.
#[derive(Debug)]
pub enum UpdateError {
    /// The envelope or payload is not what the schema says.
    Malformed(String),
    /// The envelope names a key the app does not trust.
    UnknownKey(String),
    /// The signature does not verify over the payload.
    BadSignature,
    /// The signed manifest names an older version than the running app.
    Downgrade { current: Version, offered: Version },
    /// The manifest has no build for this Mac's architecture.
    NoArtifact(String),
    /// The download stopped early; the partial file is kept for resuming.
    Interrupted {
        received: u64,
        expected: u64,
        cause: String,
    },
    /// The download is longer than the manifest says; it was deleted.
    TooLarge { expected: u64 },
    /// The download's SHA-256 is not the manifest's; it was deleted.
    DigestMismatch { expected: String, actual: String },
    /// The new bundle failed a code-signature, notarization, or identity
    /// check.
    CodeSignature(String),
    /// A local file or process step failed.
    Io(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(why) => write!(f, "the update manifest is malformed: {why}"),
            Self::UnknownKey(id) => {
                write!(f, "the update manifest is signed by an unknown key `{id}`")
            }
            Self::BadSignature => f.write_str("the update manifest's signature does not verify"),
            Self::Downgrade { current, offered } => {
                write!(
                    f,
                    "the update manifest offers {offered}, older than the running {current}"
                )
            }
            Self::NoArtifact(arch) => write!(f, "the update has no build for {arch}"),
            Self::Interrupted {
                received,
                expected,
                cause,
            } => {
                write!(
                    f,
                    "the download stopped at {received} of {expected} bytes: {cause}"
                )
            }
            Self::TooLarge { expected } => {
                write!(
                    f,
                    "the download is longer than the manifest's {expected} bytes"
                )
            }
            Self::DigestMismatch { expected, actual } => {
                write!(
                    f,
                    "the download's SHA-256 is {actual}, not the signed {expected}"
                )
            }
            Self::CodeSignature(why) => write!(f, "the new app failed its signature check: {why}"),
            Self::Io(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for UpdateError {}

fn io_error(context: &str, error: impl fmt::Display) -> UpdateError {
    UpdateError::Io(format!("{context}: {error}"))
}

/// The published file: the manifest's exact bytes and a signature over them.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    /// The signing key's ID, one of [`TRUSTED_KEYS`].
    pub key: String,
    /// Standard base64 of the manifest's JSON bytes.
    pub payload: String,
    /// Standard base64 of the 64-byte Ed25519 signature over those bytes.
    pub signature: String,
}

/// One release, as signed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Always [`MANIFEST_SCHEMA`].
    pub schema: String,
    /// Always [`BUNDLE_ID`].
    pub bundle_id: String,
    /// The release's version, `CFBundleShortVersionString` of the bundle.
    pub version: String,
    /// The Apple Developer Team ID that signed the bundle.
    pub team_id: String,
    /// When the release was signed, RFC 3339, for display.
    pub published: String,
    /// One build per architecture, or one `universal` build.
    pub artifacts: Vec<Artifact>,
}

/// One downloadable build.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// `universal`, `arm64`, or `x86_64`.
    pub arch: String,
    /// An `https` URL of a `ditto` zip of `OpenAgents.app`.
    pub url: String,
    /// Lowercase hex SHA-256 of the zip.
    pub sha256: String,
    /// The zip's length in bytes.
    pub size: u64,
}

/// A release the running app may install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub team_id: String,
    pub artifact: Artifact,
}

/// The result of a check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    /// The manifest names the running version.
    UpToDate,
    /// The manifest names a newer version.
    Available(Release),
}

/// Verifies an envelope against `keys` and returns the manifest it signs.
///
/// The signature is checked over the payload's bytes before they are
/// parsed, so a manifest that fails here was never interpreted.
pub fn verify_envelope(bytes: &[u8], keys: &[TrustedKey]) -> Result<Manifest, UpdateError> {
    let envelope: Envelope = serde_json::from_slice(bytes)
        .map_err(|error| UpdateError::Malformed(format!("envelope: {error}")))?;
    let key = keys
        .iter()
        .find(|key| key.id == envelope.key)
        .ok_or_else(|| UpdateError::UnknownKey(envelope.key.clone()))?;
    let payload = BASE64
        .decode(envelope.payload.as_bytes())
        .map_err(|error| UpdateError::Malformed(format!("payload: {error}")))?;
    let signature = BASE64
        .decode(envelope.signature.as_bytes())
        .map_err(|error| UpdateError::Malformed(format!("signature: {error}")))?;
    if signature.len() != 64 {
        return Err(UpdateError::BadSignature);
    }
    UnparsedPublicKey::new(&ED25519, key.public)
        .verify(&payload, &signature)
        .map_err(|_| UpdateError::BadSignature)?;
    let manifest: Manifest = serde_json::from_slice(&payload)
        .map_err(|error| UpdateError::Malformed(format!("manifest: {error}")))?;
    validate(&manifest)?;
    Ok(manifest)
}

fn validate(manifest: &Manifest) -> Result<(), UpdateError> {
    let bad = |why: String| Err(UpdateError::Malformed(why));
    if manifest.schema != MANIFEST_SCHEMA {
        return bad(format!("schema `{}`", manifest.schema));
    }
    if manifest.bundle_id != BUNDLE_ID {
        return bad(format!("bundle ID `{}`", manifest.bundle_id));
    }
    if let Err(error) = Version::parse(&manifest.version) {
        return bad(format!("version `{}`: {error}", manifest.version));
    }
    if manifest.team_id != TEAM_ID {
        return bad(format!("team ID `{}`", manifest.team_id));
    }
    if manifest.artifacts.is_empty() {
        return bad("no artifacts".into());
    }
    for artifact in &manifest.artifacts {
        if !matches!(artifact.arch.as_str(), "universal" | "arm64" | "x86_64") {
            return bad(format!("architecture `{}`", artifact.arch));
        }
        if !artifact.url.starts_with("https://") {
            return bad(format!("artifact URL `{}` is not https", artifact.url));
        }
        let hex = artifact.sha256.as_bytes();
        if hex.len() != 64
            || !hex
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        {
            return bad(format!("SHA-256 `{}`", artifact.sha256));
        }
        if artifact.size == 0 {
            return bad("an artifact of zero bytes".into());
        }
    }
    Ok(())
}

/// Decides what a verified manifest means for the running version `current`
/// on architecture `arch` (`arm64` or `x86_64`).
pub fn decide(manifest: &Manifest, current: &Version, arch: &str) -> Result<Check, UpdateError> {
    let offered = Version::parse(&manifest.version)
        .map_err(|error| UpdateError::Malformed(format!("version: {error}")))?;
    if offered < *current {
        return Err(UpdateError::Downgrade {
            current: current.clone(),
            offered,
        });
    }
    if offered == *current {
        return Ok(Check::UpToDate);
    }
    let artifact = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.arch == arch)
        .or_else(|| {
            manifest
                .artifacts
                .iter()
                .find(|artifact| artifact.arch == "universal")
        })
        .ok_or_else(|| UpdateError::NoArtifact(arch.to_owned()))?;
    Ok(Check::Available(Release {
        version: offered,
        team_id: manifest.team_id.clone(),
        artifact: artifact.clone(),
    }))
}

/// This Mac's architecture as the manifest names it.
pub fn this_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

/// An HTTP body, and whether it starts at the requested offset.
pub struct Body {
    /// `true` when the server honored the range (`206`); `false` when it
    /// sent the whole file from byte zero.
    pub partial: bool,
    pub reader: Box<dyn Read + Send>,
}

/// Fetches bytes. The real transport is [`HttpTransport`]; tests use a
/// fake that cuts the connection.
pub trait Transport: Send + Sync {
    /// Opens `url` from byte `from` (a range request when `from > 0`).
    fn open(&self, url: &str, from: u64) -> io::Result<Body>;
}

/// The HTTPS transport.
pub struct HttpTransport {
    client: reqwest::blocking::Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, UpdateError> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(30 * 60))
            .user_agent(concat!("OpenAgents-Desktop/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| io_error("could not start the update client", error))?;
        Ok(Self { client })
    }
}

impl Transport for HttpTransport {
    fn open(&self, url: &str, from: u64) -> io::Result<Body> {
        let mut request = self.client.get(url);
        if from > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={from}-"));
        }
        let response = request.send().map_err(io::Error::other)?;
        let status = response.status();
        let partial = status == reqwest::StatusCode::PARTIAL_CONTENT;
        if !status.is_success() {
            return Err(io::Error::other(format!("{url} answered {status}")));
        }
        Ok(Body {
            partial,
            reader: Box::new(response),
        })
    }
}

/// Downloads `artifact` into `dir` and returns the verified archive's path.
///
/// A `.part` file from an interrupted attempt is resumed. The finished file
/// must be exactly `artifact.size` bytes with SHA-256 `artifact.sha256`;
/// otherwise it is deleted. An archive already in place is checked again
/// and reused.
pub fn download(
    transport: &dyn Transport,
    artifact: &Artifact,
    version: &Version,
    dir: &Path,
) -> Result<PathBuf, UpdateError> {
    fs::create_dir_all(dir).map_err(|error| io_error("could not make the update cache", error))?;
    let done = dir.join(format!("OpenAgents-{version}-{}.zip", artifact.arch));
    if done.exists() {
        if sha256_file(&done)? == artifact.sha256 {
            return Ok(done);
        }
        let _ = fs::remove_file(&done);
    }
    let part = done.with_extension("zip.part");
    let mut from = fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0);
    if from > artifact.size {
        let _ = fs::remove_file(&part);
        from = 0;
    }
    if from < artifact.size {
        let interrupted = |received: u64, cause: String| UpdateError::Interrupted {
            received,
            expected: artifact.size,
            cause,
        };
        let body = transport
            .open(&artifact.url, from)
            .map_err(|error| interrupted(from, error.to_string()))?;
        if !body.partial {
            from = 0;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&part)
            .map_err(|error| io_error("could not open the partial download", error))?;
        file.set_len(from)
            .map_err(|error| io_error("could not trim the partial download", error))?;
        file.seek(SeekFrom::Start(from))
            .map_err(|error| io_error("could not seek the partial download", error))?;
        // Read one byte past the expected length so a longer body is caught.
        let mut limited = body.reader.take(artifact.size - from + 1);
        let copied = io::copy(&mut limited, &mut file);
        file.sync_all()
            .map_err(|error| io_error("could not flush the download", error))?;
        let received = fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0);
        if received > artifact.size {
            drop(file);
            let _ = fs::remove_file(&part);
            return Err(UpdateError::TooLarge {
                expected: artifact.size,
            });
        }
        if let Err(error) = copied {
            return Err(interrupted(received, error.to_string()));
        }
        if received < artifact.size {
            return Err(interrupted(received, "the connection closed early".into()));
        }
    }
    let actual = sha256_file(&part)?;
    if actual != artifact.sha256 {
        let _ = fs::remove_file(&part);
        return Err(UpdateError::DigestMismatch {
            expected: artifact.sha256.clone(),
            actual,
        });
    }
    fs::rename(&part, &done).map_err(|error| io_error("could not keep the download", error))?;
    Ok(done)
}

/// Lowercase hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String, UpdateError> {
    let mut file =
        File::open(path).map_err(|error| io_error("could not read the download", error))?;
    let mut context = Context::new(&SHA256);
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io_error("could not read the download", error))?;
        if read == 0 {
            break;
        }
        context.update(&buffer[..read]);
    }
    Ok(context
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// What a bundle's signature says about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeIdentity {
    pub team_id: String,
    pub bundle_id: String,
    pub version: String,
}

/// Checks a bundle's code signature and notarization and reads its
/// identity. The real inspector is [`MacInspector`].
pub trait Inspector: Send + Sync {
    fn inspect(&self, app: &Path) -> Result<CodeIdentity, UpdateError>;
}

/// `codesign`, `spctl`, and `plutil`.
pub struct MacInspector;

impl Inspector for MacInspector {
    fn inspect(&self, app: &Path) -> Result<CodeIdentity, UpdateError> {
        let run = |program: &str, args: &[&std::ffi::OsStr]| {
            Command::new(program)
                .args(args)
                .output()
                .map_err(|error| io_error(&format!("could not run {program}"), error))
        };
        let app_arg = app.as_os_str();
        let verify = run(
            "/usr/bin/codesign",
            &[
                "--verify".as_ref(),
                "--deep".as_ref(),
                "--strict".as_ref(),
                app_arg,
            ],
        )?;
        if !verify.status.success() {
            return Err(UpdateError::CodeSignature(format!(
                "codesign --verify: {}",
                String::from_utf8_lossy(&verify.stderr).trim()
            )));
        }
        let assess = run(
            "/usr/sbin/spctl",
            &[
                "--assess".as_ref(),
                "--type".as_ref(),
                "execute".as_ref(),
                "-v".as_ref(),
                app_arg,
            ],
        )?;
        let assessment = String::from_utf8_lossy(&assess.stderr).into_owned();
        if !assess.status.success() || !assessment.contains("Notarized Developer ID") {
            return Err(UpdateError::CodeSignature(format!(
                "Gatekeeper did not accept it as notarized: {}",
                assessment.trim()
            )));
        }
        let describe = run(
            "/usr/bin/codesign",
            &["-d".as_ref(), "--verbose=4".as_ref(), app_arg],
        )?;
        let details = String::from_utf8_lossy(&describe.stderr).into_owned();
        let team_id = field(&details, "TeamIdentifier=")
            .filter(|team| team != "not set")
            .ok_or_else(|| UpdateError::CodeSignature("the signature has no Team ID".into()))?;
        let plist = app.join("Contents/Info.plist");
        let read_key = |key: &str| -> Result<String, UpdateError> {
            let output = run(
                "/usr/bin/plutil",
                &[
                    "-extract".as_ref(),
                    key.as_ref(),
                    "raw".as_ref(),
                    plist.as_os_str(),
                ],
            )?;
            if !output.status.success() {
                return Err(UpdateError::CodeSignature(format!(
                    "Info.plist has no {key}"
                )));
            }
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        };
        Ok(CodeIdentity {
            team_id,
            bundle_id: read_key("CFBundleIdentifier")?,
            version: read_key("CFBundleShortVersionString")?,
        })
    }
}

fn field(text: &str, prefix: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix(prefix))
        .map(|value| value.trim().to_owned())
}

/// Refuses a bundle whose identity is not the release's: another bundle ID,
/// another version, or a Team ID other than the manifest's and the running
/// app's.
pub fn check_identity(
    identity: &CodeIdentity,
    release: &Release,
    running_team: Option<&str>,
) -> Result<(), UpdateError> {
    if identity.bundle_id != BUNDLE_ID {
        return Err(UpdateError::CodeSignature(format!(
            "bundle ID `{}`",
            identity.bundle_id
        )));
    }
    if identity.version != release.version.to_string() {
        return Err(UpdateError::CodeSignature(format!(
            "the bundle is version {}, the manifest says {}",
            identity.version, release.version
        )));
    }
    if identity.team_id != release.team_id {
        return Err(UpdateError::CodeSignature(format!(
            "signed by Team ID {}, the manifest says {}",
            identity.team_id, release.team_id
        )));
    }
    if let Some(running) = running_team
        && identity.team_id != running
    {
        return Err(UpdateError::CodeSignature(format!(
            "signed by Team ID {}, this app by {running}",
            identity.team_id
        )));
    }
    Ok(())
}

/// A downloaded, extracted, and checked bundle waiting to be installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    pub version: Version,
    pub app: PathBuf,
    release: Release,
}

/// The updater for one running app.
pub struct Updater {
    manifest_url: String,
    keys: &'static [TrustedKey],
    current: Version,
    arch: &'static str,
    cache: PathBuf,
    transport: Box<dyn Transport>,
    inspector: Box<dyn Inspector>,
}

impl Updater {
    /// The updater for this process: the published manifest, the compiled
    /// keys, this crate's version, and `~/Library/Caches/com.openagents.desktop/updates`.
    pub fn for_this_app() -> Result<Self, UpdateError> {
        let home =
            std::env::var_os("HOME").ok_or_else(|| UpdateError::Io("HOME is not set".into()))?;
        let cache = PathBuf::from(home)
            .join("Library/Caches")
            .join(BUNDLE_ID)
            .join("updates");
        let current = Version::parse(env!("CARGO_PKG_VERSION"))
            .map_err(|error| UpdateError::Malformed(format!("this app's version: {error}")))?;
        Ok(Self::new(
            MANIFEST_URL.into(),
            TRUSTED_KEYS,
            current,
            cache,
            Box::new(HttpTransport::new()?),
            Box::new(MacInspector),
        ))
    }

    pub fn new(
        manifest_url: String,
        keys: &'static [TrustedKey],
        current: Version,
        cache: PathBuf,
        transport: Box<dyn Transport>,
        inspector: Box<dyn Inspector>,
    ) -> Self {
        Self {
            manifest_url,
            keys,
            current,
            arch: this_arch(),
            cache,
            transport,
            inspector,
        }
    }

    /// The running version.
    pub fn current(&self) -> &Version {
        &self.current
    }

    /// Fetches and verifies the manifest and decides whether to update.
    pub fn check(&self) -> Result<Check, UpdateError> {
        let body = self
            .transport
            .open(&self.manifest_url, 0)
            .map_err(|error| io_error("could not fetch the update manifest", error))?;
        let mut bytes = Vec::new();
        body.reader
            .take(MAX_ENVELOPE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error("could not read the update manifest", error))?;
        if bytes.len() as u64 > MAX_ENVELOPE_BYTES {
            return Err(UpdateError::Malformed("the envelope is too large".into()));
        }
        let manifest = verify_envelope(&bytes, self.keys)?;
        decide(&manifest, &self.current, self.arch)
    }

    /// Downloads, extracts, and checks `release`. Old downloads in the cache
    /// are removed first.
    pub fn fetch(&self, release: &Release) -> Result<Staged, UpdateError> {
        self.prune(&release.version);
        let archive = download(
            &*self.transport,
            &release.artifact,
            &release.version,
            &self.cache,
        )?;
        let unpacked = self.cache.join(format!("OpenAgents-{}", release.version));
        let _ = fs::remove_dir_all(&unpacked);
        fs::create_dir_all(&unpacked)
            .map_err(|error| io_error("could not make the unpack folder", error))?;
        let status = Command::new("/usr/bin/ditto")
            .args(["-x", "-k"])
            .arg(&archive)
            .arg(&unpacked)
            .status()
            .map_err(|error| io_error("could not run ditto", error))?;
        if !status.success() {
            let _ = fs::remove_file(&archive);
            return Err(UpdateError::Io(format!(
                "ditto could not unpack the update ({status})"
            )));
        }
        let app = single_app(&unpacked)?;
        let identity = self.inspector.inspect(&app)?;
        check_identity(
            &identity,
            release,
            running_team(&*self.inspector).as_deref(),
        )?;
        Ok(Staged {
            version: release.version.clone(),
            app,
            release: release.clone(),
        })
    }

    /// Replaces the running bundle with `staged` and restarts the host
    /// agent. The caller then calls [`relaunch_after_exit`] and exits.
    pub fn install(&self, staged: &Staged) -> Result<PathBuf, UpdateError> {
        let current = running_bundle()
            .ok_or_else(|| UpdateError::Io("this app is not running from a bundle".into()))?;
        swap_bundle(&current, &staged.app, |app| {
            let identity = self.inspector.inspect(app)?;
            check_identity(
                &identity,
                &staged.release,
                running_team(&*self.inspector).as_deref(),
            )
        })?;
        let _ = fs::remove_dir_all(staged.app.parent().unwrap_or(&staged.app));
        restart_host_agent();
        Ok(current)
    }

    fn prune(&self, keep: &Version) {
        let Ok(entries) = fs::read_dir(&self.cache) else {
            return;
        };
        let keep = format!("OpenAgents-{keep}");
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&keep)
                && name[keep.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| c == '-' || c == '.')
            {
                continue;
            }
            let path = entry.path();
            let _ = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
        }
    }
}

fn single_app(dir: &Path) -> Result<PathBuf, UpdateError> {
    let apps: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|error| io_error("could not read the unpacked update", error))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "app"))
        .collect();
    match apps.as_slice() {
        [app] => Ok(app.clone()),
        _ => Err(UpdateError::Malformed(format!(
            "the archive holds {} apps, not one",
            apps.len()
        ))),
    }
}

/// The `.app` this process runs from, if any.
pub fn running_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let app = exe.parent()?.parent()?.parent()?;
    (app.extension()? == "app").then(|| app.to_path_buf())
}

fn running_team(inspector: &dyn Inspector) -> Option<String> {
    let app = running_bundle()?;
    inspector
        .inspect(&app)
        .ok()
        .map(|identity| identity.team_id)
}

/// Swaps `staged` into `current`'s place.
///
/// The new bundle is first copied with `ditto` (which keeps signatures and
/// extended attributes) beside `current`, so the swap is two renames on one
/// volume; `check` runs on that copy. If the second rename fails, the old
/// bundle is renamed back.
pub fn swap_bundle(
    current: &Path,
    staged: &Path,
    check: impl Fn(&Path) -> Result<(), UpdateError>,
) -> Result<(), UpdateError> {
    let parent = current
        .parent()
        .ok_or_else(|| UpdateError::Io("the app has no parent folder".into()))?;
    let name = current
        .file_name()
        .ok_or_else(|| UpdateError::Io("the app has no name".into()))?
        .to_string_lossy()
        .into_owned();
    let incoming = parent.join(format!(".{name}.incoming"));
    let previous = parent.join(format!(".{name}.previous"));
    let _ = fs::remove_dir_all(&incoming);
    let status = Command::new("/usr/bin/ditto")
        .arg(staged)
        .arg(&incoming)
        .status()
        .map_err(|error| io_error("could not run ditto", error))?;
    if !status.success() {
        let _ = fs::remove_dir_all(&incoming);
        return Err(UpdateError::Io(format!(
            "could not copy the update into {} ({status}); the folder may need permission",
            parent.display()
        )));
    }
    if let Err(error) = check(&incoming) {
        let _ = fs::remove_dir_all(&incoming);
        return Err(error);
    }
    let _ = fs::remove_dir_all(&previous);
    fs::rename(current, &previous).map_err(|error| {
        let _ = fs::remove_dir_all(&incoming);
        io_error("could not move the running app aside", error)
    })?;
    if let Err(error) = fs::rename(&incoming, current) {
        let restored = fs::rename(&previous, current);
        let _ = fs::remove_dir_all(&incoming);
        return Err(io_error(
            if restored.is_ok() {
                "could not put the update in place; the old app is back"
            } else {
                "could not put the update in place or restore the old app"
            },
            error,
        ));
    }
    let _ = fs::remove_dir_all(&previous);
    Ok(())
}

/// Restarts the host agent so it runs the new bundle's `coder`. Best
/// effort: when the agent is not registered there is nothing to restart.
pub fn restart_host_agent() {
    let Ok(output) = Command::new("/usr/bin/id").arg("-u").output() else {
        return;
    };
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if uid.is_empty() {
        return;
    }
    let _ = Command::new("/bin/launchctl")
        .args(["kickstart", "-k", &format!("gui/{uid}/{HOST_AGENT_LABEL}")])
        .status();
}

/// Opens `app` once this process has exited. The caller exits right after.
pub fn relaunch_after_exit(app: &Path) -> Result<(), UpdateError> {
    Command::new("/bin/sh")
        .args([
            "-c",
            "while /bin/kill -0 \"$1\" 2>/dev/null; do /bin/sleep 0.2; done; exec /usr/bin/open \"$2\"",
            "openagents-relaunch",
        ])
        .arg(std::process::id().to_string())
        .arg(app)
        .spawn()
        .map_err(|error| io_error("could not schedule the relaunch", error))?;
    Ok(())
}

/// What the updater is doing, for the menu bar and window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Downloading(Version),
    /// A checked bundle is waiting; the menu offers to restart into it.
    Ready(Staged),
    /// The last attempt failed; the next check retries.
    Failed(String),
}

/// Checks now and then every [`CHECK_INTERVAL`] on a background thread,
/// downloading and checking each new release. `report` receives each state;
/// installing a [`UpdateState::Ready`] bundle is the caller's choice.
pub fn spawn_checker(
    updater: std::sync::Arc<Updater>,
    report: impl Fn(UpdateState) + Send + 'static,
) -> io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("openagents-update".into())
        .spawn(move || {
            loop {
                report(run_once(&updater, &report));
                std::thread::sleep(CHECK_INTERVAL);
            }
        })
}

/// One check, download, and staging pass.
pub fn run_once(updater: &Updater, report: &dyn Fn(UpdateState)) -> UpdateState {
    report(UpdateState::Checking);
    match updater.check() {
        Ok(Check::UpToDate) => UpdateState::UpToDate,
        Ok(Check::Available(release)) => {
            report(UpdateState::Downloading(release.version.clone()));
            match updater.fetch(&release) {
                Ok(staged) => UpdateState::Ready(staged),
                Err(error) => UpdateState::Failed(error.to_string()),
            }
        }
        Err(error) => UpdateState::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};
    use std::sync::Mutex;

    fn key_pair() -> Ed25519KeyPair {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).expect("pkcs8");
        Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).expect("key pair")
    }

    fn trusted(pair: &Ed25519KeyPair) -> &'static [TrustedKey] {
        let public: [u8; 32] = pair.public_key().as_ref().try_into().expect("32 bytes");
        Box::leak(Box::new([TrustedKey {
            id: "test-key",
            public,
        }]))
    }

    fn manifest(version: &str, sha256: &str, size: u64) -> Manifest {
        Manifest {
            schema: MANIFEST_SCHEMA.into(),
            bundle_id: BUNDLE_ID.into(),
            version: version.into(),
            team_id: TEAM_ID.into(),
            published: "2026-09-29T00:00:00Z".into(),
            artifacts: vec![Artifact {
                arch: "universal".into(),
                url: "https://example.invalid/OpenAgents.zip".into(),
                sha256: sha256.into(),
                size,
            }],
        }
    }

    fn seal(pair: &Ed25519KeyPair, key: &str, payload: &[u8]) -> Vec<u8> {
        serde_json::to_vec(&Envelope {
            key: key.into(),
            payload: BASE64.encode(payload),
            signature: BASE64.encode(pair.sign(payload).as_ref()),
        })
        .expect("envelope")
    }

    fn digest(bytes: &[u8]) -> String {
        ring::digest::digest(&SHA256, bytes)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Signed by the real release key with the same `openssl` commands
    /// `scripts/desktop/sign-manifest.sh` runs, so the script and the
    /// compiled key agree.
    const RELEASE_KEY_FIXTURE: &str = r#"{"key":"desktop-update-2026-09","payload":"eyJzY2hlbWEiOiJvcGVuYWdlbnRzLmRlc2t0b3AudXBkYXRlLnYxIiwiYnVuZGxlX2lkIjoiY29tLm9wZW5hZ2VudHMuZGVza3RvcCIsInZlcnNpb24iOiIwLjAuMS1maXh0dXJlIiwidGVhbV9pZCI6IkhRV1NHMjZMNDMiLCJwdWJsaXNoZWQiOiIyMDI2LTA5LTI5VDAwOjAwOjAwWiIsImFydGlmYWN0cyI6W3siYXJjaCI6InVuaXZlcnNhbCIsInVybCI6Imh0dHBzOi8vc3RvcmFnZS5nb29nbGVhcGlzLmNvbS9vcGVuYWdlbnRzZ2VtaW5pLW9hLXVwZGF0ZXMvZGVza3RvcC9tYWNvcy8wLjAuMS1maXh0dXJlL09wZW5BZ2VudHMtMC4wLjEtZml4dHVyZS11bml2ZXJzYWwuemlwIiwic2hhMjU2IjoiMmQ3MTE2NDJiNzI2YjA0NDAxNjI3Y2E5ZmJhYzMyZjVjODUzMGZiMTkwM2NjNGRiMDIyNTg3MTc5MjFhNDg4MSIsInNpemUiOjF9XX0=","signature":"s14Rhp7Mecv4SO1NF8dzbOXgmBGd4Y//p6mSk06uSLWTe2OdFVvV7xXib3UO0kddiLZIEvdep/vSmrQhS/3MCw=="}"#;

    #[test]
    fn the_release_key_signs_what_the_app_accepts() {
        let manifest = verify_envelope(RELEASE_KEY_FIXTURE.as_bytes(), TRUSTED_KEYS).unwrap();
        assert_eq!(manifest.version, "0.0.1-fixture");
        // And a fixture version is older than any real release.
        let current = Version::parse("0.1.0").unwrap();
        assert!(matches!(
            decide(&manifest, &current, "arm64"),
            Err(UpdateError::Downgrade { .. })
        ));
    }

    #[test]
    fn the_compiled_key_is_a_valid_ed25519_point() {
        // A signature check against it must reach verification, not fail on
        // the key: a well-formed but wrong signature is simply refused.
        let result =
            UnparsedPublicKey::new(&ED25519, TRUSTED_KEYS[0].public).verify(b"x", &[0; 64]);
        assert!(result.is_err());
        assert_eq!(TRUSTED_KEYS.len(), 1);
    }

    #[test]
    fn a_signed_manifest_verifies() {
        let pair = key_pair();
        let signed = manifest("1.2.0", &"a".repeat(64), 10);
        let bytes = seal(&pair, "test-key", &serde_json::to_vec(&signed).unwrap());
        assert_eq!(verify_envelope(&bytes, trusted(&pair)).unwrap(), signed);
    }

    #[test]
    fn a_tampered_manifest_is_refused() {
        let pair = key_pair();
        let keys = trusted(&pair);
        let payload = serde_json::to_vec(&manifest("1.2.0", &"a".repeat(64), 10)).unwrap();
        let good: Envelope = serde_json::from_slice(&seal(&pair, "test-key", &payload)).unwrap();

        // The payload changed after signing (another artifact digest).
        let forged = serde_json::to_vec(&manifest("1.2.0", &"b".repeat(64), 10)).unwrap();
        let tampered = Envelope {
            payload: BASE64.encode(&forged),
            ..good.clone()
        };
        let error = verify_envelope(&serde_json::to_vec(&tampered).unwrap(), keys).unwrap_err();
        assert!(matches!(error, UpdateError::BadSignature), "{error}");

        // One bit of the signature flipped.
        let mut signature = BASE64.decode(&good.signature).unwrap();
        signature[5] ^= 1;
        let flipped = Envelope {
            signature: BASE64.encode(&signature),
            ..good.clone()
        };
        let error = verify_envelope(&serde_json::to_vec(&flipped).unwrap(), keys).unwrap_err();
        assert!(matches!(error, UpdateError::BadSignature), "{error}");

        // Signed by a key the app does not trust, under the trusted ID.
        let stranger = seal(&key_pair(), "test-key", &payload);
        let error = verify_envelope(&stranger, keys).unwrap_err();
        assert!(matches!(error, UpdateError::BadSignature), "{error}");

        // Naming an unknown key.
        let error = verify_envelope(&seal(&pair, "other", &payload), keys).unwrap_err();
        assert!(matches!(error, UpdateError::UnknownKey(_)), "{error}");

        // A short signature.
        let short = Envelope {
            signature: BASE64.encode([0u8; 12]),
            ..good
        };
        let error = verify_envelope(&serde_json::to_vec(&short).unwrap(), keys).unwrap_err();
        assert!(matches!(error, UpdateError::BadSignature), "{error}");

        // Not JSON at all.
        assert!(matches!(
            verify_envelope(b"nope", keys),
            Err(UpdateError::Malformed(_))
        ));
    }

    #[test]
    fn a_signed_manifest_with_bad_fields_is_refused() {
        let pair = key_pair();
        let keys = trusted(&pair);
        let mut cases = Vec::new();
        let mut m = manifest("1.2.0", &"a".repeat(64), 10);
        m.bundle_id = "com.example.other".into();
        cases.push(m);
        let mut m = manifest("1.2.0", &"a".repeat(64), 10);
        m.artifacts[0].url = "http://example.invalid/x.zip".into();
        cases.push(m);
        let mut m = manifest("1.2.0", &"a".repeat(64), 10);
        m.team_id = "ZZZZZ99999".into();
        cases.push(m);
        cases.push(manifest("not-a-version", &"a".repeat(64), 10));
        cases.push(manifest("1.2.0", "ABC", 10));
        cases.push(manifest("1.2.0", &"a".repeat(64), 0));
        let mut m = manifest("1.2.0", &"a".repeat(64), 10);
        m.schema = "openagents.desktop.update.v0".into();
        cases.push(m);
        for case in cases {
            let bytes = seal(&pair, "test-key", &serde_json::to_vec(&case).unwrap());
            assert!(
                matches!(
                    verify_envelope(&bytes, keys),
                    Err(UpdateError::Malformed(_))
                ),
                "{case:?}"
            );
        }
        // An unknown field is refused rather than ignored.
        let mut value = serde_json::to_value(manifest("1.2.0", &"a".repeat(64), 10)).unwrap();
        value["channel"] = "beta".into();
        let bytes = seal(&pair, "test-key", &serde_json::to_vec(&value).unwrap());
        assert!(matches!(
            verify_envelope(&bytes, keys),
            Err(UpdateError::Malformed(_))
        ));
    }

    #[test]
    fn a_downgrade_is_refused() {
        let current = Version::parse("1.2.0").unwrap();
        let older = manifest("1.1.9", &"a".repeat(64), 10);
        assert!(matches!(
            decide(&older, &current, "arm64"),
            Err(UpdateError::Downgrade { .. })
        ));
        // A prerelease of the running version is older than it.
        let prerelease = manifest("1.2.0-rc.1", &"a".repeat(64), 10);
        assert!(matches!(
            decide(&prerelease, &current, "arm64"),
            Err(UpdateError::Downgrade { .. })
        ));
        assert_eq!(
            decide(&manifest("1.2.0", &"a".repeat(64), 10), &current, "arm64").unwrap(),
            Check::UpToDate
        );
        let Check::Available(release) =
            decide(&manifest("1.3.0", &"a".repeat(64), 10), &current, "arm64").unwrap()
        else {
            panic!("expected an update");
        };
        assert_eq!(release.version, Version::parse("1.3.0").unwrap());
        assert_eq!(release.artifact.arch, "universal");
    }

    #[test]
    fn a_0_1_0_install_upgrades_to_1_0_0() {
        // The first public build was 0.1.0; the desktop app then joined the
        // phone app's version, 1.0.0. Installed 0.1.0 apps must take it.
        let installed = Version::parse("0.1.0").unwrap();
        let Check::Available(release) =
            decide(&manifest("1.0.0", &"a".repeat(64), 10), &installed, "arm64").unwrap()
        else {
            panic!("expected 0.1.0 to update to 1.0.0");
        };
        assert_eq!(release.version, Version::parse("1.0.0").unwrap());
        let running = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        assert!(running >= Version::parse("1.0.0").unwrap());
        assert!(matches!(
            decide(&manifest("0.1.0", &"a".repeat(64), 10), &running, "arm64"),
            Err(UpdateError::Downgrade { .. })
        ));
    }

    #[test]
    fn an_update_has_to_match_the_architecture() {
        let current = Version::parse("1.0.0").unwrap();
        let mut m = manifest("1.1.0", &"a".repeat(64), 10);
        m.artifacts[0].arch = "x86_64".into();
        assert!(matches!(
            decide(&m, &current, "arm64"),
            Err(UpdateError::NoArtifact(_))
        ));
        let mut arm = m.artifacts[0].clone();
        arm.arch = "arm64".into();
        arm.url = "https://example.invalid/arm.zip".into();
        m.artifacts.push(arm);
        let Check::Available(release) = decide(&m, &current, "arm64").unwrap() else {
            panic!()
        };
        assert_eq!(release.artifact.url, "https://example.invalid/arm.zip");
    }

    /// Serves `bytes`, cutting each connection after `cut` bytes, and
    /// records the offsets asked for.
    struct Fake {
        bytes: Vec<u8>,
        cut: Option<usize>,
        honor_range: bool,
        asked: Mutex<Vec<u64>>,
    }

    struct Cut {
        data: io::Cursor<Vec<u8>>,
        fail: bool,
    }

    impl Read for Cut {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let read = self.data.read(buf)?;
            if read == 0 && self.fail {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionReset,
                    "connection reset",
                ));
            }
            Ok(read)
        }
    }

    impl Transport for Fake {
        fn open(&self, _url: &str, from: u64) -> io::Result<Body> {
            self.asked.lock().unwrap().push(from);
            let start = if self.honor_range { from as usize } else { 0 };
            let mut data = self.bytes[start..].to_vec();
            let fail = self.cut.is_some_and(|cut| cut < data.len());
            if let Some(cut) = self.cut {
                data.truncate(cut);
            }
            Ok(Body {
                partial: self.honor_range && from > 0,
                reader: Box::new(Cut {
                    data: io::Cursor::new(data),
                    fail,
                }),
            })
        }
    }

    fn fake(bytes: &[u8], cut: Option<usize>, honor_range: bool) -> Fake {
        Fake {
            bytes: bytes.to_vec(),
            cut,
            honor_range,
            asked: Mutex::new(Vec::new()),
        }
    }

    fn artifact_for(bytes: &[u8]) -> Artifact {
        Artifact {
            arch: "universal".into(),
            url: "https://example.invalid/OpenAgents.zip".into(),
            sha256: digest(bytes),
            size: bytes.len() as u64,
        }
    }

    #[test]
    fn an_interrupted_download_resumes_and_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let build: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        let artifact = artifact_for(&build);
        let version = Version::parse("1.3.0").unwrap();

        let first = fake(&build, Some(40_000), true);
        let error = download(&first, &artifact, &version, dir.path()).unwrap_err();
        let UpdateError::Interrupted {
            received, expected, ..
        } = error
        else {
            panic!("{error}")
        };
        assert_eq!((received, expected), (40_000, 100_000));
        let part = dir.path().join("OpenAgents-1.3.0-universal.zip.part");
        assert_eq!(fs::metadata(&part).unwrap().len(), 40_000);
        assert!(!dir.path().join("OpenAgents-1.3.0-universal.zip").exists());

        let second = fake(&build, None, true);
        let done = download(&second, &artifact, &version, dir.path()).unwrap();
        assert_eq!(*second.asked.lock().unwrap(), vec![40_000]);
        assert_eq!(fs::read(&done).unwrap(), build);
        assert!(!part.exists());

        // A finished archive is reused without a request.
        let third = fake(&build, None, true);
        assert_eq!(
            download(&third, &artifact, &version, dir.path()).unwrap(),
            done
        );
        assert!(third.asked.lock().unwrap().is_empty());
    }

    #[test]
    fn a_server_that_ignores_the_range_restarts_the_download() {
        let dir = tempfile::tempdir().unwrap();
        let build = vec![7u8; 5_000];
        let artifact = artifact_for(&build);
        let version = Version::parse("1.3.0").unwrap();
        let _ = download(
            &fake(&build, Some(1_000), false),
            &artifact,
            &version,
            dir.path(),
        )
        .unwrap_err();
        let done = download(&fake(&build, None, false), &artifact, &version, dir.path()).unwrap();
        assert_eq!(fs::read(done).unwrap(), build);
    }

    #[test]
    fn a_tampered_build_is_refused_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let build = vec![1u8; 4_096];
        let artifact = artifact_for(&build);
        let version = Version::parse("1.3.0").unwrap();
        let mut tampered = build.clone();
        tampered[100] ^= 0xff;
        let error = download(
            &fake(&tampered, None, true),
            &artifact,
            &version,
            dir.path(),
        )
        .unwrap_err();
        assert!(
            matches!(error, UpdateError::DigestMismatch { .. }),
            "{error}"
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);

        // Longer than signed.
        let mut longer = build.clone();
        longer.extend_from_slice(b"extra");
        let error =
            download(&fake(&longer, None, true), &artifact, &version, dir.path()).unwrap_err();
        assert!(matches!(error, UpdateError::TooLarge { .. }), "{error}");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);

        // An archive already in the cache that was altered is fetched again.
        let done = dir.path().join("OpenAgents-1.3.0-universal.zip");
        fs::write(&done, &tampered).unwrap();
        let path = download(&fake(&build, None, true), &artifact, &version, dir.path()).unwrap();
        assert_eq!(fs::read(path).unwrap(), build);
    }

    fn release(version: &str) -> Release {
        Release {
            version: Version::parse(version).unwrap(),
            team_id: TEAM_ID.into(),
            artifact: artifact_for(b"x"),
        }
    }

    #[test]
    fn a_build_with_another_identity_is_refused() {
        let good = CodeIdentity {
            team_id: TEAM_ID.into(),
            bundle_id: BUNDLE_ID.into(),
            version: "1.3.0".into(),
        };
        let release = release("1.3.0");
        check_identity(&good, &release, Some(TEAM_ID)).unwrap();
        check_identity(&good, &release, None).unwrap();
        for bad in [
            CodeIdentity {
                team_id: "ZZZZZ99999".into(),
                ..good.clone()
            },
            CodeIdentity {
                bundle_id: "com.example.evil".into(),
                ..good.clone()
            },
            CodeIdentity {
                version: "1.2.0".into(),
                ..good.clone()
            },
        ] {
            assert!(matches!(
                check_identity(&bad, &release, Some(TEAM_ID)),
                Err(UpdateError::CodeSignature(_))
            ));
        }
        // Signed by the manifest's team, but not the running app's.
        assert!(check_identity(&good, &release, Some("QQQQQ11111")).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_swap_replaces_the_bundle_or_leaves_it_alone() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("OpenAgents.app");
        fs::create_dir_all(current.join("Contents")).unwrap();
        fs::write(current.join("Contents/version"), "1.2.0").unwrap();
        let staged = dir.path().join("staged/OpenAgents.app");
        fs::create_dir_all(staged.join("Contents")).unwrap();
        fs::write(staged.join("Contents/version"), "1.3.0").unwrap();

        let refused = swap_bundle(&current, &staged, |_| {
            Err(UpdateError::CodeSignature("no".into()))
        });
        assert!(refused.is_err());
        assert_eq!(
            fs::read_to_string(current.join("Contents/version")).unwrap(),
            "1.2.0"
        );
        assert!(!dir.path().join(".OpenAgents.app.incoming").exists());

        swap_bundle(&current, &staged, |_| Ok(())).unwrap();
        assert_eq!(
            fs::read_to_string(current.join("Contents/version")).unwrap(),
            "1.3.0"
        );
        assert!(!dir.path().join(".OpenAgents.app.previous").exists());
        assert!(!dir.path().join(".OpenAgents.app.incoming").exists());
    }

    #[test]
    fn the_check_uses_the_transport_and_the_keys() {
        let pair = key_pair();
        let signed = manifest("9.0.0", &"a".repeat(64), 10);
        let envelope = seal(&pair, "test-key", &serde_json::to_vec(&signed).unwrap());
        let updater = Updater::new(
            "https://example.invalid/manifest.json".into(),
            trusted(&pair),
            Version::parse("1.0.0").unwrap(),
            tempfile::tempdir().unwrap().keep(),
            Box::new(fake(&envelope, None, true)),
            Box::new(MacInspector),
        );
        assert!(matches!(updater.check().unwrap(), Check::Available(r) if r.version.major == 9));

        // The same envelope checked against the compiled keys is refused.
        let updater = Updater::new(
            "https://example.invalid/manifest.json".into(),
            TRUSTED_KEYS,
            Version::parse("1.0.0").unwrap(),
            tempfile::tempdir().unwrap().keep(),
            Box::new(fake(&envelope, None, true)),
            Box::new(MacInspector),
        );
        assert!(matches!(updater.check(), Err(UpdateError::UnknownKey(_))));
    }
}
