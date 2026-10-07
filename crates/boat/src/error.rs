use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

/// A server error with its HTTP status, parsed envelope, and retry hint.
///
/// The body can contain user data. Debug and display omit it.
pub struct ApiError {
    pub status: reqwest::StatusCode,
    pub envelope: Option<crate::models::ErrorEnvelope>,
    pub retry_after: Option<String>,
    pub request_id: Option<String>,
}

impl ApiError {
    pub fn code(&self) -> Option<&str> {
        self.envelope.as_ref().map(|e| e.code.as_str())
    }

    /// A `502 boat_direct_failed`: the command may already be running on the
    /// sandbox. Never send it again blindly; check `commandStatus` or events.
    pub fn may_be_running(&self) -> bool {
        matches!(
            self.code(),
            Some("boat_direct_failed" | "box_direct_failed")
        )
    }

    /// A 429 or 5xx, which the SDK retries only for reads and keyed creates.
    pub fn is_transient(&self) -> bool {
        self.status == reqwest::StatusCode::TOO_MANY_REQUESTS || self.status.is_server_error()
    }
}

impl fmt::Debug for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiError")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// Errors omit response payloads, credentials, and URLs from formatted output.
#[derive(Debug)]
pub enum Error {
    Configuration(&'static str),
    Transport,
    Encode,
    Decode,
    Api(std::boxed::Box<ApiError>),
    ResponseTooLarge,
    Cancelled,
    Deadline,
    TerminalState,
    DeletionBlocked(std::boxed::Box<crate::models::DeletionOperation>),
    StopIncomplete(std::boxed::Box<crate::models::StopOperation>),
    InvalidCursor,
    StreamLineTooLong,
    Io,
    InvalidSignature,
}

impl Error {
    /// True when a failed command request may have started the command anyway.
    pub fn may_be_running(&self) -> bool {
        matches!(self, Self::Api(api) if api.may_be_running()) || matches!(self, Self::Transport)
    }

    pub(crate) fn encode(_: serde_json::Error) -> Self {
        Self::Encode
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message) => f.write_str(message),
            Self::Api(error) => write!(f, "Boat API returned HTTP {}.", error.status.as_u16()),
            Self::Transport => f.write_str("The Boat request failed during transport."),
            Self::Encode => f.write_str("Cannot encode the Boat request."),
            Self::Decode => f.write_str("Cannot decode the Boat response."),
            Self::ResponseTooLarge => {
                f.write_str("The Boat response exceeds the configured limit.")
            }
            Self::Cancelled => f.write_str("The Boat operation was cancelled."),
            Self::Deadline => f.write_str("The Boat operation reached its deadline."),
            Self::TerminalState => f.write_str("The Boat resource entered a terminal state."),
            Self::DeletionBlocked(_) => {
                f.write_str("The Boat deletion is blocked. Inspect the operation record.")
            }
            Self::StopIncomplete(_) => {
                f.write_str("The Boat stop failed or was superseded. Inspect the operation record.")
            }
            Self::InvalidCursor => f.write_str("The Boat event cursor did not advance."),
            Self::StreamLineTooLong => {
                f.write_str("A Boat command stream line exceeds the configured limit.")
            }
            Self::Io => f.write_str("Cannot write the downloaded Boat data."),
            Self::InvalidSignature => {
                f.write_str("The Boat webhook signature is invalid or expired.")
            }
        }
    }
}

impl std::error::Error for Error {}
