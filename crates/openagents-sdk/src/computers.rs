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
mod credentials;
mod files;
mod lifecycle;
mod wire;
pub use commands::{Output, OutputReader};
pub use credentials::{Credential, CredentialRef, CredentialRequest};
pub use files::{File, WriteIntent, WriteReceipt};
pub use lifecycle::WaitFor;

/// A key that lets the service recognize a resent request as the same request.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RequestKey(String);
impl RequestKey {
    /// Creates a new random key.
    pub fn generate() -> Result<Self, Error> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| Error::RandomUnavailable)?;
        Self::try_from(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
    }
    /// Returns the key as text.
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

/// Identifies one cloud computer on one account and deployment.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerRef {
    account: String,
    deployment: Deployment,
    id: String,
}
impl ComputerRef {
    /// Returns the computer's ID.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Returns the ID of the account that owns the computer.
    pub fn account(&self) -> &str {
        &self.account
    }
    /// Returns the deployment the computer belongs to.
    pub fn deployment(&self) -> &Deployment {
        &self.deployment
    }
}
/// Identifies one command on one cloud computer.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRef {
    computer: ComputerRef,
    id: String,
}
impl CommandRef {
    /// Returns the command's ID.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Returns the computer the command runs on.
    pub fn computer(&self) -> &ComputerRef {
        &self.computer
    }
}

/// A saved request to create a cloud computer, safe to resend after a lost response.
#[derive(Clone, Serialize, Deserialize)]
pub struct CreateRequest {
    v: String,
    account: String,
    deployment: Deployment,
    key: RequestKey,
    body: CreateOptions,
}
impl CreateRequest {
    /// Returns the key that identifies this request.
    pub fn key(&self) -> &RequestKey {
        &self.key
    }
}
/// A saved request to run a command, safe to resend after a lost response.
#[derive(Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    v: String,
    computer: ComputerRef,
    key: RequestKey,
    body: CommandOptions,
}
impl CommandRequest {
    /// The exact saved command settings, including credential references.
    pub fn options(&self) -> &CommandOptions {
        &self.body
    }
    /// Returns the key that identifies this request.
    pub fn key(&self) -> &RequestKey {
        &self.key
    }
    /// Returns the computer the command runs on.
    pub fn computer(&self) -> &ComputerRef {
        &self.computer
    }
}

/// A cloud computer as the service last reported it.
#[derive(Clone)]
pub struct Computer {
    reference: ComputerRef,
    data: generated::AppComputer,
}
impl Computer {
    /// Returns the reference to this computer.
    pub fn reference(&self) -> &ComputerRef {
        &self.reference
    }
    /// Returns the computer's details as the service sent them.
    pub fn data(&self) -> &generated::AppComputer {
        &self.data
    }
    /// Returns whether the computer is running and online.
    pub fn awake(&self) -> bool {
        self.data.state == "awake" && self.data.online && self.data.desired == "running"
    }
    /// Returns whether the computer is stopped and offline.
    pub fn stopped(&self) -> bool {
        self.data.state == "stopped" && !self.data.online && self.data.desired == "stopped"
    }
}
/// A command as the service last reported it.
#[derive(Clone)]
pub struct Command {
    reference: CommandRef,
    data: generated::AppCommand,
}
impl Command {
    /// Returns the reference to this command.
    pub fn reference(&self) -> &CommandRef {
        &self.reference
    }
    /// Returns the command's details as the service sent them.
    pub fn data(&self) -> &generated::AppCommand {
        &self.data
    }
    /// Returns how the command ended, or `None` while it still runs.
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
/// How a finished command ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Unknown,
}
/// One page of cloud computers from a list call.
pub struct ComputerPage {
    pub computers: Vec<Computer>,
    pub next_cursor: Option<String>,
}

/// How many times, and how long apart, to retry a failed request.
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
    /// Returns a policy that never retries.
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
    PaymentRequired,
    Missing,
    Quota,
    RateLimited,
    Conflict,
    Unsupported,
    Unavailable,
    Other,
}

/// An error from a cloud computer call.
#[derive(Clone)]
pub enum Error {
    Session(crate::Error),
    InvalidInput,
    InvalidResponse,
    TooLarge,
    Redirect,
    PaymentRequired(crate::PaymentRequired),
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
    /// Returns the kind of refusal, when the service refused the request.
    pub fn refusal(&self) -> Option<Refusal> {
        match self {
            Self::Unsupported => Some(Refusal::Unsupported),
            Self::PaymentRequired(_) => Some(Refusal::PaymentRequired),
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
    /// Returns the HTTP status code, when the service sent one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            Self::PaymentRequired(_) => Some(402),
            Self::Uncertain { cause, .. } => cause.status(),
            _ => None,
        }
    }
    /// Returns the service's error code, when it sent one.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Api { code, .. } => code.as_deref(),
            Self::Uncertain { cause, .. } => cause.code(),
            _ => None,
        }
    }
    /// Returns how long the service asked you to wait before trying again.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api { retry_after, .. } => *retry_after,
            Self::Uncertain { cause, .. } => cause.retry_after(),
            _ => None,
        }
    }
    /// Returns the payment request, when the call needs payment.
    pub fn payment_required(&self) -> Option<&crate::PaymentRequired> {
        match self {
            Self::PaymentRequired(challenge) => Some(challenge),
            Self::Session(error) => error.payment_required(),
            Self::Uncertain { cause, .. } => cause.payment_required(),
            _ => None,
        }
    }
    fn definite_refusal(&self) -> bool {
        matches!(self, Self::PaymentRequired(_))
            || matches!(self, Self::Api { status, .. } if (400..500).contains(status) && *status != 408)
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
            Self::PaymentRequired(_) => "Payment is required. No payment was sent.",
            Self::Cancelled => "Canceled.",
            Self::WaitTimedOut => "The wait ended. Check the computer or command again.",
            Self::Unsupported => "This action isn't available for this computer.",
            Self::RandomUnavailable => "The request couldn't be created. Try again.",
            Self::Network { .. } => "Couldn't connect. Check your connection and try again.",
            Self::Api { status: 401, .. } => "We couldn't connect to your account. Try again.",
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
            crate::Error::PaymentRequired(challenge) => Self::PaymentRequired(challenge),
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
    /// Returns a client for the cloud computers on the session's account.
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
    /// Sets how failed requests are retried.
    pub fn with_retries(mut self, policy: RetryPolicy) -> Result<Self, Error> {
        self.retries = policy.validate()?;
        Ok(self)
    }
    /// Sets how long one call may take, up to 10 minutes.
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
    /// Returns a reference to the computer with this ID on the session's account.
    pub fn reference(&self, id: &str) -> Result<ComputerRef, Error> {
        self.client.check_session(self.session)?;
        cloud_identity(id)?;
        Ok(ComputerRef {
            account: self.session.account().id.clone(),
            deployment: self.client.deployment.clone(),
            id: id.into(),
        })
    }
    /// Returns a reference to the command with this ID on a computer.
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
