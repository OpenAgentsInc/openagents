//! The Postgres backend for the account service's stores (#11154).
//!
//! The file stores (`accounts.json`, `sessions.json`, `keys.json`, and
//! the GitHub-access and provider-key files beside them) keep working
//! unchanged for local development and tests. A process that wants them
//! in Postgres opens a [`Database`] and [`attach`]es it to the registry
//! directory the stores are opened with; from then on every load, save
//! and lock for that directory goes to the database instead of the
//! files. Nothing else about the stores changes: the same documents, the
//! same validation, the same digest chain.
//!
//! A document is kept as rows ([`docs`]): each collection in it (the
//! accounts, the workspaces, each workspace's memberships, the sessions)
//! is a table of `(key..., record jsonb)`, and what has no table yet sits
//! in `identity.stores.rest`. A writer takes a transaction and a Postgres
//! advisory lock per store, so two writers in any number of processes
//! serialize the way the lock files made them, and a reader sees one
//! snapshot. The schema is `docs/data/schema.md`; the migrations are
//! embedded and applied by [`Database::connect`] under an advisory lock.
//!
//! The stores' API is synchronous and is called from async handlers, so
//! the database runs on a small runtime of its own and a call made from
//! inside another runtime hops to a scoped thread rather than nesting
//! runtimes.

pub mod docs;
pub mod import;
pub mod records;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use serde_json::Value;
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, NoTls, Row};

/// The embedded migrations, in order. Each runs once, in its own
/// transaction, recorded in `public.schema_migrations`.
pub const MIGRATIONS: &[(i32, &str, &str)] = &[(
    1,
    "identity_workspace",
    include_str!("../migrations/0001_identity_workspace.sql"),
)];

/// The advisory lock key migrations run under.
const MIGRATION_LOCK: i64 = 0x6f61_6d69_6772_0001;

/// How many idle connections the pool keeps.
const POOL_IDLE: usize = 8;

/// A database fault, with the text Postgres or the driver gave.
#[derive(Debug, Clone)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "account database: {}", self.0)
    }
}

impl std::error::Error for Error {}

impl From<tokio_postgres::Error> for Error {
    fn from(error: tokio_postgres::Error) -> Self {
        // `as_db_error` carries the server's message; the plain Display
        // is often just "db error".
        match error.as_db_error() {
            Some(db) => Self(format!("{}: {}", db.code().code(), db.message())),
            None => Self(error.to_string()),
        }
    }
}

impl From<Error> for std::io::Error {
    fn from(error: Error) -> Self {
        std::io::Error::other(error.to_string())
    }
}

struct Inner {
    runtime: tokio::runtime::Runtime,
    config: tokio_postgres::Config,
    idle: Mutex<Vec<Client>>,
    /// Store name to the last document read and the revision it was at.
    cache: Mutex<HashMap<String, (i64, Arc<Value>)>>,
}

/// A connection pool to the account database.
#[derive(Clone)]
pub struct Database {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

impl Database {
    /// Connect with a libpq-style connection string (`host=/cloudsql/P:R:I
    /// user=… dbname=… password=…`, or a `postgres://` URL) and apply any
    /// migration not yet applied.
    ///
    /// # Errors
    ///
    /// The string does not parse, the server cannot be reached, or a
    /// migration fails.
    pub fn connect(dsn: &str) -> Result<Self, Error> {
        let database = Self::open(dsn)?;
        database.migrate()?;
        Ok(database)
    }

