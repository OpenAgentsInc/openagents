//! Shared projects (#11585): a project has members, each with a role,
//! instead of living inside one account's GitHub-access record.
//!
//! | Role | Can |
//! | --- | --- |
//! | Owner | Everything an editor can, plus invite, change roles, remove members, rename, delete, and give the project to another member. Exactly one per project. |
//! | Editor | Read and add chats, files, and notes; edit instructions; add and remove repositories. |
//! | Viewer | Read the project's chats, files, notes, and instructions. |
//!
//! A member can always leave (the owner first gives the project to
//! someone else). Every call reads membership fresh, so removing a member
//! takes effect on their next request. A project the caller isn't in is
//! [`RepoError::NotFound`], the same as one that doesn't exist.
//!
//! Invitations are single-use and expire. The link carries a secret; only
//! its SHA-256 digest is kept, as `tenancy::accounts` keeps workspace
//! invitations. An invitation can't grant owner.
//!
//! Storage: in Postgres, `identity.projects` and
//! `identity.project_members` (migration 3, which copied every project
//! the GitHub-access records held, each owned by its account). On files
//! (local development and tests), one `projects/store.json` beside
//! `accounts.json`, written whole under a lock; when it doesn't exist yet,
//! it reads as the projects the `github-access/` records hold, each owned
//! by its account, and the first change writes it. An account is in at
//! most [`MAX_PROJECTS`] projects, its own and shared ones together.

use std::path::{Path, PathBuf};

use aes_gcm::aead::OsRng;
use aes_gcm::aead::rand_core::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tenancy::Accounts;

use super::{
    Lock, MAX_PROJECTS, Project, RepoError, STORE_DIR, check_account, create_private_dir, hex, now,
    project_id, read_record,
};

/// A member's role in a project.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    Editor,
    Owner,
}

impl Role {
    /// The word the API uses.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }

    /// Read [`Self::word`] back.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "owner" => Some(Self::Owner),
            "editor" => Some(Self::Editor),
            "viewer" => Some(Self::Viewer),
            _ => None,
        }
    }

    /// Adds and changes chats, files, notes, instructions, and
    /// repositories.
    #[must_use]
    pub fn edits(self) -> bool {
        self >= Self::Editor
    }

    /// Invites, changes roles, removes members, renames, deletes.
    #[must_use]
    pub fn owns(self) -> bool {
        self == Self::Owner
    }
}

/// A project as one member sees it: their role, where its files are kept
/// when that isn't their own storage, and how many people are in it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sharing {
    pub role: Role,
    /// The account whose storage keeps the project's files, notes, and
    /// Drive sources (the account that made it), when it isn't the
    /// reader's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    /// How many people are in it, the reader included.
    pub members: usize,
}

impl Sharing {
    /// Whether other people are in the project.
    #[must_use]
    pub fn shared(&self) -> bool {
        self.members > 1
    }
}

/// One member.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub account: String,
    pub role: Role,
    pub joined_unix: u64,
}

/// The most people in one project.
pub const MAX_MEMBERS: usize = 100;
/// The most open invitations one project keeps; a new one past this
/// replaces the oldest.
pub const MAX_INVITATIONS: usize = 50;
/// How long an invitation lasts when the request doesn't say.
pub const DEFAULT_INVITATION_SECONDS: u64 = 7 * 24 * 3600;
/// The shortest and longest an invitation may last.
pub const MIN_INVITATION_SECONDS: u64 = 600;
pub const MAX_INVITATION_SECONDS: u64 = 30 * 24 * 3600;
/// What an invitation link's code starts with.
pub const INVITATION_PREFIX: &str = "prjinv_";

/// One invitation, kept by its secret's digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Invitation {
    id: String,
    digest: String,
    role: Role,
    invited_by: String,
    created_unix: u64,
    expires_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accepted_by: Option<String>,
}

impl Invitation {
    fn open(&self, now: u64) -> bool {
        self.accepted_by.is_none() && self.expires_unix > now
    }
}

