//! The dual-write (#12321) and the backfill (#12322): bring WorkOS in
//! line with our stores for one account, one workspace, or all of them.
//!
//! Our stores stay the source of truth. Every write is keyed by external
//! ID (`acct_…` on a User, `ws_…` on an Organization), so running a sync
//! twice changes nothing the second time, and a sync interrupted halfway
//! finishes on the next run. Only organization workspaces become
//! Organizations (owner decision 5); a personal workspace is its User.
//! Agent accounts aren't Users.
//!
//! With `apply` off, a sync reads WorkOS and reports what it would write
//! without writing anything: the backfill's `--dry-run`.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use tenancy::accounts::deletion::DELETED_LABEL;
use tenancy::workos_links::Links;
use tenancy::{Account, Accounts, MemberStatus, Store, Workspace, WorkspaceKind};

use super::client::{NewUser, Refusal, UserChange, Workos};

/// What one sync did, or would do with `apply` off.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    pub users_created: u32,
    pub users_linked: u32,
    pub users_updated: u32,
    pub users_deleted: u32,
    pub orgs_created: u32,
    pub orgs_linked: u32,
    pub orgs_updated: u32,
    pub orgs_deleted: u32,
    pub memberships_added: u32,
    pub memberships_changed: u32,
    pub memberships_removed: u32,
    /// Accounts or workspaces skipped, with why.
    pub skipped: Vec<String>,
    /// Records that couldn't be written, with why (no secrets).
    pub failed: Vec<String>,
}

impl Report {
    fn add(&mut self, other: Report) {
        self.users_created += other.users_created;
        self.users_linked += other.users_linked;
        self.users_updated += other.users_updated;
        self.users_deleted += other.users_deleted;
        self.orgs_created += other.orgs_created;
        self.orgs_linked += other.orgs_linked;
        self.orgs_updated += other.orgs_updated;
        self.orgs_deleted += other.orgs_deleted;
        self.memberships_added += other.memberships_added;
        self.memberships_changed += other.memberships_changed;
        self.memberships_removed += other.memberships_removed;
        self.skipped.extend(other.skipped);
        self.failed.extend(other.failed);
    }

    /// Whether anything was (or would be) written.
    #[must_use]
    pub fn wrote(&self) -> bool {
        self.users_created
            + self.users_linked
            + self.users_updated
            + self.users_deleted
            + self.orgs_created
            + self.orgs_linked
            + self.orgs_updated
            + self.orgs_deleted
            + self.memberships_added
            + self.memberships_changed
            + self.memberships_removed
            > 0
    }
}

/// How a sync runs.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Write to WorkOS; off reports only.
    pub apply: bool,
    /// Mark new Users and Organizations as test records (`test=1`), for
    /// runs with `X-OpenAgents-Test: 1`.
    pub test: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            apply: true,
            test: false,
        }
    }
}

/// Bring one account's User, and every organization workspace it is an
/// active member of, in line with our stores. A deleted account's User is
/// deleted.
pub async fn account(
    dir: &Path,
    workos: &Workos,
    account: &str,
    options: Options,
) -> Result<Report, Refusal> {
    let store = load(dir)?;
    let mut report = Report::default();
    user(dir, workos, &store, account, options, &mut report).await?;
    for workspace in store.workspaces.values().filter(|workspace| {
        workspace.kind == WorkspaceKind::Organization
            && workspace
                .members
                .get(account)
                .is_some_and(|member| member.status == MemberStatus::Active)
    }) {
        report.add(Box::pin(self::workspace(dir, workos, &workspace.id, options)).await?);
    }
    Ok(report)
}