    /// For tests: create a fresh database on the server `admin_dsn`
    /// reaches (a libpq key=value string), migrate it, and connect.
    ///
    /// # Errors
    ///
    /// The server refuses.
    pub fn scratch(admin_dsn: &str) -> Result<Self, Error> {
        let admin = Self::open(admin_dsn)?;
        let mut bytes = [0_u8; 8];
        getrandom::fill(&mut bytes).map_err(|e| Error(e.to_string()))?;
        let name = format!(
            "scratch_{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        admin.execute(&format!("CREATE DATABASE {name}"), &[])?;
        Self::connect(&format!("{admin_dsn} dbname={name}"))
    }

    /// Connect without migrating.
    ///
    /// # Errors
    ///
    /// The string does not parse or the server cannot be reached.
    pub fn open(dsn: &str) -> Result<Self, Error> {
        let mut config: tokio_postgres::Config = dsn
            .parse()
            .map_err(|e: tokio_postgres::Error| Error(format!("connection string: {e}")))?;
        if config.get_application_name().is_none() {
            config.application_name("openagents-tenancy");
        }
        config.connect_timeout(std::time::Duration::from_secs(10));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("tenancy-db")
            .enable_all()
            .build()
            .map_err(|e| Error(format!("runtime: {e}")))?;
        let database = Self {
            inner: Arc::new(Inner {
                runtime,
                config,
                idle: Mutex::new(Vec::new()),
                cache: Mutex::new(HashMap::new()),
            }),
        };
        let client = database.client()?;
        database.give_back(client);
        Ok(database)
    }

    /// Run a future on the database's runtime and wait for it, from sync
    /// code that may itself be running inside another runtime.
    fn block<F>(&self, future: F) -> F::Output
    where
        F: std::future::Future + Send,
        F::Output: Send,
    {
        if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::scope(|scope| {
                scope
                    .spawn(|| self.inner.runtime.block_on(future))
                    .join()
                    .expect("the database thread does not panic")
            })
        } else {
            self.inner.runtime.block_on(future)
        }
    }

    fn client(&self) -> Result<Client, Error> {
        loop {
            let pooled = self.inner.idle.lock().ok().and_then(|mut idle| idle.pop());
            match pooled {
                Some(client) if !client.is_closed() => return Ok(client),
                Some(_) => {}
                None => break,
            }
        }
        let config = self.inner.config.clone();
        let handle = self.inner.runtime.handle().clone();
        self.block(async move {
            let (client, connection) = config.connect(NoTls).await?;
            handle.spawn(async move {
                let _ = connection.await;
            });
            Ok(client)
        })
    }

    fn give_back(&self, client: Client) {
        if client.is_closed() {
            return;
        }
        if let Ok(mut idle) = self.inner.idle.lock()
            && idle.len() < POOL_IDLE
        {
            idle.push(client);
        }
    }

