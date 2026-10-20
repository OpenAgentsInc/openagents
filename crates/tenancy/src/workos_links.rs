//! Links between our accounts and workspaces and their WorkOS twins
//! (#12320, docs/auth/2026-10-20-workos-source-of-truth.md).
//!
//! One WorkOS User per account (`acct_…` to `user_…`) and one WorkOS
//! Organization per shared workspace (`ws_…` to `org_…`). Each side of a
//! link is unique: a WorkOS User names one account and an account has one
//! User. The account and workspace documents never change; the links live
//! beside them, in `workos-links.json` on files or in migration 4's
//! `identity.workos_users` and `workspace.workos_orgs` in Postgres, so a
//! binary from before this module still opens the stores.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store_lock::StoreLock;

/// The file the links live in on files.
pub const FILE: &str = "workos-links.json";
const LOCK: &str = "workos-links.lock";
const SCHEMA: &str = "openagents.workos-links.v1";
const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Why a link could not be read or made.
#[derive(Debug)]
pub enum LinkError {
    /// The other side is already linked to someone else.
    Taken,
    /// An ID isn't the shape WorkOS or we make.
    Invalid,
    /// The store can't be read or written; the detail is the store's.
    Unavailable(String),
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Taken => f.write_str("already linked to another record"),
            Self::Invalid => f.write_str("not a valid ID"),
            Self::Unavailable(detail) => write!(f, "links unavailable: {detail}"),
        }
    }
}

impl std::error::Error for LinkError {}

impl From<std::io::Error> for LinkError {
    fn from(error: std::io::Error) -> Self {
        Self::Unavailable(error.to_string())
    }
}

/// One link as kept.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Link {
    /// The WorkOS ID (`user_…` or `org_…`).
    pub workos: String,
    /// When it was made, Unix seconds.
    pub linked: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Book {
    v: String,
    #[serde(default)]
    users: BTreeMap<String, Link>,
    #[serde(default)]
    orgs: BTreeMap<String, Link>,
}

/// Which pair of tables or maps a call reads.
#[derive(Clone, Copy)]
enum Side {
    Users,
    Orgs,
}

/// Whether `id` is a WorkOS User ID.
#[must_use]
pub fn valid_user(id: &str) -> bool {
    valid_workos(id, "user_")
}

/// Whether `id` is a WorkOS Organization ID.
#[must_use]
pub fn valid_org(id: &str) -> bool {
    valid_workos(id, "org_")
}