/// A project with its members and invitations, as kept.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Held {
    /// The project's own record (`sharing` is never kept).
    pub(crate) project: Project,
    pub(crate) home: String,
    pub(crate) members: Vec<Member>,
    pub(crate) invitations: Vec<Invitation>,
}

impl Held {
    fn new(project: Project, owner: &str) -> Self {
        let joined_unix = now();
        Self {
            project,
            home: owner.to_string(),
            members: vec![Member {
                account: owner.to_string(),
                role: Role::Owner,
                joined_unix,
            }],
            invitations: Vec::new(),
        }
    }

    /// `account`'s role, when it is a member.
    pub(crate) fn role_of(&self, account: &str) -> Option<Role> {
        self.members
            .iter()
            .find(|m| m.account == account)
            .map(|m| m.role)
    }

    /// The project as `account` sees it.
    pub(crate) fn shown_to(&self, account: &str) -> Project {
        let mut project = self.project.clone();
        project.sharing = self.role_of(account).map(|role| Sharing {
            role,
            home: (self.home != account).then(|| self.home.clone()),
            members: self.members.len(),
        });
        project
    }

    /// `account`'s role, or [`RepoError::NotFound`] for someone who isn't
    /// in the project (so ids don't leak).
    fn member(&self, account: &str) -> Result<Role, RepoError> {
        self.role_of(account).ok_or(RepoError::NotFound)
    }

    fn require(&self, account: &str, allowed: impl Fn(Role) -> bool) -> Result<Role, RepoError> {
        let role = self.member(account)?;
        if allowed(role) {
            Ok(role)
        } else {
            Err(RepoError::NotAllowed)
        }
    }
}

// ------------------------------------------------------------- storage

const PROJECTS_DIR: &str = "projects";
const STORE_SCHEMA: &str = "openagents.projects.v1";
const STORE_MAX: u64 = 64 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileStore {
    v: String,
    /// The next member's place in its account's list.
    next_position: u64,
    projects: Vec<FileProject>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileProject {
    project: Project,
    home: String,
    members: Vec<FileMember>,
    #[serde(default)]
    invitations: Vec<Invitation>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileMember {
    account: String,
    role: Role,
    joined_unix: u64,
    position: u64,
}

impl FileProject {
    fn held(&self) -> Held {
        Held {
            project: kept(self.project.clone()),
            home: self.home.clone(),
            members: self
                .members
                .iter()
                .map(|m| Member {
                    account: m.account.clone(),
                    role: m.role,
                    joined_unix: m.joined_unix,
                })
                .collect(),
            invitations: self.invitations.clone(),
        }
    }
}

/// The project record as kept: no reader's view on it.
fn kept(mut project: Project) -> Project {
    project.sharing = None;
    project
}

fn store_path(dir: &Path) -> PathBuf {
    dir.join(PROJECTS_DIR).join("store.json")
}

/// The file store, or (before its first change) the projects the
/// GitHub-access records hold, each owned by its account.
fn load_file(dir: &Path) -> Result<FileStore, RepoError> {
    let path = store_path(dir);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) => {
            if !meta.is_file() || meta.len() > STORE_MAX {
                return Err(RepoError::Storage);
            }
            let bytes = std::fs::read(&path).map_err(|_| RepoError::Storage)?;
            let store: FileStore =
                serde_json::from_slice(&bytes).map_err(|_| RepoError::Storage)?;
            if store.v != STORE_SCHEMA {
                return Err(RepoError::Storage);
            }
            Ok(store)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => imported(dir),
        Err(_) => Err(RepoError::Storage),
    }
}

/// The projects in the `github-access/` records (as migration 3 copies
/// them in Postgres): each owned by its account, in each account's order,
/// accounts by id; an id two accounts hold stays with the first.
fn imported(dir: &Path) -> Result<FileStore, RepoError> {
    let mut store = FileStore {
        v: STORE_SCHEMA.into(),
        next_position: 1,
        projects: Vec::new(),
    };
    let records = dir.join(STORE_DIR);
    let entries = match std::fs::read_dir(&records) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(store),
        Err(_) => return Err(RepoError::Storage),
    };
    let mut kept_records = Vec::new();
    for entry in entries {
        let path = entry.map_err(|_| RepoError::Storage)?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some(record) = read_record(&path)? {
            kept_records.push(record);
        }
    }
    kept_records.sort_by(|a, b| a.account.cmp(&b.account));
    for record in kept_records {
        for project in record.projects {
            if !project_id(&project.id) || store.projects.iter().any(|p| p.project.id == project.id)
            {
                continue;
            }
            let position = store.next_position;
            store.next_position += 1;
            store.projects.push(FileProject {
                members: vec![FileMember {
                    account: record.account.clone(),
                    role: Role::Owner,
                    joined_unix: project.created_unix,
                    position,
                }],
                project: kept(project),
                home: record.account.clone(),
                invitations: Vec::new(),
            });
        }
    }
    Ok(store)
}