    /// One statement outside a transaction.
    ///
    /// # Errors
    ///
    /// The statement fails.
    pub fn query(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, Error> {
        let client = self.client()?;
        let rows = self.block(async { client.query(sql, params).await });
        self.give_back(client);
        Ok(rows?)
    }

    /// One statement outside a transaction; the rows it changed.
    ///
    /// # Errors
    ///
    /// The statement fails.
    pub fn execute(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, Error> {
        let client = self.client()?;
        let count = self.block(async { client.execute(sql, params).await });
        self.give_back(client);
        Ok(count?)
    }

    /// Begin a transaction. It rolls back when dropped uncommitted.
    ///
    /// # Errors
    ///
    /// No connection, or `BEGIN` fails.
    pub fn transaction(&self) -> Result<Tx, Error> {
        let client = self.client()?;
        self.block(async { client.batch_execute("BEGIN").await })?;
        Ok(Tx {
            database: self.clone(),
            client: Some(client),
            failed: false,
        })
    }

    /// Apply every embedded migration not yet applied, under an advisory
    /// lock so two starting processes do not race.
    ///
    /// # Errors
    ///
    /// A migration fails; nothing of that migration is applied.
    pub fn migrate(&self) -> Result<Vec<i32>, Error> {
        let client = self.client()?;
        let applied = self.block(async {
            client
                .execute("SELECT pg_advisory_lock($1)", &[&MIGRATION_LOCK])
                .await?;
            let result = async {
                client
                    .batch_execute(
                        "CREATE TABLE IF NOT EXISTS public.schema_migrations (
                            version integer PRIMARY KEY,
                            name text NOT NULL,
                            applied_at timestamptz NOT NULL DEFAULT now())",
                    )
                    .await?;
                let mut applied = Vec::new();
                for (version, name, sql) in MIGRATIONS {
                    let done = client
                        .query_opt(
                            "SELECT 1 FROM public.schema_migrations WHERE version = $1",
                            &[version],
                        )
                        .await?
                        .is_some();
                    if done {
                        continue;
                    }
                    client.batch_execute("BEGIN").await?;
                    let step = async {
                        client.batch_execute(sql).await?;
                        client
                            .execute(
                                "INSERT INTO public.schema_migrations (version, name) VALUES ($1, $2)",
                                &[version, name],
                            )
                            .await
                    }
                    .await;
                    match step {
                        Ok(_) => client.batch_execute("COMMIT").await?,
                        Err(error) => {
                            let _ = client.batch_execute("ROLLBACK").await;
                            return Err(error);
                        }
                    }
                    applied.push(*version);
                }
                Ok::<_, tokio_postgres::Error>(applied)
            }
            .await;
            let _ = client
                .execute("SELECT pg_advisory_unlock($1)", &[&MIGRATION_LOCK])
                .await;
            result
        });
        self.give_back(client);
        Ok(applied?)
    }

    pub(crate) fn cached(&self, store: &str, revision: i64) -> Option<Arc<Value>> {
        let cache = self.inner.cache.lock().ok()?;
        cache
            .get(store)
            .filter(|(at, _)| *at == revision)
            .map(|(_, doc)| doc.clone())
    }

    pub(crate) fn remember(&self, store: &str, revision: i64, doc: Arc<Value>) {
        if let Ok(mut cache) = self.inner.cache.lock() {
            cache.insert(store.to_string(), (revision, doc));
        }
    }

    pub(crate) fn forget(&self, store: &str) {
        if let Ok(mut cache) = self.inner.cache.lock() {
            cache.remove(store);
        }
    }
}

/// A transaction on one pooled connection. Commit it with
/// [`Tx::commit`]; dropping it rolls back.
pub struct Tx {
    database: Database,
    client: Option<Client>,
    failed: bool,
}

impl Tx {
    fn client(&self) -> &Client {
        self.client
            .as_ref()
            .expect("an open transaction has a client")
    }

