//! Errors preserve whether a payment might have been dispatched.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("the wallet identity changed")]
    NodeMismatch,
    #[error("no payment route is available")]
    NoRoute,
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("incorrect payment details")]
    IncorrectPaymentDetails,
    #[error("payment already completed")]
    AlreadyPaid,
    #[error("payment is still pending")]
    InFlight,
    #[error("the request timed out; check the payment before trying again")]
    Timeout,
    #[error("access denied")]
    PermissionDenied,
    #[error("the node is unavailable")]
    Unavailable,
    #[error("the node release does not match the configured release")]
    VersionMismatch,
    #[error("the node is still syncing")]
    NotSynced,
    #[error("the node has no record of this payment")]
    NotFound,
    #[error("invalid {0}")]
    Invalid(&'static str),
    #[error("the node refused the request")]
    Refused,
    #[error("the certificate does not match the configured pin")]
    CertificatePin,
    #[error("could not save the invoice position")]
    Checkpoint,
}
impl From<tonic::Status> for Error {
    fn from(status: tonic::Status) -> Self {
        use tonic::Code;
        match status.code() {
            // LND 0.21 forwards bakery's pre-dispatch denial as Unknown.
            // Match only its fixed literal; never retain the server message.
            Code::Unknown if status.message() == "permission denied" => Self::PermissionDenied,
            Code::NotFound => Self::NotFound,
            Code::AlreadyExists => Self::AlreadyPaid,
            Code::PermissionDenied | Code::Unauthenticated => Self::PermissionDenied,
            Code::DeadlineExceeded | Code::Cancelled => Self::Timeout,
            Code::Unavailable | Code::Unknown => Self::Unavailable,
            Code::InvalidArgument | Code::OutOfRange => Self::Invalid("RPC argument"),
            Code::FailedPrecondition => Self::NotSynced,
            _ => Self::Refused,
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn signed(value: u64, name: &'static str) -> Result<i64> {
    i64::try_from(value).map_err(|_| Error::Invalid(name))
}
pub(crate) fn unsigned(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|_| Error::Invalid("negative amount"))
}