fn write_file(dir: &Path, store: &FileStore) -> Result<(), RepoError> {
    use std::io::Write;
    let text = serde_json::to_vec_pretty(store).map_err(|_| RepoError::Storage)?;
    let temporary = dir.join(PROJECTS_DIR).join("store.tmp");
    std::fs::remove_file(&temporary).ok();
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut handle = options.open(&temporary).map_err(|_| RepoError::Storage)?;
    handle
        .write_all(&text)
        .and_then(|()| handle.sync_all())
        .map_err(|_| RepoError::Storage)?;
    std::fs::rename(&temporary, store_path(dir)).map_err(|_| RepoError::Storage)
}

#[cfg(feature = "postgres")]
fn from_row(row: tenancy::db::projects::Row) -> Result<Held, RepoError> {
    let project: Project = serde_json::from_value(row.record).map_err(|_| RepoError::Storage)?;
    let invitations: Vec<Invitation> =
        serde_json::from_value(row.invitations).map_err(|_| RepoError::Storage)?;
    let members = row
        .members
        .into_iter()
        .map(|m| {
            Ok(Member {
                role: Role::from_word(&m.role).ok_or(RepoError::Storage)?,
                account: m.account,
                joined_unix: u64::try_from(m.joined_unix).unwrap_or(0),
            })
        })
        .collect::<Result<Vec<_>, RepoError>>()?;
    if project.id != row.id {
        return Err(RepoError::Storage);
    }
    Ok(Held {
        project: kept(project),
        home: row.home,
        members,
        invitations,
    })
}

#[cfg(feature = "postgres")]
fn to_row(held: &Held) -> Result<tenancy::db::projects::Row, RepoError> {
    Ok(tenancy::db::projects::Row {
        id: held.project.id.clone(),
        home: held.home.clone(),
        record: serde_json::to_value(kept(held.project.clone())).map_err(|_| RepoError::Storage)?,
        invitations: serde_json::to_value(&held.invitations).map_err(|_| RepoError::Storage)?,
        members: held
            .members
            .iter()
            .map(|m| tenancy::db::projects::MemberRow {
                account: m.account.clone(),
                role: m.role.word().to_string(),
                joined_unix: i64::try_from(m.joined_unix).unwrap_or(i64::MAX),
            })
            .collect(),
    })
}

/// One project with its members, when it is kept.
pub(crate) fn held(dir: &Path, id: &str) -> Result<Option<Held>, RepoError> {
    if !project_id(id) {
        return Ok(None);
    }
    #[cfg(feature = "postgres")]
    if let Some(database) = tenancy::db::bound(dir) {
        return tenancy::db::projects::load(&database, id)
            .map_err(|_| RepoError::Storage)?
            .map(from_row)
            .transpose();
    }
    Ok(load_file(dir)?
        .projects
        .iter()
        .find(|p| p.project.id == id)
        .map(FileProject::held))
}

