//! The retail service's durable journal: one SQLite file of intents and
//! observations, written before each side effect so a restart finds every
//! funded identity and never repeats an effect it cannot prove did not
//! happen. Money stays in the central ledger; the journal holds only
//! references to it.

use std::path::Path;
use std::sync::Arc;

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
    custody_check: Option<Arc<dyn Fn() -> Result<()> + Send + Sync>>,
}

impl Journal {
    /// Open or create the journal at `path`.
    ///
    /// # Errors
    ///
    /// A SQLite failure.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                return Err(crate::Error::Invalid(
                    "the journal must be a regular private file",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt as _;
                    options.mode(0o600);
                }
                options
                    .open(path)
                    .map_err(|_| crate::Error::Invalid("cannot create the private journal"))?;
            }
            Err(_) => return Err(crate::Error::Invalid("cannot inspect the private journal")),
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| crate::Error::Invalid("cannot restrict the private journal"))?;
        }
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
        connection.pragma_update(None, "secure_delete", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(SCHEMA)?;
        for extra in crate::EXTRA_SCHEMAS {
            connection.execute_batch(extra)?;
        }
        Ok(Self {
            connection,
            custody_check: None,
        })
    }

    /// Inspect an existing journal without creating tables or changing state.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self> {
        let connection =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Self {
            connection,
            custody_check: None,
        })
    }

    /// Install the exclusive service's pathname/descriptor fence. The service
    /// cannot replace or remove it while this journal owns accepted records.
    pub fn install_custody_check(
        &mut self,
        check: impl Fn() -> Result<()> + Send + Sync + 'static,
    ) -> Result<()> {
        if self.custody_check.is_some() {
            return Err(crate::Error::Conflict("journal custody is already bound"));
        }
        check()?;
        self.custody_check = Some(Arc::new(check));
        Ok(())
    }
    pub(crate) fn check_custody(&self) -> Result<()> {
        if let Some(check) = &self.custody_check {
            check()?;
        }
        Ok(())
    }
    pub(crate) fn immediate(&mut self) -> Result<rusqlite::Transaction<'_>> {
        self.check_custody()?;
        let check = self.custody_check.clone();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(check) = check {
            check()?;
        }
        Ok(tx)
    }
}