    /// One statement in the transaction.
    ///
    /// # Errors
    ///
    /// The statement fails; the transaction is then only good for
    /// rolling back.
    pub fn query(&mut self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, Error> {
        let client = self.client();
        let rows = self
            .database
            .block(async { client.query(sql, params).await });
        if rows.is_err() {
            self.failed = true;
        }
        Ok(rows?)
    }

    /// One statement in the transaction; the rows it changed.
    ///
    /// # Errors
    ///
    /// The statement fails.
    pub fn execute(&mut self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, Error> {
        let client = self.client();
        let count = self
            .database
            .block(async { client.execute(sql, params).await });
        if count.is_err() {
            self.failed = true;
        }
        Ok(count?)
    }

    /// Take a transaction-scoped advisory lock on `key`, waiting up to
    /// `lock_timeout` (ten seconds) for another holder.
    ///
    /// # Errors
    ///
    /// The lock was not granted in time.
    pub fn lock(&mut self, key: &str) -> Result<(), Error> {
        self.execute("SET LOCAL lock_timeout = '10s'", &[])?;
        self.execute(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            &[&key],
        )?;
        Ok(())
    }

    /// Mark the transaction as one that must not commit.
    pub fn poison(&mut self) {
        self.failed = true;
    }

    /// Whether a statement failed or the transaction was poisoned.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// The database the transaction runs on.
    #[must_use]
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Commit, or roll back and report when a statement failed.
    ///
    /// # Errors
    ///
    /// A statement failed earlier, or `COMMIT` fails.
    pub fn commit(mut self) -> Result<(), Error> {
        let client = self
            .client
            .take()
            .expect("an open transaction has a client");
        if self.failed {
            let _ = self
                .database
                .block(async { client.batch_execute("ROLLBACK").await });
            self.database.give_back(client);
            return Err(Error("the transaction failed and was rolled back".into()));
        }
        let done = self
            .database
            .block(async { client.batch_execute("COMMIT").await });
        self.database.give_back(client);
        Ok(done?)
    }
}

impl Drop for Tx {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let rolled = self
                .database
                .block(async { client.batch_execute("ROLLBACK").await });
            if rolled.is_ok() {
                self.database.give_back(client);
            }
        }
    }
}

// --- Which registry directories are kept in which database ---

fn bindings() -> &'static RwLock<Vec<(PathBuf, Database)>> {
    static BINDINGS: OnceLock<RwLock<Vec<(PathBuf, Database)>>> = OnceLock::new();
    BINDINGS.get_or_init(|| RwLock::new(Vec::new()))
}

fn key_of(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

/// Keep the stores opened on `dir` in `database` from now on.
pub fn attach(dir: &Path, database: Database) {
    let key = key_of(dir);
    if let Ok(mut bound) = bindings().write() {
        bound.retain(|(path, _)| *path != key);
        bound.push((key, database));
    }
}

/// Go back to the files for `dir`.
pub fn detach(dir: &Path) {
    let key = key_of(dir);
    if let Ok(mut bound) = bindings().write() {
        bound.retain(|(path, _)| *path != key);
    }
}

/// The database `dir`'s stores are kept in, when one is attached.
#[must_use]
pub fn bound(dir: &Path) -> Option<Database> {
    let found = {
        let bound = bindings().read().ok()?;
        if bound.is_empty() {
            None
        } else {
            let key = key_of(dir);
            bound
                .iter()
                .find(|(path, _)| *path == key || path.as_path() == dir)
                .map(|(_, database)| database.clone())
        }
    };
    #[cfg(test)]
    if found.is_none() {
        return testing::auto_attach(dir);
    }
    found
}

// --- The transaction a store's writer lock holds, per thread ---

thread_local! {
    static HELD: std::cell::RefCell<Vec<(String, Tx)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A store writer's lock in the database: a transaction holding the
/// store's advisory lock, kept for this thread so the store's loads and
/// saves inside the lock run in it. Dropping the guard commits what was
/// saved, or rolls back when a statement failed.
pub(crate) struct Held {
    name: String,
}

impl Held {
    pub(crate) fn acquire(database: &Database, store: &str) -> Result<Self, Error> {
        let mut tx = database.transaction()?;
        tx.lock(&format!("openagents.store.{store}"))?;
        let name = store.to_string();
        HELD.with(|held| held.borrow_mut().push((name.clone(), tx)));
        Ok(Self { name })
    }
}

fn take_held(name: &str) -> Option<Tx> {
    HELD.with(|held| {
        let mut held = held.borrow_mut();
        let at = held.iter().rposition(|(store, _)| store == name)?;
        Some(held.remove(at).1)
    })
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(tx) = take_held(&self.name) {
            let _ = tx.commit();
        }
    }
}

/// Run `f` in the transaction this thread holds for `store`, or in a
/// fresh one (taking the store's lock) committed at the end.
pub(crate) fn in_store_tx<T>(
    database: &Database,
    store: &str,
    f: impl FnOnce(&mut Tx) -> Result<T, Error>,
) -> Result<T, Error> {
    let held = HELD.with(|held| {
        let mut held = held.borrow_mut();
        let at = held.iter().rposition(|(name, _)| name == store)?;
        Some(held.remove(at))
    });
    match held {
        Some((name, mut tx)) => {
            let out = f(&mut tx);
            if out.is_err() {
                tx.poison();
            }
            HELD.with(|held| held.borrow_mut().push((name, tx)));
            out
        }
        None => {
            let mut tx = database.transaction()?;
            tx.lock(&format!("openagents.store.{store}"))?;
            let out = f(&mut tx)?;
            tx.commit()?;
            Ok(out)
        }
    }
}

/// Commit the transaction this thread holds for `store` and hold the
/// store's lock again in a new one.
pub(crate) fn recommit(database: &Database, store: &str) -> Result<(), Error> {
    let Some(tx) = take_held(store) else {
        return Ok(());
    };
    tx.commit()?;
    let mut tx = database.transaction()?;
    tx.lock(&format!("openagents.store.{store}"))?;
    HELD.with(|held| held.borrow_mut().push((store.to_string(), tx)));
    Ok(())
}

/// Whether this thread holds `store`'s writer lock.
pub(crate) fn holds(store: &str) -> bool {
    HELD.with(|held| held.borrow().iter().any(|(name, _)| name == store))
}

#[cfg(test)]
pub(crate) mod testing {
    //! With `TENANCY_TEST_DATABASE_URL` set (a server the tests may
    //! create databases on), every store a test opens under a temporary
    //! directory runs on a fresh database of its own, so the store test
    //! suites run against Postgres unchanged. Unset, they run on files.
    use super::{Database, attach};
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};

    pub(crate) fn url() -> Option<String> {
        std::env::var("TENANCY_TEST_DATABASE_URL")
            .ok()
            .filter(|url| !url.is_empty())
    }

    fn admin() -> &'static Mutex<Option<(Database, String)>> {
        static ADMIN: OnceLock<Mutex<Option<(Database, String)>>> = OnceLock::new();
        ADMIN.get_or_init(|| Mutex::new(None))
    }