/// The projects `account` is in, in the order it got them.
pub(crate) fn held_by(dir: &Path, account: &str) -> Result<Vec<Held>, RepoError> {
    check_account(account)?;
    #[cfg(feature = "postgres")]
    if let Some(database) = tenancy::db::bound(dir) {
        return tenancy::db::projects::for_account(&database, account)
            .map_err(|_| RepoError::Storage)?
            .into_iter()
            .map(from_row)
            .collect();
    }
    let store = load_file(dir)?;
    let mut found: Vec<(u64, Held)> = store
        .projects
        .iter()
        .filter_map(|p| {
            p.members
                .iter()
                .find(|m| m.account == account)
                .map(|m| (m.position, p.held()))
        })
        .collect();
    found.sort_by_key(|(position, _)| *position);
    Ok(found.into_iter().map(|(_, held)| held).collect())
}

/// The projects `account` is in, as it sees them.
pub fn projects_of(dir: &Path, account: &str) -> Result<Vec<Project>, RepoError> {
    Ok(held_by(dir, account)?
        .iter()
        .map(|held| held.shown_to(account))
        .collect())
}

/// Change one project under its lock: `change` gets it (or `None`) and
/// returns what to keep (`None` removes it) and its answer. A member the
/// change adds who is already in [`MAX_PROJECTS`] projects refuses it
/// ([`RepoError::Full`]).
pub(crate) fn change<T>(
    dir: &Path,
    id: &str,
    change: impl FnOnce(Option<Held>) -> Result<(Option<Held>, T), RepoError>,
) -> Result<T, RepoError> {
    if !project_id(id) {
        return Err(RepoError::Invalid);
    }
    let checked = |next: Option<Held>| -> Result<Option<Held>, RepoError> {
        if let Some(next) = &next {
            let owners = next
                .members
                .iter()
                .filter(|m| m.role == Role::Owner)
                .count();
            if owners != 1
                || next.members.len() > MAX_MEMBERS
                || next.project.id != id
                || next
                    .members
                    .iter()
                    .any(|m| check_account(&m.account).is_err())
            {
                return Err(RepoError::Invalid);
            }
        }
        Ok(next.map(|mut held| {
            held.project = kept(held.project);
            held
        }))
    };
    #[cfg(feature = "postgres")]
    if let Some(database) = tenancy::db::bound(dir) {
        use tenancy::db::projects::Outcome;
        let outcome = tenancy::db::projects::update(&database, id, MAX_PROJECTS, |current| {
            let current = current.map(from_row).transpose()?;
            let (next, out) = change(current)?;
            let next = checked(next)?;
            Ok((next.as_ref().map(to_row).transpose()?, out))
        })
        .map_err(|_| RepoError::Storage)?;
        return match outcome {
            Outcome::Done(out) => Ok(out),
            Outcome::Refused(error) => Err(error),
            Outcome::Full(_) => Err(RepoError::Full),
        };
    }
    create_private_dir(&dir.join(PROJECTS_DIR))?;
    let _lock = Lock::acquire(&dir.join(PROJECTS_DIR).join("store.lock"))?;
    let mut store = load_file(dir)?;
    let at = store.projects.iter().position(|p| p.project.id == id);
    let current = at.map(|at| store.projects[at].held());
    let (next, out) = change(current.clone())?;
    let next = checked(next)?;
    if next == current {
        return Ok(out);
    }
    match next {
        None => {
            if let Some(at) = at {
                store.projects.remove(at);
            }
        }
        Some(next) => {
            let before: Vec<FileMember> = at
                .map(|at| store.projects[at].members.clone())
                .unwrap_or_default();
            let mut members = Vec::with_capacity(next.members.len());
            for member in &next.members {
                let position = match before.iter().find(|b| b.account == member.account) {
                    Some(kept) => kept.position,
                    None => {
                        let held = store
                            .projects
                            .iter()
                            .filter(|p| p.project.id != id)
                            .filter(|p| p.members.iter().any(|m| m.account == member.account))
                            .count();
                        if held >= MAX_PROJECTS {
                            return Err(RepoError::Full);
                        }
                        store.next_position += 1;
                        store.next_position - 1
                    }
                };
                members.push(FileMember {
                    account: member.account.clone(),
                    role: member.role,
                    joined_unix: member.joined_unix,
                    position,
                });
            }
            let entry = FileProject {
                project: next.project,
                home: next.home,
                members,
                invitations: next.invitations,
            };
            match at {
                Some(at) => store.projects[at] = entry,
                None => store.projects.push(entry),
            }
        }
    }
    write_file(dir, &store)?;
    Ok(out)
}

