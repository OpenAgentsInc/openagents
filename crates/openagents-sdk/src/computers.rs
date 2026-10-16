//! Account-bound cloud computers over the generated public contract.
//! Save mutation requests before sending them; reuse the same request after
//! response loss. Local cancellation never stops a remote computer or command.
use crate::generated::computers as generated;
pub use crate::generated::computers::{
    AppComputerCapabilities as Capabilities, AppCreateCommand as CommandOptions,
    AppCreateComputer as CreateOptions,
};
use crate::{CancellationToken, Client, Deployment, Session};
use serde::{Deserialize, Serialize};
use std::{fmt, time::Duration};

macro_rules! opaque_debug {
    ($($name:ident),+ $(,)?) => {$(impl fmt::Debug for $name {
        fn fmt(&self, f:&mut fmt::Formatter<'_>)->fmt::Result {
            f.debug_struct(stringify!($name)).finish_non_exhaustive()
        }
    })+};
}
mod commands;
mod files;
mod lifecycle;
mod wire;
pub use commands::{Output, OutputReader};
pub use files::{File, WriteIntent, WriteReceipt};
pub use lifecycle::WaitFor;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RequestKey(String);
impl RequestKey {
    pub fn generate() -> Result<Self, Error> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| Error::RandomUnavailable)?;
        Self::try_from(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for RequestKey {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        if value.is_empty()
            || value.len() > 128
            || value.starts_with('.')
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
        {
            return Err(Error::InvalidInput);
        }
        Ok(Self(value))
    }
}
impl From<RequestKey> for String {
    fn from(key: RequestKey) -> Self {
        key.0
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerRef {
    account: String,
    deployment: Deployment,
    id: String,
}
impl ComputerRef {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn account(&self) -> &str {
        &self.account
    }
    pub fn deployment(&self) -> &Deployment {
        &self.deployment
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRef {
    computer: ComputerRef,
    id: String,
}
impl CommandRef {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn computer(&self) -> &ComputerRef {
        &self.computer
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CreateRequest {
    v: String,
    account: String,
    deployment: Deployment,
    key: RequestKey,
    body: CreateOptions,
}
impl CreateRequest {
    pub fn key(&self) -> &RequestKey {
        &self.key
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    v: String,
    computer: ComputerRef,
    key: RequestKey,
    body: CommandOptions,
}
impl CommandRequest {
    pub fn key(&self) -> &RequestKey {
        &self.key
    }
    pub fn computer(&self) -> &ComputerRef {
        &self.computer
    }
}

#[derive(Clone)]
pub struct Computer {
    reference: ComputerRef,
    data: generated::AppComputer,
}
impl Computer {
    pub fn reference(&self) -> &ComputerRef {
        &self.reference
    }
    pub fn data(&self) -> &generated::AppComputer {
        &self.data
    }
    pub fn awake(&self) -> bool {
        self.data.state == "awake" && self.data.online && self.data.desired == "running"
    }
    pub fn stopped(&self) -> bool {
        self.data.state == "stopped" && !self.data.online && self.data.desired == "stopped"
    }
}
#[derive(Clone)]
pub struct Command {
    reference: CommandRef,
    data: generated::AppCommand,
}
impl Command {
    pub fn reference(&self) -> &CommandRef {
        &self.reference
    }
    pub fn data(&self) -> &generated::AppCommand {
        &self.data
    }
    pub fn outcome(&self) -> Option<CommandOutcome> {
        match self.data.state.as_str() {
            "succeeded" if self.data.exit_code == Some(0) => Some(CommandOutcome::Succeeded),
            "failed" => Some(CommandOutcome::Failed),
            "cancelled" => Some(CommandOutcome::Cancelled),
            "timed_out" => Some(CommandOutcome::TimedOut),
            "unknown" => Some(CommandOutcome::Unknown),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Unknown,
}
pub struct ComputerPage {
    pub computers: Vec<Computer>,
    pub next_cursor: Option<String>,
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
impl RetryPolicy {
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }
    fn validate(self) -> Result<Self, Error> {
        if self.max_retries > 5
            || self.initial_delay.is_zero()
            || self.initial_delay > self.max_delay
            || self.max_delay > Duration::from_secs(120)
        {
            return Err(Error::InvalidInput);
        }
        Ok(self)
    }
}

/// A stable refusal category; `Error::code` preserves future server codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Authentication,
    AccessDenied,
    Missing,
    Quota,
    RateLimited,
    Conflict,
    Unsupported,
    Unavailable,
    Other,
}

#[derive(Clone)]
pub enum Error {
    Session(crate::Error),
    InvalidInput,
    InvalidResponse,
    TooLarge,
    Redirect,
    Cancelled,
    WaitTimedOut,
    Unsupported,
    RandomUnavailable,
    Network {
        timeout: bool,
    },
    Api {
        status: u16,
        code: Option<String>,
        retry_after: Option<Duration>,
    },
    /// Preserve the original request or inspect the same resource. Never
    /// create a new key or command to resolve this error.
    Uncertain {
        operation: &'static str,
        key: Option<String>,
        cause: Box<Error>,
    },
}
impl Error {
    pub fn refusal(&self) -> Option<Refusal> {
        match self {
            Self::Unsupported => Some(Refusal::Unsupported),
            Self::Uncertain { cause, .. } => cause.refusal(),
            Self::Api { status, code, .. } => Some(match (*status, code.as_deref()) {
                (_, Some("quota_exhausted")) => Refusal::Quota,
                (_, Some("unsupported_profile" | "unsupported_capability")) => Refusal::Unsupported,
                (401, _) => Refusal::Authentication,
                (403, _) => Refusal::AccessDenied,
                (404, _) => Refusal::Missing,
                (409, _) => Refusal::Conflict,
                (429, _) => Refusal::RateLimited,
                (500..=599, _) => Refusal::Unavailable,
                _ => Refusal::Other,
            }),
            _ => None,
        }
    }
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            Self::Uncertain { cause, .. } => cause.status(),
            _ => None,
        }
    }
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Api { code, .. } => code.as_deref(),
            Self::Uncertain { cause, .. } => cause.code(),
            _ => None,
        }
    }
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api { retry_after, .. } => *retry_after,
            Self::Uncertain { cause, .. } => cause.retry_after(),
            _ => None,
        }
    }
    fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Network { .. }
                | Self::Api {
                    status: 408 | 429 | 500 | 502 | 503 | 504,
                    ..
                }
        )
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::Session(e) = self {
            return fmt::Display::fmt(e, f);
        }
        f.write_str(match self {
            Self::InvalidInput => "Check the request and try again.",
            Self::InvalidResponse => "That response couldn't be opened.",
            Self::TooLarge => "The file or output is too large.",
            Self::Redirect => "Check the website address and try again.",
            Self::Cancelled => "Canceled.",
            Self::WaitTimedOut => "The wait ended. Check the computer or command again.",
            Self::Unsupported => "This action isn't available for this computer.",
            Self::RandomUnavailable => "The request couldn't be created. Try again.",
            Self::Network { .. } => "Couldn't connect. Check your connection and try again.",
            Self::Api { status: 401, .. } => "Log in again to continue.",
            Self::Api { status: 403, .. } => "You don't have access to this computer.",
            Self::Api { status: 404, .. } => {
                "This computer, command, or file is no longer available."
            }
            Self::Api { status: 429, .. } => "Too many requests. Try again later.",
            Self::Api { .. } => "The service could not complete this request.",
            Self::Uncertain { .. } => {
                "The request may have completed. Check it before starting another."
            }
            Self::Session(_) => unreachable!(),
        })
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComputersError")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}
impl std::error::Error for Error {}
impl From<crate::Error> for Error {
    fn from(value: crate::Error) -> Self {
        match value {
            crate::Error::TooLarge => Self::TooLarge,
            crate::Error::Network { timeout } => Self::Network { timeout },
            e => Self::Session(e),
        }
    }
}

/// A borrowed client bound to one authenticated account. Auth and history
/// retain their existing retry behavior; only eligible Computers operations retry.
pub struct Computers<'a> {
    client: &'a Client,
    session: &'a Session,
    retries: RetryPolicy,
    operation_timeout: Duration,
}
impl fmt::Debug for Computers<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Computers").finish_non_exhaustive()
    }
}
impl Client {
    pub fn computers<'a>(&'a self, session: &'a Session) -> Result<Computers<'a>, Error> {
        self.check_session(session)?;
        Ok(Computers {
            client: self,
            session,
            retries: RetryPolicy::default(),
            operation_timeout: Duration::from_secs(60),
        })
    }
}
impl Computers<'_> {
    pub fn with_retries(mut self, policy: RetryPolicy) -> Result<Self, Error> {
        self.retries = policy.validate()?;
        Ok(self)
    }
    pub fn with_operation_timeout(mut self, timeout: Duration) -> Result<Self, Error> {
        if timeout.is_zero() || timeout > Duration::from_secs(600) {
            return Err(Error::InvalidInput);
        }
        self.operation_timeout = timeout;
        Ok(self)
    }
    fn check(&self, reference: &ComputerRef) -> Result<(), Error> {
        self.client.check_session(self.session)?;
        if reference.account != self.session.account().id {
            return Err(crate::Error::WrongAccount.into());
        }
        if reference.deployment != self.client.deployment {
            return Err(crate::Error::WrongDeployment.into());
        }
        cloud_identity(&reference.id)
    }
    pub fn reference(&self, id: &str) -> Result<ComputerRef, Error> {
        self.client.check_session(self.session)?;
        cloud_identity(id)?;
        Ok(ComputerRef {
            account: self.session.account().id.clone(),
            deployment: self.client.deployment.clone(),
            id: id.into(),
        })
    }
    pub fn command_reference(&self, computer: &ComputerRef, id: &str) -> Result<CommandRef, Error> {
        self.check(computer)?;
        identity(id)?;
        Ok(CommandRef {
            computer: computer.clone(),
            id: id.into(),
        })
    }
    fn computer(
        &self,
        data: generated::AppComputer,
        expected: Option<&str>,
    ) -> Result<Computer, Error> {
        cloud_identity(&data.id).map_err(|_| Error::InvalidResponse)?;
        if data.kind != "cloud" || expected.is_some_and(|id| id != data.id) {
            return Err(Error::InvalidResponse);
        }
        Ok(Computer {
            reference: self.reference(&data.id)?,
            data,
        })
    }
    fn command(
        &self,
        data: generated::AppCommand,
        computer: &ComputerRef,
        expected: Option<&str>,
    ) -> Result<Command, Error> {
        self.check(computer)?;
        identity(&data.id)?;
        if data.computer != computer.id || expected.is_some_and(|id| id != data.id) {
            return Err(Error::InvalidResponse);
        }
        Ok(Command {
            reference: CommandRef {
                computer: computer.clone(),
                id: data.id.clone(),
            },
            data,
        })
    }
}
fn cloud_identity(value: &str) -> Result<(), Error> {
    if value
        .strip_prefix("cmp_")
        .is_some_and(|suffix| suffix.len() == 20 && suffix.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        Ok(())
    } else {
        Err(Error::InvalidInput)
    }
}
fn identity(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 256
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn request_key(value: &Option<String>) -> Result<RequestKey, Error> {
    match value {
        Some(value) => RequestKey::try_from(value.clone()),
        None => RequestKey::generate(),
    }
}
opaque_debug!(
    RequestKey,
    ComputerRef,
    CommandRef,
    CreateRequest,
    CommandRequest,
    Computer,
    Command,
    ComputerPage
);