/// Bring one organization workspace's Organization and memberships in
/// line. A deleted workspace's Organization is deleted.
pub async fn workspace(
    dir: &Path,
    workos: &Workos,
    workspace: &str,
    options: Options,
) -> Result<Report, Refusal> {
    let store = load(dir)?;
    let links = Links::open(dir);
    let mut report = Report::default();
    let Some(record) = store
        .workspaces
        .get(workspace)
        .filter(|record| !deleted_workspace(&store, record))
    else {
        if let Some(org) = links.org_of(workspace).map_err(unavailable)? {
            report.orgs_deleted += 1;
            if options.apply {
                workos.delete_organization(&org).await?;
                links.unlink_workspace(workspace).map_err(unavailable)?;
            }
        }
        return Ok(report);
    };
    if record.kind != WorkspaceKind::Organization {
        report.skipped.push(format!("{workspace}: personal"));
        return Ok(report);
    }
    // The Organization: linked, else by external ID, else new.
    let org = match links.org_of(workspace).map_err(unavailable)? {
        Some(org) => Some(org),
        None => match workos.organization_by_external_id(workspace).await? {
            Some(found) => {
                report.orgs_linked += 1;
                if options.apply {
                    links.link_org(workspace, &found.id).map_err(unavailable)?;
                }
                if found.name != record.name {
                    report.orgs_updated += 1;
                    if options.apply {
                        workos.rename_organization(&found.id, &record.name).await?;
                    }
                }
                Some(found.id)
            }
            None => {
                report.orgs_created += 1;
                if options.apply {
                    let made = workos.create_organization(&record.name, workspace).await?;
                    links.link_org(workspace, &made.id).map_err(unavailable)?;
                    Some(made.id)
                } else {
                    None
                }
            }
        },
    };
    // Everyone active needs a User before they can be a member.
    let mut wanted: BTreeMap<String, String> = BTreeMap::new();
    for member in record.members.values().filter(|member| {
        member.status == MemberStatus::Active
            && store
                .accounts
                .get(&member.account)
                .is_some_and(|account| !deleted(account))
    }) {
        let mut inner = Report::default();
        user(dir, workos, &store, &member.account, options, &mut inner).await?;
        report.add(inner);
        if let Some(user_id) = links.user_of(&member.account).map_err(unavailable)? {
            wanted.insert(user_id, workos.role_slug(member.role).await);
        } else if options.apply {
            report.skipped.push(format!("{}: no user", member.account));
        }
    }
    let Some(org) = org else {
        report.memberships_added += u32::try_from(wanted.len()).unwrap_or(u32::MAX);
        return Ok(report);
    };
    let have = workos.memberships(&org).await?;
    let mut seen: HashMap<&str, ()> = HashMap::new();
    for membership in &have {
        seen.insert(membership.user_id.as_str(), ());
        match wanted.get(&membership.user_id) {
            None => {
                report.memberships_removed += 1;
                if options.apply {
                    workos.remove_membership(&membership.id).await?;
                }
            }
            Some(role) if *role != membership.role => {
                report.memberships_changed += 1;
                if options.apply {
                    workos.set_membership_role(&membership.id, role).await?;
                }
            }
            Some(_) => {}
        }
    }
    for (user_id, role) in &wanted {
        if seen.contains_key(user_id.as_str()) {
            continue;
        }
        report.memberships_added += 1;
        if options.apply {
            workos.add_membership(user_id, &org, role).await?;
        }
    }
    Ok(report)
}