// ------------------------------------------------------------ the calls

/// Keep a new project owned by `account`.
pub(crate) fn create(dir: &Path, account: &str, project: Project) -> Result<Project, RepoError> {
    let id = project.id.clone();
    change(dir, &id, |current| {
        if current.is_some() {
            return Err(RepoError::Invalid);
        }
        let held = Held::new(project, account);
        let shown = held.shown_to(account);
        Ok((Some(held), shown))
    })
}

/// Change a project's own record as `account`: the owner only when
/// `owner_only` says so of the project as kept, else an editor or the
/// owner.
pub(crate) fn edit(
    dir: &Path,
    account: &str,
    id: &str,
    owner_only: impl FnOnce(&Project) -> bool,
    apply: impl FnOnce(&mut Project) -> Result<(), RepoError>,
) -> Result<Project, RepoError> {
    change(dir, id, |current| {
        let mut held = current.ok_or(RepoError::NotFound)?;
        let owner_only = owner_only(&held.project);
        held.require(account, |role| {
            if owner_only {
                role.owns()
            } else {
                role.edits()
            }
        })?;
        apply(&mut held.project)?;
        let shown = held.shown_to(account);
        Ok((Some(held), shown))
    })
}

/// Delete a project (its owner only). One that isn't there, or that
/// `account` can't see, is not an error, as before (#11585 keeps it).
pub(crate) fn remove(dir: &Path, account: &str, id: &str) -> Result<(), RepoError> {
    change(dir, id, |current| match current {
        None => Ok((None, ())),
        Some(held) => match held.role_of(account) {
            None => Ok((Some(held), ())),
            Some(role) if role.owns() => Ok((None, ())),
            Some(_) => Err(RepoError::NotAllowed),
        },
    })
}

/// One member as the members list shows them.
fn member_body(member: &Member, names: &Names, account: &str) -> Value {
    let (name, login) = names.of(&member.account);
    json!({
        "account": member.account,
        "role": member.role,
        "joined_unix": member.joined_unix,
        "name": name,
        "login": login,
        "you": member.account == account,
    })
}

/// Account names for showing members: the GitHub name, else the login,
/// else the account's label.
struct Names(Option<tenancy::accounts::Store>);

impl Names {
    fn read(dir: &Path) -> Self {
        Self(Accounts::open(dir).ok().and_then(|a| a.store().ok()))
    }

    fn of(&self, account: &str) -> (String, Option<String>) {
        let Some(store) = &self.0 else {
            return ("Someone".into(), None);
        };
        let github = store.identities.github_of(account);
        let login = github.map(|i| i.profile.login.clone());
        let name = github
            .and_then(|i| i.profile.name.clone())
            .filter(|n| !n.trim().is_empty())
            .or_else(|| login.clone())
            .or_else(|| store.accounts.get(account).map(|a| a.label.clone()))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "Someone".into());
        (name, login)
    }
}

/// `GET /v1/projects/{id}/members`: any member reads the list.
pub(crate) fn members(dir: &Path, account: &str, id: &str) -> Result<Value, RepoError> {
    let held = held(dir, id)?.ok_or(RepoError::NotFound)?;
    let role = held.member(account)?;
    let names = Names::read(dir);
    let now = now();
    let mut body = json!({
        "project": held.shown_to(account),
        "role": role,
        "members": held.members.iter().map(|m| member_body(m, &names, account)).collect::<Vec<_>>(),
    });
    if role.owns() {
        body["invitations"] = held
            .invitations
            .iter()
            .filter(|i| i.open(now))
            .map(|i| json!({"id": i.id, "role": i.role, "expires_unix": i.expires_unix}))
            .collect::<Vec<_>>()
            .into();
    }
    Ok(body)
}

