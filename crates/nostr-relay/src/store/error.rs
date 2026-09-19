use std::fmt;

use crate::domain::DomainError;

/// A storage, migration, or row-decoding failure.
#[derive(Debug)]
pub enum StoreError {
    Database(tokio_postgres::Error),
    Domain(DomainError),
    ConnectionClosed,
    WorkQueueFull,
    MigrationDrift(String),
    InvalidPolicy(String),
    Management(String),
    Media(String),
    QueryCancelled,
    EphemeralTooLarge(usize),
    TimestampOutOfRange { field: &'static str, value: u64 },
    InvalidLimit(usize),
    Serialization(String),
    CorruptRow(String),
    LegacyImport(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "Postgres error: {error}"),
            Self::Domain(error) => write!(f, "invalid event: {error}"),
            Self::ConnectionClosed => f.write_str("Postgres connection driver is not current"),
            Self::WorkQueueFull => f.write_str("database work queue is full"),
            Self::MigrationDrift(reason) => write!(f, "schema migration drift: {reason}"),
            Self::InvalidPolicy(reason) => write!(f, "invalid relay admission policy: {reason}"),
            Self::Management(reason) => write!(f, "management request failed: {reason}"),
            Self::Media(reason) => write!(f, "media request failed: {reason}"),
            Self::QueryCancelled => f.write_str("database query was cancelled"),
            Self::EphemeralTooLarge(bytes) => {
                write!(f, "ephemeral event is {bytes} bytes; maximum is 1048576")
            }
            Self::TimestampOutOfRange { field, value } => {
                write!(f, "{field} timestamp {value} exceeds Postgres bigint range")
            }
            Self::InvalidLimit(value) => {
                write!(f, "query limit {value} exceeds Postgres bigint range")
            }
            Self::Serialization(reason) => write!(f, "event serialization failed: {reason}"),
            Self::CorruptRow(reason) => write!(f, "stored event is corrupt: {reason}"),
            Self::LegacyImport(reason) => write!(f, "legacy relay import failed: {reason}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Domain(error) => Some(error),
            _ => None,
        }
    }
}

impl From<tokio_postgres::Error> for StoreError {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Database(error)
    }
}

impl From<DomainError> for StoreError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}
