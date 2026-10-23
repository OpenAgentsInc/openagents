use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PROTOCOL: &str = "openagents.aethyrforge.host";
pub const VERSION: u16 = 5;
pub const MAX_BYTES: usize = 65_536;
pub const MAX_PENDING: usize = 16;
pub const MAX_RESULTS: usize = 64;
pub const DEADLINE_MS: u64 = 15_000;
/// Initial asset transfer and GPU setup have a separate bounded deadline.
pub const OPEN_DEADLINE_MS: u64 = 120_000;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid(&'static str),
    Peer,
    Version,
    Session,
    Replay,
    Phase,
    Busy,
    Unsupported,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

pub(crate) fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
}
fn text(s: &str, max: usize) -> bool {
    s.len() <= max && !s.chars().any(|c| c == '\0')
}
fn point(v: &[f64; 3]) -> bool {
    v.iter().all(|n| n.is_finite() && n.abs() <= 1_000_000.0)
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub branch: String,
    pub revision: u64,
}
impl Project {
    pub fn validate(&self) -> Result<(), Error> {
        if id(&self.id) && id(&self.branch) && self.revision <= MAX_SAFE_INTEGER {
            Ok(())
        } else {
            Err(Error::Invalid("project"))
        }
    }
    pub fn same_branch(&self, other: &Self) -> bool {
        self.id == other.id && self.branch == other.branch
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CameraMode {
    Walk,
    Fly,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub eye: [f64; 3],
    pub target: [f64; 3],
    pub mode: CameraMode,
}
impl Checkpoint {
    pub fn validate(&self) -> Result<(), Error> {
        if point(&self.eye)
            && point(&self.target)
            && self
                .eye
                .iter()
                .zip(self.target)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                > 0.0001
        {
            Ok(())
        } else {
            Err(Error::Invalid("checkpoint"))
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Inspect,
    Propose,
    Draft,
    Compile,
    Undo,
    Redo,
    Save,
    Publish,
    Share,
    Navigate,
    Capture,
    Observe,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    Shaded,
    Mesh,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SaveState {
    Unsaved,
    Device,
    Project,
    Failed,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Public,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    Universe,
    House,
    PersonalPlanet,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultState {
    Applied,
    Rejected,
    Unsupported,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SelectedObject {
    pub id: String,
    pub asset: String,
    pub position: [f64; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub project: Project,
    pub generation: String,
    pub frame: u64,
    pub save: SaveState,
    pub checkpoint: Option<Checkpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectedObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authoring: Option<crate::AuthoringView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewMode>,
}
impl Status {
    pub fn validate(&self) -> Result<(), Error> {
        self.project.validate()?;
        if !id(&self.generation) || self.frame > MAX_SAFE_INTEGER {
            return Err(Error::Invalid("status"));
        }
        if self
            .selection
            .as_ref()
            .is_some_and(|s| !id(&s.id) || !id(&s.asset) || !point(&s.position))
        {
            return Err(Error::Invalid("selection"));
        }
        if let Some(view) = &self.authoring {
            view.validate()?;
            if view.revision != self.project.revision {
                return Err(Error::Invalid("authoring revision"));
            }
        }
        if let Some(c) = &self.checkpoint {
            c.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Move {
        target: String,
        position: [f64; 3],
    },
    Material {
        target: String,
        parameter: String,
        value: f64,
    },
    Place {
        asset: String,
        position: [f64; 3],
    },
    Remove {
        target: String,
    },
}
impl Edit {
    fn valid(&self) -> bool {
        match self {
            Self::Move { target, position } => id(target) && point(position),
            Self::Material {
                target,
                parameter,
                value,
            } => id(target) && id(parameter) && value.is_finite(),
            Self::Place { asset, position } => id(asset) && point(position),
            Self::Remove { target } => id(target),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Authorize {
        grant: crate::ScopedGrant,
    },
    RevokeAuthorization,
    Open {
        checkpoint: Option<Checkpoint>,
    },
    Inspect,
    ExportProject,
    RestoreProject {
        document: crate::ProjectDocument,
        checkpoint: Option<Checkpoint>,
    },
    View {
        mode: ViewMode,
    },
    Select {
        target: String,
    },
    Propose {
        operations: Vec<Edit>,
    },
    Draft {
        asset: String,
        source: String,
    },
    Compile {
        asset: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_sha256: Option<String>,
    },
    Undo,
    Redo,
    Save,
    Publish {
        visibility: Visibility,
    },
    Share {
        visibility: Visibility,
    },
    Navigate {
        destination: Destination,
    },
    Capture,
    Observe {
        enabled: bool,
    },
    Cancel {
        target: String,
    },
    Lookup {
        target: String,
    },
    Close,
}
impl Action {
    pub fn capability(&self) -> Option<Capability> {
        Some(match self {
            Self::Authorize { .. }
            | Self::RevokeAuthorization
            | Self::Open { .. }
            | Self::Close
            | Self::Cancel { .. }
            | Self::Lookup { .. } => {
                return None;
            }
            Self::Inspect | Self::ExportProject | Self::View { .. } | Self::Select { .. } => {
                Capability::Inspect
            }
            Self::RestoreProject { .. } => Capability::Propose,
            Self::Propose { .. } => Capability::Propose,
            Self::Draft { .. } => Capability::Draft,
            Self::Compile { .. } => Capability::Compile,
            Self::Undo => Capability::Undo,
            Self::Redo => Capability::Redo,
            Self::Save => Capability::Save,
            Self::Publish { .. } => Capability::Publish,
            Self::Share { .. } => Capability::Share,
            Self::Navigate { .. } => Capability::Navigate,
            Self::Capture => Capability::Capture,
            Self::Observe { .. } => Capability::Observe,
        })
    }
    pub fn validate_outcome(&self, outcome: &Outcome) -> Result<(), Error> {
        outcome.validate()?;
        if outcome.state != ResultState::Applied {
            return Ok(());
        }
        let valid = match self {
            Self::Authorize { grant } => grant.validate().is_ok(),
            Self::Open { .. } | Self::Inspect | Self::Observe { .. } => outcome.status.is_some(),
            Self::ExportProject => matches!(outcome.detail, Some(Detail::ProjectDocument { .. })),
            Self::RestoreProject { .. } => outcome.status.is_some(),
            Self::Publish { .. } => matches!(outcome.detail, Some(Detail::Published { .. })),
            Self::Share { .. } => matches!(outcome.detail, Some(Detail::Shared { .. })),
            Self::Capture => matches!(
                outcome.detail,
                Some(Detail::Capture { .. } | Detail::CapturePreview { .. })
            ),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Invalid("result detail"))
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        let valid = match self {
            Self::Authorize { grant } => grant.validate().is_ok(),
            Self::Open {
                checkpoint: Some(c),
            } => c.validate().is_ok(),
            Self::RestoreProject {
                document,
                checkpoint,
            } => {
                document.validate().is_ok()
                    && checkpoint.as_ref().is_none_or(|c| c.validate().is_ok())
            }
            Self::Propose { operations } => {
                !operations.is_empty()
                    && operations.len() <= 8
                    && operations.iter().all(Edit::valid)
            }
            Self::Draft { asset, source } => id(asset) && text(source, 48_000),
            Self::Compile {
                asset,
                source_sha256,
            } => {
                asset.as_ref().is_none_or(|a| id(a))
                    && source_sha256.as_ref().is_none_or(|s| {
                        s.len() == 64
                            && s.bytes()
                                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                    })
            }
            Self::Select { target } | Self::Cancel { target } | Self::Lookup { target } => {
                id(target)
            }
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Invalid("action"))
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Detail {
    ProjectDocument {
        document: crate::ProjectDocument,
    },
    CapturePreview {
        png: String,
        sha256: String,
        width: u32,
        height: u32,
    },
    Diagnostics {
        items: Vec<Diagnostic>,
    },
    Published {
        release: String,
        url: String,
    },
    Shared {
        url: String,
    },
    Capture {
        url: String,
        sha256: String,
        width: u32,
        height: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub asset: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub state: ResultState,
    pub message: String,
    pub status: Option<Status>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<Detail>,
}
impl Outcome {
    pub fn validate(&self) -> Result<(), Error> {
        if !text(&self.message, 2048) {
            return Err(Error::Invalid("message"));
        }
        if let Some(s) = &self.status {
            s.validate()?;
        }
        if let Some(detail) = &self.detail {
            let valid = match detail {
                Detail::ProjectDocument { document } => document.validate().is_ok(),
                Detail::CapturePreview {
                    png,
                    sha256,
                    width,
                    height,
                } => {
                    use base64::Engine;
                    use sha2::Digest;
                    base64::engine::general_purpose::STANDARD
                        .decode(png)
                        .is_ok_and(|bytes| {
                            bytes.len() <= 24_576
                                && bytes.len() >= 24
                                && bytes[..8] == *b"\x89PNG\r\n\x1a\n"
                                && u32::from_be_bytes(bytes[16..20].try_into().unwrap()) == *width
                                && u32::from_be_bytes(bytes[20..24].try_into().unwrap()) == *height
                                && *width > 0
                                && *height > 0
                                && *width <= 320
                                && *height <= 320
                                && format!("{:x}", sha2::Sha256::digest(&bytes)) == *sha256
                        })
                }
                Detail::Diagnostics { items } => {
                    items.len() <= 32
                        && items.iter().all(|d| {
                            id(&d.code)
                                && d.asset.as_ref().is_none_or(|a| id(a))
                                && text(&d.message, 2048)
                        })
                }
                Detail::Published { release, url } => id(release) && endpoint(url).is_ok(),
                Detail::Shared { url } => endpoint(url).is_ok(),
                Detail::Capture {
                    url,
                    sha256,
                    width,
                    height,
                } => {
                    endpoint(url).is_ok()
                        && sha256.len() == 64
                        && sha256
                            .bytes()
                            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                        && *width > 0
                        && *height > 0
                        && *width <= 8192
                        && *height <= 8192
                }
            };
            if !valid {
                return Err(Error::Invalid("detail"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    Hello {
        versions: Vec<u16>,
    },
    Ready {
        release: String,
        engine_abi: String,
        project_format: u16,
        capabilities: Vec<Capability>,
    },
    Request {
        request_id: String,
        project: Project,
        action: Action,
    },
    Reply {
        request_id: String,
        outcome: Outcome,
    },
    Status {
        status: Status,
    },
    AuthRequired {
        request_id: String,
        message: String,
    },
    Revoked {
        message: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub protocol: String,
    pub version: u16,
    pub session: String,
    pub sequence: u64,
    pub message: Message,
}
impl Envelope {
    pub fn validate(&self) -> Result<(), Error> {
        if self.protocol != PROTOCOL || self.version != VERSION {
            return Err(Error::Version);
        }
        if !id(&self.session) || self.sequence == 0 || self.sequence > MAX_SAFE_INTEGER {
            return Err(Error::Invalid("envelope"));
        }
        match &self.message {
            Message::Hello { versions } => {
                if versions.is_empty() || versions.len() > 8 || !versions.contains(&VERSION) {
                    return Err(Error::Version);
                }
            }
            Message::Ready {
                release,
                engine_abi,
                project_format,
                capabilities,
            } => {
                if !id(release)
                    || !id(engine_abi)
                    || *project_format == 0
                    || capabilities.len() > 32
                {
                    return Err(Error::Invalid("ready"));
                }
                for (i, c) in capabilities.iter().enumerate() {
                    if capabilities[..i].contains(c) {
                        return Err(Error::Invalid("capabilities"));
                    }
                }
            }
            Message::Request {
                request_id,
                project,
                action,
            } => {
                if !id(request_id) {
                    return Err(Error::Invalid("request"));
                }
                project.validate()?;
                action.validate()?;
            }
            Message::Reply {
                request_id,
                outcome,
            } => {
                if !id(request_id) {
                    return Err(Error::Invalid("reply"));
                }
                outcome.validate()?;
            }
            Message::Status { status } => status.validate()?,
            Message::AuthRequired {
                request_id,
                message,
            } => {
                if !id(request_id) || !text(message, 2048) {
                    return Err(Error::Invalid("auth"));
                }
            }
            Message::Revoked { message } => {
                if !text(message, 2048) {
                    return Err(Error::Invalid("revocation"));
                }
            }
        }
        Ok(())
    }
    pub fn decode(text: &str) -> Result<Self, Error> {
        if text.len() > MAX_BYTES {
            return Err(Error::Invalid("bytes"));
        }
        let value: Self = serde_json::from_str(text).map_err(|_| Error::Invalid("json"))?;
        value.validate()?;
        Ok(value)
    }
    pub fn encode(&self) -> Result<String, Error> {
        self.validate()?;
        let s = serde_json::to_string(self).map_err(|_| Error::Invalid("json"))?;
        if s.len() > MAX_BYTES {
            return Err(Error::Invalid("bytes"));
        }
        Ok(s)
    }
}

/// Only HTTPS or loopback HTTP endpoints are eligible. Credentials are never URLs.
pub fn endpoint(input: &str) -> Result<url::Url, Error> {
    let u = url::Url::parse(input).map_err(|_| Error::Invalid("url"))?;
    let loopback = u
        .host_str()
        .is_some_and(|h| h == "localhost" || h == "127.0.0.1" || h == "[::1]");
    if !(u.scheme() == "https" || (u.scheme() == "http" && loopback))
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || u.query().is_some()
    {
        return Err(Error::Invalid("url"));
    }
    Ok(u)
}