/// `PATCH /v1/projects/{id}/members/{member}` `{role}`: the owner sets a
/// member's role; `owner` gives them the project, and the old owner
/// becomes an editor.
pub(crate) fn set_role(
    dir: &Path,
    account: &str,
    id: &str,
    member: &str,
    role: Role,
) -> Result<Value, RepoError> {
    change(dir, id, |current| {
        let mut held = current.ok_or(RepoError::NotFound)?;
        held.require(account, Role::owns)?;
        if held.role_of(member).is_none() {
            return Err(RepoError::NotFound);
        }
        if member == account {
            // The owner's own role changes only by giving the project away.
            return if role == Role::Owner {
                Ok((Some(held.clone()), held.shown_to(account)))
            } else {
                Err(RepoError::OwnerStays)
            };
        }
        for m in &mut held.members {
            if m.account == member {
                m.role = role;
            } else if role == Role::Owner && m.role == Role::Owner {
                m.role = Role::Editor;
            }
        }
        let shown = held.shown_to(account);
        Ok((Some(held), shown))
    })
    .map(|project| json!({"project": project}))
}

/// `DELETE /v1/projects/{id}/members/{member}`: the owner removes a
/// member; a member removing themselves leaves.
pub(crate) fn remove_member(
    dir: &Path,
    account: &str,
    id: &str,
    member: &str,
) -> Result<Value, RepoError> {
    if member == account {
        return leave(dir, account, id);
    }
    change(dir, id, |current| {
        let mut held = current.ok_or(RepoError::NotFound)?;
        held.require(account, Role::owns)?;
        let before = held.members.len();
        held.members.retain(|m| m.account != member);
        if held.members.len() == before {
            return Err(RepoError::NotFound);
        }
        let shown = held.shown_to(account);
        Ok((Some(held), shown))
    })
    .map(|project| json!({"project": project}))
}

/// `POST /v1/projects/{id}/leave`: any member but the owner.
pub(crate) fn leave(dir: &Path, account: &str, id: &str) -> Result<Value, RepoError> {
    change(dir, id, |current| {
        let mut held = current.ok_or(RepoError::NotFound)?;
        if held.member(account)?.owns() {
            return Err(RepoError::OwnerStays);
        }
        held.members.retain(|m| m.account != account);
        Ok((Some(held), ()))
    })
    .map(|()| json!({"left": id}))
}

fn digest(secret: &str) -> String {
    hex(&Sha256::digest(secret.as_bytes()))
}

/// The project id, invitation id, and secret in an invitation code
/// (`prjinv_` + the project's 16 hex + `_` + 16 hex + `_` + 64 hex).
fn split_code(code: &str) -> Option<(String, String, String)> {
    let rest = code.trim().strip_prefix(INVITATION_PREFIX)?;
    let mut parts = rest.split('_');
    let (project, id, secret) = (parts.next()?, parts.next()?, parts.next()?);
    let hexed = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_hexdigit());
    if parts.next().is_some() || !hexed(project, 16) || !hexed(id, 16) || !hexed(secret, 64) {
        return None;
    }
    Some((
        format!("prj_{}", project.to_ascii_lowercase()),
        id.to_ascii_lowercase(),
        secret.to_ascii_lowercase(),
    ))
}