/// Bring one account's User in line.
async fn user(
    dir: &Path,
    workos: &Workos,
    store: &Store,
    account: &str,
    options: Options,
    report: &mut Report,
) -> Result<(), Refusal> {
    let links = Links::open(dir);
    let linked = links.user_of(account).map_err(unavailable)?;
    let Some(record) = store
        .accounts
        .get(account)
        .filter(|record| !deleted(record))
    else {
        // Deleted here: delete its User, when it is still ours.
        let user = match &linked {
            Some(id) => workos.user(id).await?,
            None => workos.user_by_external_id(account).await?,
        };
        if let Some(user) = user.filter(|user| user.external_id.as_deref() == Some(account)) {
            report.users_deleted += 1;
            if options.apply {
                workos.delete_user(&user.id).await?;
            }
        }
        if options.apply && linked.is_some() {
            links.unlink_account(account).map_err(unavailable)?;
        }
        return Ok(());
    };
    if record.agent.is_some() {
        report.skipped.push(format!("{account}: agent"));
        return Ok(());
    }
    let npub = tenancy::accounts::nostr::keys_of(record)
        .first()
        .and_then(|hex| {
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .filter_map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
                .collect();
            <[u8; 32]>::try_from(bytes).ok()
        })
        .map(|key| nostr::nip19::encode_npub(&key));
    let mut metadata = HashMap::new();
    if let Some(npub) = &npub {
        metadata.insert("npub".to_string(), npub.clone());
    }
    if options.test {
        metadata.insert("test".to_string(), "1".to_string());
    }
    // A linked User: keep its email (the person may have set a real one),
    // and make sure it names this account and its primary key.
    if let Some(id) = linked {
        match workos.user(&id).await? {
            Some(user) => {
                let stale_npub = npub.is_some() && user.metadata.get("npub") != npub.as_ref();
                let stale_id = user.external_id.as_deref() != Some(account);
                if stale_npub || stale_id {
                    report.users_updated += 1;
                    if options.apply {
                        let mut merged = user.metadata.clone();
                        merged.extend(metadata);
                        workos
                            .update_user(
                                &user.id,
                                UserChange {
                                    external_id: Some(account.to_string()),
                                    metadata: Some(merged),
                                    ..UserChange::default()
                                },
                            )
                            .await?;
                    }
                }
                return Ok(());
            }
            None => {
                // Deleted in WorkOS: forget the link and make it again below.
                if options.apply {
                    links.unlink_account(account).map_err(unavailable)?;
                }
            }
        }
    }
    // Not linked: by external ID, then by the account's verified email.
    let verified = store
        .identities
        .github_of(account)
        .and_then(|identity| identity.profile.verified_email())
        .map(str::to_string);
    let found = match workos.user_by_external_id(account).await? {
        Some(user) => Some(user),
        None => match &verified {
            Some(email) => workos.users_by_email(email).await?.into_iter().next(),
            None => None,
        },
    };
    if let Some(user) = found {
        if links
            .account_of(&user.id)
            .map_err(unavailable)?
            .is_some_and(|other| other != account)
        {
            report.failed.push(format!(
                "{account}: its WorkOS user belongs to another account"
            ));
            return Ok(());
        }
        report.users_linked += 1;
        if options.apply {
            links.link_user(account, &user.id).map_err(unavailable)?;
            if user.external_id.as_deref() != Some(account)
                && user
                    .external_id
                    .as_deref()
                    .is_none_or(|id| id.starts_with("acct_"))
            {
                let mut merged = user.metadata.clone();
                merged.extend(metadata);
                workos
                    .update_user(
                        &user.id,
                        UserChange {
                            external_id: Some(account.to_string()),
                            metadata: Some(merged),
                            ..UserChange::default()
                        },
                    )
                    .await?;
            }
        }
        return Ok(());
    }
    report.users_created += 1;
    if options.apply {
        let email = verified.unwrap_or_else(|| super::synthetic_email(account));
        let made = workos
            .create_user(NewUser {
                email,
                email_verified: Some(true),
                external_id: Some(account.to_string()),
                first_name: None,
                last_name: None,
                password: None,
                metadata,
            })
            .await?;
        links.link_user(account, &made.id).map_err(unavailable)?;
    }
    Ok(())
}

