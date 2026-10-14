//! Projects and their members (#11585, migration 3): `identity.projects`
//! (the project's own record, its home account, its invitations) and
//! `identity.project_members` (who may use it, with one owner).
//! `oa_auth::repos` owns the record types and every rule; this module
//! only reads and writes the rows, a writer holding the project's lock for
//! its read-modify-write.

use serde_json::Value;

use super::{Database, Error, Tx};

/// One member as kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRow {
    pub account: String,
    /// `owner`, `editor`, or `viewer`.
    pub role: String,
    pub joined_unix: i64,
}

/// One project as kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: String,
    /// The account whose storage holds the project's files.
    pub home: String,
    /// The project record (`oa_auth::repos::Project`).
    pub record: Value,
    /// Its invitations, as a JSON array.
    pub invitations: Value,
    /// Its members, in the order they joined.
    pub members: Vec<MemberRow>,
}

/// What [`update`] did.
#[derive(Debug)]
pub enum Outcome<T, E> {
    /// Written (or left as it was), with the change's answer.
    Done(T),
    /// The change refused; nothing written.
    Refused(E),
    /// This account would be in more than the limit's projects; nothing
    /// written.
    Full(String),
}

fn members_of(tx_rows: Vec<tokio_postgres::Row>) -> Vec<(String, MemberRow)> {
    tx_rows
        .into_iter()
        .map(|row| {
            (
                row.get::<_, String>(0),
                MemberRow {
                    account: row.get(1),
                    role: row.get(2),
                    joined_unix: row.get(3),
                },
            )
        })
        .collect()
}

const MEMBERS: &str = "SELECT project_id, account_id, role, joined_unix FROM identity.project_members WHERE project_id = ANY($1) ORDER BY position";

fn rows(projects: Vec<tokio_postgres::Row>, members: Vec<(String, MemberRow)>) -> Vec<Row> {
    let mut found: Vec<Row> = projects
        .into_iter()
        .map(|row| Row {
            id: row.get(0),
            home: row.get(1),
            record: row.get(2),
            invitations: row.get(3),
            members: Vec::new(),
        })
        .collect();
    for (project, member) in members {
        if let Some(row) = found.iter_mut().find(|row| row.id == project) {
            row.members.push(member);
        }
    }
    found
}

/// One project, when it is kept.
///
/// # Errors
///
/// The read fails.
pub fn load(database: &Database, id: &str) -> Result<Option<Row>, Error> {
    let projects = database.query(
        "SELECT id, home_account, record, invitations FROM identity.projects WHERE id = $1",
        &[&id],
    )?;
    if projects.is_empty() {
        return Ok(None);
    }
    let ids = vec![id.to_string()];
    let members = members_of(database.query(MEMBERS, &[&ids])?);
    Ok(rows(projects, members).pop())
}

/// The projects `account` is a member of, in the order it got them.
///
/// # Errors
///
/// The read fails.
pub fn for_account(database: &Database, account: &str) -> Result<Vec<Row>, Error> {
    let projects = database.query(
        "SELECT p.id, p.home_account, p.record, p.invitations
           FROM identity.project_members m
           JOIN identity.projects p ON p.id = m.project_id
          WHERE m.account_id = $1
          ORDER BY m.position",
        &[&account],
    )?;
    let ids: Vec<String> = projects.iter().map(|row| row.get(0)).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let members = members_of(database.query(MEMBERS, &[&ids])?);
    Ok(rows(projects, members))
}

fn load_in(tx: &mut Tx, id: &str) -> Result<Option<Row>, Error> {
    let projects = tx.query(
        "SELECT id, home_account, record, invitations FROM identity.projects WHERE id = $1 FOR UPDATE",
        &[&id],
    )?;
    if projects.is_empty() {
        return Ok(None);
    }
    let ids = vec![id.to_string()];
    let members = members_of(tx.query(MEMBERS, &[&ids])?);
    Ok(rows(projects, members).pop())
}