/// `POST /v1/projects/{id}/invitations` `{role?, seconds?}`: the owner
/// makes a single-use link for an editor (the default) or a viewer. The
/// code is shown once; only its digest is kept.
pub(crate) fn invite(
    dir: &Path,
    account: &str,
    id: &str,
    role: Role,
    seconds: u64,
) -> Result<Value, RepoError> {
    if role == Role::Owner {
        return Err(RepoError::Invalid);
    }
    let seconds = if seconds == 0 {
        DEFAULT_INVITATION_SECONDS
    } else {
        seconds.clamp(MIN_INVITATION_SECONDS, MAX_INVITATION_SECONDS)
    };
    let mut raw = [0u8; 8];
    OsRng.fill_bytes(&mut raw);
    let invitation_id = hex(&raw);
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let secret_hex = hex(&secret);
    secret.fill(0);
    let now = now();
    let expires_unix = now + seconds;
    let project_hex = id.strip_prefix("prj_").unwrap_or_default().to_string();
    let code = format!("{INVITATION_PREFIX}{project_hex}_{invitation_id}_{secret_hex}");
    change(dir, id, |current| {
        let mut held = current.ok_or(RepoError::NotFound)?;
        held.require(account, Role::owns)?;
        if held.members.len() >= MAX_MEMBERS {
            return Err(RepoError::ProjectFull);
        }
        held.invitations.retain(|i| i.open(now));
        while held.invitations.len() >= MAX_INVITATIONS {
            held.invitations.remove(0);
        }
        held.invitations.push(Invitation {
            id: invitation_id.clone(),
            digest: digest(&secret_hex),
            role,
            invited_by: account.to_string(),
            created_unix: now,
            expires_unix,
            accepted_by: None,
        });
        Ok((Some(held), ()))
    })?;
    Ok(json!({
        "code": code,
        "invitation": {"id": invitation_id, "role": role, "expires_unix": expires_unix},
    }))
}

/// The open invitation `code` names, checked against its digest.
fn find<'a>(held: &'a Held, id: &str, secret: &str, now: u64) -> Option<&'a Invitation> {
    let wanted = digest(secret);
    held.invitations
        .iter()
        .find(|i| i.id == id && bool::from(i.digest.as_bytes().ct_eq(wanted.as_bytes())))
        .filter(|i| i.open(now))
}

/// `POST /v1/projects/invitations/preview` `{code}`: what the link offers
/// (the project's name, who invited, the role), without using it.
pub(crate) fn preview(dir: &Path, account: &str, code: &str) -> Result<Value, RepoError> {
    check_account(account)?;
    let (project, id, secret) = split_code(code).ok_or(RepoError::InvitationGone)?;
    let held = held(dir, &project)?.ok_or(RepoError::InvitationGone)?;
    let invitation = find(&held, &id, &secret, now()).ok_or(RepoError::InvitationGone)?;
    let names = Names::read(dir);
    let (inviter, _) = names.of(&invitation.invited_by);
    Ok(json!({
        "project": {"id": held.project.id, "name": held.project.name},
        "role": invitation.role,
        "invited_by": inviter,
        "expires_unix": invitation.expires_unix,
        "member": held.role_of(account),
    }))
}

/// `POST /v1/projects/invitations/accept` `{code}`: join with the
/// invitation's role, using it up. Someone already in the project keeps
/// their role and leaves the invitation for whoever it was meant for.
pub(crate) fn accept(dir: &Path, account: &str, code: &str) -> Result<Value, RepoError> {
    check_account(account)?;
    let (project, id, secret) = split_code(code).ok_or(RepoError::InvitationGone)?;
    let now = now();
    change(dir, &project, |current| {
        let mut held = current.ok_or(RepoError::InvitationGone)?;
        let role = find(&held, &id, &secret, now)
            .ok_or(RepoError::InvitationGone)?
            .role;
        if held.role_of(account).is_some() {
            let shown = held.shown_to(account);
            return Ok((Some(held), shown));
        }
        if held.members.len() >= MAX_MEMBERS {
            return Err(RepoError::ProjectFull);
        }
        for invitation in &mut held.invitations {
            if invitation.id == id {
                invitation.accepted_by = Some(account.to_string());
            }
        }
        held.invitations.retain(|i| i.open(now) || i.id == id);
        held.members.push(Member {
            account: account.to_string(),
            role,
            joined_unix: now,
        });
        let shown = held.shown_to(account);
        Ok((Some(held), shown))
    })
    .map(|project| json!({"project": project}))
}

#[cfg(test)]
mod tests;