    /// A fresh, migrated database: a copy of a template migrated once.
    pub(crate) fn fresh_database() -> Option<Database> {
        let url = url()?;
        let mut admin = admin().lock().unwrap_or_else(|e| e.into_inner());
        if admin.is_none() {
            let database = Database::open(&url).expect("the test server answers");
            let template = format!("tenancy_tpl_{}", std::process::id());
            database
                .execute(&format!("DROP DATABASE IF EXISTS {template}"), &[])
                .expect("drop template");
            database
                .execute(&format!("CREATE DATABASE {template}"), &[])
                .expect("create template");
            let migrated = Database::connect(&with_dbname(&url, &template)).expect("migrate");
            drop(migrated);
            *admin = Some((database, template));
        }
        let (database, template) = admin.as_ref().expect("set above");
        let name = format!(
            "tenancy_t_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        // A template must have no other connection while it is copied.
        for _ in 0..50 {
            match database.execute(&format!("CREATE DATABASE {name} TEMPLATE {template}"), &[]) {
                Ok(_) => break,
                Err(error) if error.0.contains("being accessed") => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(error) => panic!("create test database: {error}"),
            }
        }
        Some(Database::open(&with_dbname(&url, &name)).expect("open test database"))
    }

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn with_dbname(url: &str, name: &str) -> String {
        format!("{url} dbname={name}")
    }

    fn on_files() -> &'static Mutex<Vec<std::path::PathBuf>> {
        static ON_FILES: OnceLock<Mutex<Vec<std::path::PathBuf>>> = OnceLock::new();
        ON_FILES.get_or_init(|| Mutex::new(Vec::new()))
    }

    /// Keep `dir` on files even with a test database configured.
    pub(crate) fn keep_on_files(dir: &Path) {
        if let Ok(canonical) = std::fs::canonicalize(dir) {
            on_files()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(canonical);
        }
    }

    pub(super) fn auto_attach(dir: &Path) -> Option<Database> {
        url()?;
        let temp = std::fs::canonicalize(std::env::temp_dir()).ok()?;
        let here = std::fs::canonicalize(dir).ok()?;
        if !here.starts_with(&temp)
            || on_files()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .any(|path| here.starts_with(path))
        {
            return None;
        }
        static ATTACHING: Mutex<()> = Mutex::new(());
        let _guard = ATTACHING.lock().unwrap_or_else(|e| e.into_inner());
        // Another thread may have attached it while this one waited.
        if let Ok(bound) = super::bindings().read()
            && let Some((_, database)) = bound.iter().find(|(path, _)| *path == here)
        {
            return Some(database.clone());
        }
        let database = fresh_database()?;
        attach(dir, database.clone());
        Some(database)
    }
}
