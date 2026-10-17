//! Account-bound Labs, checked edits, and cancellable job and event readers.
//! Save a job request before sending it; response loss reuses its exact key.
use crate::{CancellationToken, Deployment, Session};
pub use lab_kit::api::{
    AddNote, CreateDocument, CreateJob, EditDocument, Event, List, ToolCall, ToolResult, Upload,
};
pub use lab_kit::{self, *};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    sync::{Arc, RwLock},
    time::Duration,
};

mod artifacts;
#[cfg(feature = "labs-cad")]
pub mod cad;
mod events;
mod wire;
pub mod worker;
pub use artifacts::{ArtifactDownload, ArtifactHandle};
pub use events::Events;

#[derive(Clone)]
pub enum Error {
    Session(crate::Error),
    InvalidInput,
    InvalidResponse,
    TooLarge,
    Cancelled,
    AccountChanged,
    WaitTimedOut,
    ExpiredLink,
    Refused {
        status: u16,
        refusal: Refusal,
        retry_after: Option<Duration>,
    },
}
impl Error {
    pub fn payment_required(&self) -> Option<&crate::PaymentRequired> {
        match self {
            Self::Session(error) => error.payment_required(),
            _ => None,
        }
    }
    pub fn refusal(&self) -> Option<&Refusal> {
        match self {
            Self::Refused { refusal, .. } => Some(refusal),
            _ => None,
        }
    }
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Refused { status, .. } => Some(*status),
            _ => None,
        }
    }
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Refused { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LabsError")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session(e) => e.fmt(f),
            Self::InvalidInput => f.write_str("Check the lab settings and try again."),
            Self::InvalidResponse => f.write_str("The lab response couldn't be opened. Try again."),
            Self::TooLarge => f.write_str("That response is too large to open."),
            Self::Cancelled => f.write_str("Canceled."),
            Self::AccountChanged => f.write_str("The account changed. Try again."),
            Self::WaitTimedOut => f.write_str("The job is still running. Check it again later."),
            Self::ExpiredLink => f.write_str("Refresh this file link and try again."),
            Self::Refused { .. } => f.write_str(
                "That lab request couldn't be completed. Check its details and try again.",
            ),
        }
    }
}
impl std::error::Error for Error {}
impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Self {
        Self::Session(e)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Binding {
    account: String,
    deployment: Deployment,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Request<T> {
    binding: Binding,
    body: T,
}
impl<T> Request<T> {
    pub fn body(&self) -> &T {
        &self.body
    }
}
impl<T> fmt::Debug for Request<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LabRequest").finish_non_exhaustive()
    }
}

macro_rules! reference {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $name {
            binding: Binding,
            space: String,
            id: String,
        }
        impl $name {
            pub fn id(&self) -> &str {
                &self.id
            }
            pub fn space(&self) -> &str {
                &self.space
            }
            pub fn account(&self) -> &str {
                &self.binding.account
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }
    };
}
reference!(DocumentRef);
reference!(JobRef);
reference!(WorkerRef);

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RequestKey(String);
impl RequestKey {
    pub fn generate() -> Result<Self, Error> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| Error::InvalidInput)?;
        Self::try_from(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for RequestKey {
    type Error = Error;
    fn try_from(key: String) -> Result<Self, Error> {
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        {
            return Err(Error::InvalidInput);
        }
        Ok(Self(key))
    }
}
impl From<RequestKey> for String {
    fn from(key: RequestKey) -> Self {
        key.0
    }
}
impl fmt::Debug for RequestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LabRequestKey(..)")
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct JobRequest {
    schema: String,
    request: Request<CreateJob>,
    key: RequestKey,
}
impl JobRequest {
    pub fn key(&self) -> &RequestKey {
        &self.key
    }
    pub fn body(&self) -> &CreateJob {
        self.request.body()
    }
}
impl fmt::Debug for JobRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LabJobRequest").finish_non_exhaustive()
    }
}

pub enum EditResult {
    Created(Version),
    Stale,
    Refused(Refusal),
}
impl fmt::Debug for EditResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Created(_) => "Created(..)",
            Self::Stale => "Stale",
            Self::Refused(_) => "Refused(..)",
        })
    }
}
#[derive(Clone, Debug)]
pub struct PageOptions {
    pub after: u64,
    pub limit: u16,
    pub kind: Option<String>,
}
impl Default for PageOptions {
    fn default() -> Self {
        Self {
            after: 0,
            limit: 50,
            kind: None,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct RetryPolicy {
    pub max_retries: u8,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}
impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct WaitOptions {
    pub timeout: Duration,
    pub poll_interval: Duration,
}
impl Default for WaitOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            poll_interval: Duration::from_millis(500),
        }
    }
}

