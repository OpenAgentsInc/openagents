//! The retail service's durable journal: one SQLite file of intents and
//! observations, written before each side effect so a restart finds every
//! funded identity and never repeats an effect it cannot prove did not
//! happen. Money stays in the central ledger; the journal holds only
//! references to it.

use std::path::Path;

use rusqlite::{Connection, TransactionBehavior};

use crate::Result;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS funded (
    offer TEXT PRIMARY KEY,
    request TEXT NOT NULL UNIQUE,
    execution TEXT NOT NULL UNIQUE,
    account TEXT NOT NULL,
    offer_digest TEXT NOT NULL,
    admission TEXT NOT NULL,
    quote TEXT NOT NULL,
    task TEXT NOT NULL,
    confirmed_at INTEGER NOT NULL
);
";

/// The journal. Mutations take `&mut self`; one writer per file at a time,
/// serialized by SQLite's immediate transactions.
pub struct Journal {
    pub(crate) connection: Connection,
}

impl Journal {
    /// Open or create the journal at `path`.
    ///
    /// # Errors
    ///
    /// A SQLite failure.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::initialize(Connection::open(path)?)
    }

    /// A journal that lives only in memory, for tests.
    ///
    /// # Errors
    ///
    /// A SQLite failure.
    pub fn in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(connection: Connection) -> Result<Self> {
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(SCHEMA)?;
        for extra in crate::EXTRA_SCHEMAS {
            connection.execute_batch(extra)?;
        }
        Ok(Self { connection })
    }

    pub(crate) fn immediate(&mut self) -> Result<rusqlite::Transaction<'_>> {
        Ok(self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?)
    }
}