/// Change one project under its lock: `change` gets the kept project (or
/// `None`) and returns the project to keep (`None` removes it) and its
/// answer. An account the change adds as a member who is already in
/// `limit` projects stops the change ([`Outcome::Full`]). Roles are
/// written demotions first, so the one-owner index holds at every step.
///
/// # Errors
///
/// The database fails.
pub fn update<T, E>(
    database: &Database,
    id: &str,
    limit: usize,
    change: impl FnOnce(Option<Row>) -> Result<(Option<Row>, T), E>,
) -> Result<Outcome<T, E>, Error> {
    let mut tx = database.transaction()?;
    tx.lock(&format!("openagents.project.{id}"))?;
    let current = load_in(&mut tx, id)?;
    let (next, out) = match change(current.clone()) {
        Ok(changed) => changed,
        Err(error) => return Ok(Outcome::Refused(error)),
    };
    if next.as_ref() == current.as_ref() {
        return Ok(Outcome::Done(out));
    }
    let Some(next) = next else {
        tx.execute("DELETE FROM identity.projects WHERE id = $1", &[&id])?;
        tx.commit()?;
        return Ok(Outcome::Done(out));
    };
    let before: Vec<MemberRow> = current.map(|row| row.members).unwrap_or_default();
    let mut added: Vec<&str> = next
        .members
        .iter()
        .filter(|m| !before.iter().any(|b| b.account == m.account))
        .map(|m| m.account.as_str())
        .collect();
    added.sort_unstable();
    for account in &added {
        tx.lock(&format!("openagents.project-member.{account}"))?;
        let held: i64 = tx
            .query(
                "SELECT count(*) FROM identity.project_members WHERE account_id = $1 AND project_id <> $2",
                &[account, &id],
            )?
            .first()
            .map_or(0, |row| row.get(0));
        if usize::try_from(held).unwrap_or(usize::MAX) >= limit {
            return Ok(Outcome::Full((*account).to_string()));
        }
    }
    tx.execute(
        "INSERT INTO identity.projects (id, home_account, record, invitations) VALUES ($1, $2, $3, $4)
         ON CONFLICT (id) DO UPDATE SET home_account = EXCLUDED.home_account, record = EXCLUDED.record,
             invitations = EXCLUDED.invitations, updated_at = now()",
        &[&id, &next.home, &next.record, &next.invitations],
    )?;
    for gone in before
        .iter()
        .filter(|b| !next.members.iter().any(|m| m.account == b.account))
    {
        tx.execute(
            "DELETE FROM identity.project_members WHERE project_id = $1 AND account_id = $2",
            &[&id, &gone.account],
        )?;
    }
    // Demotions first, the new owner last.
    let mut changed: Vec<&MemberRow> = next
        .members
        .iter()
        .filter(|m| {
            before
                .iter()
                .any(|b| b.account == m.account && b.role != m.role)
        })
        .collect();
    changed.sort_by_key(|m| m.role == "owner");
    for member in changed {
        tx.execute(
            "UPDATE identity.project_members SET role = $3 WHERE project_id = $1 AND account_id = $2",
            &[&id, &member.account, &member.role],
        )?;
    }
    for member in next
        .members
        .iter()
        .filter(|m| !before.iter().any(|b| b.account == m.account))
    {
        tx.execute(
            "INSERT INTO identity.project_members (project_id, account_id, role, joined_unix) VALUES ($1, $2, $3, $4)",
            &[&id, &member.account, &member.role, &member.joined_unix],
        )?;
    }
    tx.commit()?;
    Ok(Outcome::Done(out))
}

/// What migration 3 copied, for the dry run: the projects kept in GitHub
/// access records, the project rows, the owner rows, and any project id
/// kept by more than one account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Census {
    pub kept_in_records: i64,
    pub accounts_with_projects: i64,
    pub project_rows: i64,
    pub member_rows: i64,
    pub owner_rows: i64,
    pub duplicate_ids: Vec<String>,
    /// Projects in a record with no row owned by that account.
    pub missing: Vec<String>,
    /// Rows whose record differs from the record's copy.
    pub changed: Vec<String>,
}

/// Count what migration 3 copied (`tenant-db projects-census`).
///
/// # Errors
///
/// A read fails.
pub fn census(database: &Database) -> Result<Census, Error> {
    let one = |sql: &str| -> Result<i64, Error> {
        Ok(database
            .query(sql, &[])?
            .first()
            .map_or(0, |row| row.get(0)))
    };
    let list = |sql: &str| -> Result<Vec<String>, Error> {
        Ok(database
            .query(sql, &[])?
            .into_iter()
            .map(|row| row.get(0))
            .collect())
    };
    const KEPT: &str = "SELECT access.account_id, kept.value
          FROM identity.github_access AS access
         CROSS JOIN LATERAL jsonb_array_elements(
               CASE WHEN jsonb_typeof(access.record -> 'projects') = 'array'
                    THEN access.record -> 'projects' ELSE '[]'::jsonb END) AS kept (value)";
    Ok(Census {
        kept_in_records: one(&format!("SELECT count(*) FROM ({KEPT}) k"))?,
        accounts_with_projects: one(&format!(
            "SELECT count(DISTINCT account_id) FROM ({KEPT}) k"
        ))?,
        project_rows: one("SELECT count(*) FROM identity.projects")?,
        member_rows: one("SELECT count(*) FROM identity.project_members")?,
        owner_rows: one("SELECT count(*) FROM identity.project_members WHERE role = 'owner'")?,
        duplicate_ids: list(&format!(
            "SELECT value ->> 'id' FROM ({KEPT}) k GROUP BY 1 HAVING count(*) > 1 ORDER BY 1"
        ))?,
        missing: list(&format!(
            "SELECT k.value ->> 'id' FROM ({KEPT}) k
              WHERE NOT EXISTS (SELECT 1 FROM identity.project_members m
                                 WHERE m.project_id = k.value ->> 'id' AND m.account_id = k.account_id
                                   AND m.role = 'owner')
              ORDER BY 1"
        ))?,
        changed: list(&format!(
            "SELECT k.value ->> 'id' FROM ({KEPT}) k
               JOIN identity.projects p ON p.id = k.value ->> 'id' AND p.home_account = k.account_id
              WHERE p.record <> k.value ORDER BY 1"
        ))?,
    })
}