struct Active {
    session: Option<Session>,
    invalidated: CancellationToken,
}
#[derive(Clone)]
struct Snapshot {
    session: Session,
    invalidated: CancellationToken,
}
impl Snapshot {
    fn binding(&self) -> Binding {
        Binding {
            account: self.session.account().id.clone(),
            deployment: self.session.deployment().clone(),
        }
    }
}
/// Clones share account changes. Call `set_session` or `clear_session` when
/// the application changes accounts; outstanding reads then discard late results.
#[derive(Clone)]
pub struct Client {
    transport: crate::Client,
    active: Arc<RwLock<Active>>,
    retries: RetryPolicy,
}
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LabsClient").finish_non_exhaustive()
    }
}
impl crate::Client {
    pub fn labs(&self, session: &Session) -> Result<Client, Error> {
        self.check_session(session)?;
        Ok(Client {
            transport: self.clone(),
            active: Arc::new(RwLock::new(Active {
                session: Some(session.clone()),
                invalidated: CancellationToken::new(),
            })),
            retries: RetryPolicy::default(),
        })
    }
}
impl Client {
    pub fn set_session(&self, session: &Session) -> Result<(), Error> {
        self.transport.check_session(session)?;
        let mut active = self.active.write().map_err(|_| Error::InvalidResponse)?;
        active.invalidated.cancel();
        *active = Active {
            session: Some(session.clone()),
            invalidated: CancellationToken::new(),
        };
        Ok(())
    }
    pub fn clear_session(&self) {
        if let Ok(mut active) = self.active.write() {
            active.invalidated.cancel();
            active.session = None;
        }
    }
    pub fn with_retries(mut self, retries: RetryPolicy) -> Result<Self, Error> {
        if retries.max_retries > 5
            || retries.initial_delay.is_zero()
            || retries.initial_delay > retries.max_delay
            || retries.max_delay > Duration::from_secs(120)
        {
            return Err(Error::InvalidInput);
        }
        self.retries = retries;
        Ok(self)
    }
    fn snapshot(&self) -> Result<Snapshot, Error> {
        let active = self.active.read().map_err(|_| Error::InvalidResponse)?;
        let session = active.session.as_ref().ok_or(crate::Error::Unauthorized)?;
        self.transport.check_session(session)?;
        Ok(Snapshot {
            session: session.clone(),
            invalidated: active.invalidated.clone(),
        })
    }
    fn bound(&self, binding: &Binding) -> Result<Snapshot, Error> {
        let snapshot = self.snapshot()?;
        if binding.deployment != *self.transport.deployment() {
            return Err(crate::Error::WrongDeployment.into());
        }
        if binding.account != snapshot.session.account().id {
            return Err(crate::Error::WrongAccount.into());
        }
        Ok(snapshot)
    }
    pub fn bind<T>(&self, body: T) -> Result<Request<T>, Error> {
        Ok(Request {
            binding: self.snapshot()?.binding(),
            body,
        })
    }
    pub fn document_ref(&self, space: &str, id: &str) -> Result<DocumentRef, Error> {
        valid_space(space)?;
        valid_id(id)?;
        Ok(DocumentRef {
            binding: self.snapshot()?.binding(),
            space: space.into(),
            id: id.into(),
        })
    }
    pub fn job_ref(&self, space: &str, id: &str) -> Result<JobRef, Error> {
        valid_space(space)?;
        valid_id(id)?;
        Ok(JobRef {
            binding: self.snapshot()?.binding(),
            space: space.into(),
            id: id.into(),
        })
    }
    pub fn job_request(&self, body: CreateJob) -> Result<JobRequest, Error> {
        self.job_request_with_key(body, RequestKey::generate()?)
    }
    pub fn job_request_with_key(
        &self,
        body: CreateJob,
        key: RequestKey,
    ) -> Result<JobRequest, Error> {
        valid_space(&body.space)?;
        valid_id(&body.version.document)?;
        if body.version.version == 0 {
            return Err(Error::InvalidInput);
        }
        Ok(JobRequest {
            schema: "openagents.labs.job-request.v1".into(),
            request: self.bind(body)?,
            key,
        })
    }
}
fn valid_id(id: &str) -> Result<(), Error> {
    if id.is_empty()
        || id.len() > 256
        || matches!(id, "." | "..")
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn valid_space(space: &str) -> Result<(), Error> {
    if space == "me" {
        Ok(())
    } else {
        valid_id(space.strip_prefix("project:").ok_or(Error::InvalidInput)?)
    }
}