fn valid_workos(id: &str, prefix: &str) -> bool {
    id.strip_prefix(prefix).is_some_and(|rest| {
        !rest.is_empty() && rest.len() <= 64 && rest.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

fn valid_account(id: &str) -> bool {
    id.strip_prefix("acct_").is_some_and(|hex| {
        hex.len() == 16 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

fn valid_workspace(id: &str) -> bool {
    id.strip_prefix("ws_").is_some_and(|rest| {
        !rest.is_empty()
            && rest.len() <= 64
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    })
}

/// The links kept for one registry directory.
#[derive(Clone, Debug)]
pub struct Links {
    dir: PathBuf,
}

impl Links {
    /// The links beside the stores in `dir`. Nothing is read until asked.
    #[must_use]
    pub fn open(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    /// The WorkOS User linked to `account`.
    pub fn user_of(&self, account: &str) -> Result<Option<String>, LinkError> {
        self.ours(Side::Users, account)
    }

    /// The account linked to the WorkOS User `user`.
    pub fn account_of(&self, user: &str) -> Result<Option<String>, LinkError> {
        self.theirs(Side::Users, user)
    }

    /// Link `account` to `user`. Linking a pair that is already linked is
    /// a no-op; either side linked elsewhere is [`LinkError::Taken`].
    pub fn link_user(&self, account: &str, user: &str) -> Result<(), LinkError> {
        if !valid_account(account) || !valid_user(user) {
            return Err(LinkError::Invalid);
        }
        self.link(Side::Users, account, user)
    }

    /// Forget `account`'s link, answering the User it named.
    pub fn unlink_account(&self, account: &str) -> Result<Option<String>, LinkError> {
        self.unlink(Side::Users, account)
    }

    /// The WorkOS Organization linked to `workspace`.
    pub fn org_of(&self, workspace: &str) -> Result<Option<String>, LinkError> {
        self.ours(Side::Orgs, workspace)
    }

    /// The workspace linked to the WorkOS Organization `org`.
    pub fn workspace_of(&self, org: &str) -> Result<Option<String>, LinkError> {
        self.theirs(Side::Orgs, org)
    }

    /// Link `workspace` to `org`, as [`Self::link_user`] does.
    pub fn link_org(&self, workspace: &str, org: &str) -> Result<(), LinkError> {
        if !valid_workspace(workspace) || !valid_org(org) {
            return Err(LinkError::Invalid);
        }
        self.link(Side::Orgs, workspace, org)
    }

    /// Forget `workspace`'s link, answering the Organization it named.
    pub fn unlink_workspace(&self, workspace: &str) -> Result<Option<String>, LinkError> {
        self.unlink(Side::Orgs, workspace)
    }

    /// Every account link, in account order.
    pub fn users(&self) -> Result<Vec<(String, Link)>, LinkError> {
        self.all(Side::Users)
    }

    /// Every workspace link, in workspace order.
    pub fn orgs(&self) -> Result<Vec<(String, Link)>, LinkError> {
        self.all(Side::Orgs)
    }

    fn ours(&self, side: Side, ours: &str) -> Result<Option<String>, LinkError> {
        #[cfg(feature = "postgres")]
        if let Some(database) = crate::db::bound(&self.dir) {
            return pg::ours(&database, side, ours);
        }
        Ok(self
            .read()?
            .map(side)
            .get(ours)
            .map(|link| link.workos.clone()))
    }

    fn theirs(&self, side: Side, theirs: &str) -> Result<Option<String>, LinkError> {
        #[cfg(feature = "postgres")]
        if let Some(database) = crate::db::bound(&self.dir) {
            return pg::theirs(&database, side, theirs);
        }
        Ok(self
            .read()?
            .map(side)
            .iter()
            .find(|(_, link)| link.workos == theirs)
            .map(|(ours, _)| ours.clone()))
    }

    fn all(&self, side: Side) -> Result<Vec<(String, Link)>, LinkError> {
        #[cfg(feature = "postgres")]
        if let Some(database) = crate::db::bound(&self.dir) {
            return pg::all(&database, side);
        }
        Ok(self
            .read()?
            .map(side)
            .iter()
            .map(|(ours, link)| (ours.clone(), link.clone()))
            .collect())
    }

    fn link(&self, side: Side, ours: &str, theirs: &str) -> Result<(), LinkError> {
        #[cfg(feature = "postgres")]
        if let Some(database) = crate::db::bound(&self.dir) {
            return pg::link(&database, side, ours, theirs);
        }
        self.mutate(|book| {
            let map = book.map_mut(side);
            if let Some(existing) = map.get(ours) {
                return if existing.workos == theirs {
                    Ok(false)
                } else {
                    Err(LinkError::Taken)
                };
            }
            if map.values().any(|link| link.workos == theirs) {
                return Err(LinkError::Taken);
            }
            map.insert(
                ours.to_string(),
                Link {
                    workos: theirs.to_string(),
                    linked: now(),
                },
            );
            Ok(true)
        })
        .map(|_| ())
    }

    fn unlink(&self, side: Side, ours: &str) -> Result<Option<String>, LinkError> {
        #[cfg(feature = "postgres")]
        if let Some(database) = crate::db::bound(&self.dir) {
            return pg::unlink(&database, side, ours);
        }
        let mut removed = None;
        self.mutate(|book| {
            removed = book.map_mut(side).remove(ours).map(|link| link.workos);
            Ok(removed.is_some())
        })?;
        Ok(removed)
    }

    fn read(&self) -> Result<Book, LinkError> {
        let path = self.dir.join(FILE);
        let mut text = String::new();
        match std::fs::File::open(&path) {
            Ok(file) => {
                file.take(MAX_BYTES + 1).read_to_string(&mut text)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Book {
                    v: SCHEMA.into(),
                    ..Book::default()
                });
            }
            Err(error) => return Err(error.into()),
        }
        if text.len() as u64 > MAX_BYTES {
            return Err(LinkError::Unavailable("the links file is too large".into()));
        }
        let book: Book = serde_json::from_str(&text).map_err(|error| {
            LinkError::Unavailable(format!("the links file is malformed: {error}"))
        })?;
        if book.v != SCHEMA {
            return Err(LinkError::Unavailable(
                "the links file has another schema".into(),
            ));
        }
        Ok(book)
    }

    /// Run `change` under the writer lock and save when it says so.
    fn mutate(
        &self,
        change: impl FnOnce(&mut Book) -> Result<bool, LinkError>,
    ) -> Result<(), LinkError> {
        std::fs::create_dir_all(&self.dir)?;
        let _lock = StoreLock::acquire(&self.dir.join(LOCK), 500).map_err(|failure| {
            LinkError::Unavailable(match failure {
                crate::store_lock::LockFailure::Held(_) => "the links are busy".to_string(),
                crate::store_lock::LockFailure::Io(error) => error.to_string(),
                crate::store_lock::LockFailure::Invalid(why) => why.to_string(),
            })
        })?;
        let mut book = self.read()?;
        if !change(&mut book)? {
            return Ok(());
        }
        let text = serde_json::to_string_pretty(&book)
            .map_err(|error| LinkError::Unavailable(error.to_string()))?;
        let temporary = self.dir.join(format!("{FILE}.{}.tmp", std::process::id()));
        let _ = std::fs::remove_file(&temporary);
        {
            let mut options = std::fs::OpenOptions::new();
            options.create_new(true).write(true);
            let mut file = crate::private_fs::mode(&mut options, 0o600)?.open(&temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&temporary, self.dir.join(FILE))?;
        Ok(())
    }
}

impl Book {
    fn map(&self, side: Side) -> &BTreeMap<String, Link> {
        match side {
            Side::Users => &self.users,
            Side::Orgs => &self.orgs,
        }
    }

    fn map_mut(&mut self, side: Side) -> &mut BTreeMap<String, Link> {
        if self.v.is_empty() {
            self.v = SCHEMA.into();
        }
        match side {
            Side::Users => &mut self.users,
            Side::Orgs => &mut self.orgs,
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

#[cfg(feature = "postgres")]
mod pg {
    use super::{Link, LinkError, Side};
    use crate::db::Database;

    fn table(side: Side) -> (&'static str, &'static str, &'static str) {
        match side {
            Side::Users => ("identity.workos_users", "account_id", "workos_user_id"),
            Side::Orgs => ("workspace.workos_orgs", "workspace_id", "workos_org_id"),
        }
    }

    fn fault(error: crate::db::Error) -> LinkError {
        LinkError::Unavailable(error.to_string())
    }

    pub(super) fn ours(
        database: &Database,
        side: Side,
        ours: &str,
    ) -> Result<Option<String>, LinkError> {
        let (table, key, value) = table(side);
        let rows = database
            .query(
                &format!("SELECT {value} FROM {table} WHERE {key} = $1"),
                &[&ours],
            )
            .map_err(fault)?;
        Ok(rows.first().map(|row| row.get(0)))
    }

    pub(super) fn theirs(
        database: &Database,
        side: Side,
        theirs: &str,
    ) -> Result<Option<String>, LinkError> {
        let (table, key, value) = table(side);
        let rows = database
            .query(
                &format!("SELECT {key} FROM {table} WHERE {value} = $1"),
                &[&theirs],
            )
            .map_err(fault)?;
        Ok(rows.first().map(|row| row.get(0)))
    }

    pub(super) fn all(database: &Database, side: Side) -> Result<Vec<(String, Link)>, LinkError> {
        let (table, key, value) = table(side);
        let rows = database
            .query(
                &format!(
                    "SELECT {key}, {value}, extract(epoch FROM linked_at)::bigint FROM {table} ORDER BY {key}"
                ),
                &[],
            )
            .map_err(fault)?;
        Ok(rows
            .iter()
            .map(|row| {
                let linked: i64 = row.get(2);
                (
                    row.get(0),
                    Link {
                        workos: row.get(1),
                        linked: u64::try_from(linked).unwrap_or_default(),
                    },
                )
            })
            .collect())
    }

    pub(super) fn link(
        database: &Database,
        side: Side,
        ours: &str,
        theirs: &str,
    ) -> Result<(), LinkError> {
        let (table, key, value) = table(side);
        let inserted = database
            .execute(
                &format!(
                    "INSERT INTO {table} ({key}, {value}) VALUES ($1, $2) ON CONFLICT DO NOTHING"
                ),
                &[&ours, &theirs],
            )
            .map_err(fault)?;
        if inserted == 1 {
            return Ok(());
        }
        // Nothing inserted: the same pair is a no-op, anything else is taken.
        match self::ours(database, side, ours)? {
            Some(existing) if existing == theirs => Ok(()),
            _ => Err(LinkError::Taken),
        }
    }

    pub(super) fn unlink(
        database: &Database,
        side: Side,
        ours: &str,
    ) -> Result<Option<String>, LinkError> {
        let (table, key, value) = table(side);
        let rows = database
            .query(
                &format!("DELETE FROM {table} WHERE {key} = $1 RETURNING {value}"),
                &[&ours],
            )
            .map_err(fault)?;
        Ok(rows.first().map(|row| row.get(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "acct_0123456789abcdef";
    const B: &str = "acct_fedcba9876543210";

    #[test]
    fn a_user_links_to_one_account_and_back() {
        let dir = tempfile::tempdir().unwrap();
        let links = Links::open(dir.path());
        assert_eq!(links.user_of(A).unwrap(), None);
        links.link_user(A, "user_01ABC").unwrap();
        links.link_user(A, "user_01ABC").unwrap();
        assert_eq!(links.user_of(A).unwrap().as_deref(), Some("user_01ABC"));
        assert_eq!(links.account_of("user_01ABC").unwrap().as_deref(), Some(A));
        assert!(matches!(
            links.link_user(B, "user_01ABC"),
            Err(LinkError::Taken)
        ));
        assert!(matches!(
            links.link_user(A, "user_02XYZ"),
            Err(LinkError::Taken)
        ));
        assert!(matches!(
            links.link_user("acct_x", "user_1"),
            Err(LinkError::Invalid)
        ));
        assert_eq!(links.users().unwrap().len(), 1);
        assert_eq!(
            links.unlink_account(A).unwrap().as_deref(),
            Some("user_01ABC")
        );
        assert_eq!(links.account_of("user_01ABC").unwrap(), None);
        links.link_user(B, "user_01ABC").unwrap();
    }

    #[test]
    fn a_workspace_links_to_one_organization() {
        let dir = tempfile::tempdir().unwrap();
        let links = Links::open(dir.path());
        links.link_org("ws_0123456789abcdef", "org_01ABC").unwrap();
        assert_eq!(
            links.workspace_of("org_01ABC").unwrap().as_deref(),
            Some("ws_0123456789abcdef")
        );
        assert!(matches!(
            links.link_org("ws_other", "org_01ABC"),
            Err(LinkError::Taken)
        ));
        assert_eq!(links.orgs().unwrap().len(), 1);
        links.unlink_workspace("ws_0123456789abcdef").unwrap();
        assert!(links.orgs().unwrap().is_empty());
    }
}