/// Every account and organization workspace, in ID order, starting after
/// `after` (to resume). `pause` runs between accounts, to keep under the
/// WorkOS write limit (500 a 10 seconds).
pub async fn all<F, Fut>(
    dir: &Path,
    workos: &Workos,
    options: Options,
    after: Option<&str>,
    mut pause: F,
    mut progress: impl FnMut(&str, &Report),
) -> Result<Report, Refusal>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let store = load(dir)?;
    let mut total = Report::default();
    for id in store
        .accounts
        .iter()
        .filter(|(_, record)| !deleted(record))
        .map(|(id, _)| id)
        .filter(|id| after.is_none_or(|after| id.as_str() > after))
    {
        let mut report = Report::default();
        match user(dir, workos, &store, id, options, &mut report).await {
            Ok(()) => {}
            Err(Refusal::RateLimited(seconds)) => {
                tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
                report = Report::default();
                if let Err(refusal) = user(dir, workos, &store, id, options, &mut report).await {
                    report.failed.push(format!("{id}: {}", refusal.code()));
                }
            }
            Err(refusal) => report.failed.push(format!("{id}: {}", refusal.code())),
        }
        progress(id, &report);
        total.add(report);
        pause().await;
    }
    for id in store
        .workspaces
        .values()
        .filter(|workspace| workspace.kind == WorkspaceKind::Organization)
        .map(|workspace| workspace.id.clone())
    {
        let report = match workspace(dir, workos, &id, options).await {
            Ok(report) => report,
            Err(refusal) => Report {
                failed: vec![format!("{id}: {}", refusal.code())],
                ..Report::default()
            },
        };
        progress(&id, &report);
        total.add(report);
        pause().await;
    }
    Ok(total)
}

/// The reconcile report: counts, and the accounts with no WorkOS User.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Reconcile {
    pub accounts: usize,
    pub agent_accounts: usize,
    pub linked_accounts: usize,
    pub organization_workspaces: usize,
    pub linked_workspaces: usize,
    /// Accounts (people, not agents) with no linked User.
    pub accounts_without_user: Vec<String>,
    /// Links to a missing or deleted account.
    pub links_without_account: Vec<String>,
}

/// Count what is linked, from our side only (no WorkOS calls).
pub fn reconcile(dir: &Path) -> Result<Reconcile, Refusal> {
    let store = load(dir)?;
    let links = Links::open(dir);
    let users: BTreeMap<String, _> = links.users().map_err(unavailable)?.into_iter().collect();
    let orgs: BTreeMap<String, _> = links.orgs().map_err(unavailable)?.into_iter().collect();
    let mut out = Reconcile {
        accounts: store
            .accounts
            .values()
            .filter(|account| !deleted(account))
            .count(),
        ..Reconcile::default()
    };
    for (id, account) in &store.accounts {
        if deleted(account) {
            continue;
        }
        if account.agent.is_some() {
            out.agent_accounts += 1;
        } else if users.contains_key(id) {
            out.linked_accounts += 1;
        } else {
            out.accounts_without_user.push(id.clone());
        }
    }
    out.links_without_account = users
        .keys()
        .filter(|id| store.accounts.get(*id).is_none_or(deleted))
        .cloned()
        .collect();
    for workspace in store.workspaces.values().filter(|workspace| {
        workspace.kind == WorkspaceKind::Organization && !deleted_workspace(&store, workspace)
    }) {
        out.organization_workspaces += 1;
        if orgs.contains_key(&workspace.id) {
            out.linked_workspaces += 1;
        }
    }
    Ok(out)
}

// Deletion retains the row; an empty principal list alone also occurs on new accounts.
fn deleted(account: &Account) -> bool {
    account.label == DELETED_LABEL && account.principals.is_empty()
}

fn deleted_workspace(store: &Store, workspace: &Workspace) -> bool {
    workspace.kind == WorkspaceKind::Organization
        && workspace
            .members
            .values()
            .filter(|member| member.status == MemberStatus::Active)
            .all(|member| store.accounts.get(&member.account).is_none_or(deleted))
}

fn load(dir: &Path) -> Result<Store, Refusal> {
    Accounts::open(dir)
        .and_then(|accounts| accounts.store())
        .map_err(|_| Refusal::Unavailable)
}

fn unavailable(_: tenancy::workos_links::LinkError) -> Refusal {
    Refusal::Unavailable
}
