//! One claim record for a GitHub issue, which every path writes and reads
//! (#10203): the chat issue flow ([`crate::task::issue_run`]),
//! `coder-project`, and `openagents issue claim|release`.
//!
//! A claim is three marks, written in this order, each attempted even
//! when an earlier one failed:
//!
//! 1. a comment carrying [`CLAIM_MARK`] (every path, always);
//! 2. the signed-in GitHub user as an assignee;
//! 3. on each GitHub Project the issue is on that has the configured
//!    status field, the "in progress" value.
//!
//! Releasing comments [`RELEASE_MARK`], removes that assignee, and moves
//! the status back to the first "ready" value the project has; landing
//! moves it to "done", and [`blocked`] to "blocked" with the reason as a
//! comment (#11108). When the GraphQL limit is spent, [`Gh`] finds and
//! moves the issue's items through the REST projects API instead. An issue on no project gets the comment and the
//! assignee only, so a repository without Projects behaves the same way
//! minus the board.
//!
//! Reading a claim ([`held`]): the latest claim comment no later release
//! answered, within the claim window, or a project status of "in
//! progress" set within the window and after the latest release. Field
//! and value names come from the `project` object of
//! `.openagents/coder-issues.json` ([`Project`]); names match without
//! regard to case, so "In progress" finds a project's "In Progress".
//!
//! Choosing what to pick up ([`pickup`]) uses the repository's project
//! when it has one: its item order, a "ready" status, and no open
//! `blockedBy`; without one, a label (`coder-sized`) in issue order.
//!
//! A claim is also held on this computer by one agent session
//! ([`take`], [`give_back`], #10764): an `issue/<n>` record under the
//! lease root ([`coder_lease::claims`]) that refuses another live
//! session's claim, and a `session=` field in the claim marker
//! ([`marker`]), so a fresh marker from another session refuses too.
//! Markers without the field, written before it, still parse.
//!
//! GitHub is reached through the `gh` CLI the person is signed in to
//! ([`Gh`]); nothing here reads, stores, or prints a token.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Marks a claim comment.
pub const CLAIM_MARK: &str = "<!-- openagents-coder-claim";
/// Marks a comment that releases a claim, so another may take the issue.
pub const RELEASE_MARK: &str = "<!-- openagents-coder-release -->";
/// The label pickup reads when the repository has no project.
pub const PICKUP_LABEL: &str = "coder-sized";

/// One comment on an issue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub body: String,
    /// Unix seconds.
    pub at: u64,
}

/// The project names a repository's claims use, from the `project`
/// object of `.openagents/coder-issues.json`. Every field is optional.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Project {
    /// The single-select field a claim moves.
    pub field: String,
    /// The value a claim sets.
    pub in_progress: String,
    /// The values that mean "ready to pick up", in preference order; a
    /// release sets the first one the project has.
    pub ready: Vec<String>,
    /// The value a landed issue gets.
    pub done: String,
    /// The value an issue waiting on the owner or another issue gets.
    pub blocked: String,
    /// The project (by number, under the repository's owner) pickup
    /// orders by. Without it, the one open project linked to the
    /// repository, when there is exactly one.
    pub number: Option<u64>,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            field: "Status".into(),
            in_progress: "In progress".into(),
            ready: vec!["Ready".into(), "Todo".into()],
            done: "Done".into(),
            blocked: "Blocked".into(),
            number: None,
        }
    }
}

/// The issue as an item of one project.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// The project's node id.
    pub project: String,
    pub project_title: String,
    /// The item's node id.
    pub item: String,
    /// The status field's node id, when the project has that field.
    pub field: Option<String>,
    /// The field's options: (name, id).
    pub options: Vec<(String, String)>,
    /// The item's status, when set.
    pub status: Option<String>,
    /// When the status was last set, Unix seconds (0 when unknown).
    pub status_at: u64,
}

impl Item {
    fn option(&self, name: &str) -> Option<&(String, String)> {
        self.options
            .iter()
            .find(|(option, _)| option.eq_ignore_ascii_case(name))
    }
    fn is(&self, name: &str) -> bool {
        self.status
            .as_deref()
            .is_some_and(|status| status.eq_ignore_ascii_case(name))
    }
}

/// One issue of the repository's project, as pickup reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queued {
    pub number: u64,
    pub open: bool,
    pub status: Option<String>,
    pub labels: Vec<String>,
    /// An open blocker, or a blocker list the fetch could not finish.
    pub blocked: bool,
}

/// What a claim asks of GitHub. [`Gh`] is the real one; tests fake it.
pub trait Hub: Send + Sync {
    /// # Errors
    /// Why the comment was not posted.
    fn comment(&self, repository: &str, number: u64, body: &str) -> Result<(), String>;
    /// The issue's comments, oldest first.
    ///
    /// # Errors
    /// Why they cannot be read.
    fn comments(&self, repository: &str, number: u64) -> Result<Vec<Comment>, String>;
    /// The open issues with `label`, oldest first.
    ///
    /// # Errors
    /// Why they cannot be listed.
    fn labeled(&self, repository: &str, label: &str) -> Result<Vec<u64>, String>;
    /// The signed-in GitHub user's login.
    ///
    /// # Errors
    /// Why it cannot tell; the claim then sets no assignee.
    fn viewer(&self) -> Result<String, String> {
        Err("this tracker has no signed-in user".into())
    }
    /// Adds (`add`) or removes `login` as an assignee.
    ///
    /// # Errors
    /// Why it did not.
    fn assign(&self, repository: &str, number: u64, login: &str, add: bool) -> Result<(), String> {
        let _ = (repository, number, login, add);
        Ok(())
    }
    /// The issue's items on open projects, with `field`'s value.
    ///
    /// # Errors
    /// Why they cannot be read.
    fn items(&self, repository: &str, number: u64, field: &str) -> Result<Vec<Item>, String> {
        let _ = (repository, number, field);
        Ok(Vec::new())
    }
    /// Sets the item's status field to the option `option` (an id).
    ///
    /// # Errors
    /// Why it did not.
    fn set_status(&self, item: &Item, option: &str) -> Result<(), String> {
        let _ = (item, option);
        Ok(())
    }
    /// Adds the issue to `project`'s numbered project. `Ok(false)` when
    /// there is no numbered project to add it to.
    ///
    /// # Errors
    /// Why it was not added.
    fn add_item(&self, repository: &str, number: u64, project: &Project) -> Result<bool, String> {
        let _ = (repository, number, project);
        Ok(false)
    }
    /// The repository's project issues in project order, or `None` when
    /// the repository has no project to order by.
    ///
    /// # Errors
    /// Why the project cannot be read.
    fn board(&self, repository: &str, project: &Project) -> Result<Option<Vec<Queued>>, String> {
        let _ = (repository, project);
        Ok(None)
    }
}

/// The latest claim comment no later release answered.
#[must_use]
pub fn active(comments: &[Comment]) -> Option<&Comment> {
    let mut claim = None;
    for comment in comments {
        let body = comment.body.trim_start();
        if body.contains(CLAIM_MARK)
            || body
                .get(..7)
                .is_some_and(|head| head.eq_ignore_ascii_case("claimed"))
        {
            claim = Some(comment);
        } else if body.contains(RELEASE_MARK) {
            claim = None;
        }
    }
    claim
}

/// Why `number` is claimed, or `None`: a claim comment within `hours`
/// that no later release answered, or an "in progress" project status
/// set within `hours` and after the latest release comment. A claim
/// comment is one that starts with "Claimed" (the workspace convention)
/// or carries [`CLAIM_MARK`].
#[must_use]
pub fn held(
    number: u64,
    comments: &[Comment],
    items: &[Item],
    now: u64,
    hours: u64,
    project: &Project,
) -> Option<String> {
    let window = hours * 3_600;
    if let Some(claim) = active(comments) {
        let age = now.saturating_sub(claim.at);
        if age < window {
            return Some(format!(
                "#{number} was claimed {} ago: \"{}\"",
                ago(age),
                clip(claim.body.lines().next().unwrap_or("").trim(), 120)
            ));
        }
    }
    let released = comments
        .iter()
        .filter(|comment| comment.body.contains(RELEASE_MARK))
        .map(|comment| comment.at)
        .max()
        .unwrap_or(0);
    items
        .iter()
        .find(|item| {
            item.is(&project.in_progress)
                && item.status_at > released
                && now.saturating_sub(item.status_at) < window
        })
        .map(|item| {
            format!(
                "#{number} is \"{}\" on the project \"{}\" (set {} ago)",
                item.status.as_deref().unwrap_or_default(),
                item.project_title,
                ago(now.saturating_sub(item.status_at))
            )
        })
}

/// Writes a claim: `body` as a comment (it should carry [`CLAIM_MARK`]),
/// the signed-in user as assignee, and "in progress" on each project the
/// issue is on. Returns what each step did, a sentence each; a failed
/// step is said, never fatal.
pub fn claim<H: Hub + ?Sized>(
    hub: &H,
    repository: &str,
    number: u64,
    body: &str,
    project: &Project,
) -> Vec<String> {
    let mut said = vec![match hub.comment(repository, number, body) {
        Ok(()) => format!("Claimed #{number} with a comment on the issue."),
        Err(why) => format!("Could not post the claim comment on #{number}: {why}"),
    }];
    assignee(hub, repository, number, true, &mut said);
    let target = project.in_progress.clone();
    status(hub, repository, number, project, &[target], &mut said);
    said
}

/// Releases a claim: `comment` when given (it should carry
/// [`RELEASE_MARK`]), the signed-in user off the assignees, and the first
/// "ready" value on each project the issue is on.
pub fn release<H: Hub + ?Sized>(
    hub: &H,
    repository: &str,
    number: u64,
    comment: Option<&str>,
    project: &Project,
) -> Vec<String> {
    let mut said = Vec::new();
    if let Some(body) = comment {
        said.push(match hub.comment(repository, number, body) {
            Ok(()) => format!("Released #{number} with a comment on the issue."),
            Err(why) => format!("Could not post the release comment on #{number}: {why}"),
        });
    }
    assignee(hub, repository, number, false, &mut said);
    status(hub, repository, number, project, &project.ready, &mut said);
    said
}

/// Marks a landed issue "done" on each project it is on. The assignee
/// stays, as the record of who did it.
pub fn done<H: Hub + ?Sized>(
    hub: &H,
    repository: &str,
    number: u64,
    project: &Project,
) -> Vec<String> {
    let mut said = Vec::new();
    status(
        hub,
        repository,
        number,
        project,
        std::slice::from_ref(&project.done),
        &mut said,
    );
    said
}

/// Marks an issue that waits on the owner or another issue: `reason` as a
/// "Blocked: ..." comment, and "blocked" on each project it is on. The
/// claim and assignee stay, so nobody else picks it up meanwhile.
pub fn blocked<H: Hub + ?Sized>(
    hub: &H,
    repository: &str,
    number: u64,
    reason: &str,
    project: &Project,
) -> Vec<String> {
    let mut said = vec![
        match hub.comment(repository, number, &format!("Blocked: {reason}")) {
            Ok(()) => format!("Said why #{number} is blocked on the issue."),
            Err(why) => format!("Could not comment why #{number} is blocked: {why}"),
        },
    ];
    status(
        hub,
        repository,
        number,
        project,
        std::slice::from_ref(&project.blocked),
        &mut said,
    );
    said
}

fn assignee<H: Hub + ?Sized>(
    hub: &H,
    repository: &str,
    number: u64,
    add: bool,
    said: &mut Vec<String>,
) {
    match hub.viewer() {
        Ok(login) => said.push(match hub.assign(repository, number, &login, add) {
            Ok(()) if add => format!("Assigned #{number} to @{login}."),
            Ok(()) => format!("Removed @{login} from #{number}'s assignees."),
            Err(why) => format!("Could not change #{number}'s assignee @{login}: {why}"),
        }),
        Err(why) => said.push(format!("No assignee change on #{number}: {why}")),
    }
}

/// The repositories whose projects this process could not read; each is
/// said once, not on every claim, release and close.
static UNREADABLE: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// What to say the first time `repository`'s projects cannot be read
/// (`why`), or `None` when this process already said it. A token without
/// the `project` scope reads like this: the flow goes on with comments
/// and assignees, and the project status stays as it was.
#[must_use]
pub fn unreadable_projects(repository: &str, why: &str) -> Option<String> {
    let mut said = UNREADABLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if said.iter().any(|seen| seen == repository) {
        return None;
    }
    said.push(repository.to_owned());
    let scope = if why.contains("project") || why.contains("INSUFFICIENT_SCOPES") {
        " The GitHub token needs the `project` scope (`read:project` to read it)."
    } else {
        ""
    };
    Some(format!(
        "Could not read {repository}'s project board, so issue statuses there stay as they are: {why}.{scope} Claims go on with comments and assignees."
    ))
}

/// Moves the issue's status to the first of `targets` each project has.
fn status<H: Hub + ?Sized>(
    hub: &H,
    repository: &str,
    number: u64,
    project: &Project,
    targets: &[String],
    said: &mut Vec<String>,
) {
    let mut items = match hub.items(repository, number, &project.field) {
        Ok(items) => items,
        Err(why) => {
            if let Some(line) = unreadable_projects(repository, &why) {
                said.push(line);
            }
            return;
        }
    };
    // A configured project should hold every issue Coder works: add one
    // that is not on any open project yet, then move it like the rest.
    if items.is_empty() && project.number.is_some() {
        match hub.add_item(repository, number, project) {
            Ok(true) => match hub.items(repository, number, &project.field) {
                Ok(added) => items = added,
                Err(why) => said.push(format!(
                    "Added #{number} to the project, but could not read it back: {why}"
                )),
            },
            Ok(false) => {}
            Err(why) => said.push(format!("Could not add #{number} to the project: {why}")),
        }
    }
    for item in items {
        if item.field.is_none() {
            continue;
        }
        let Some((name, id)) = targets.iter().find_map(|target| item.option(target)) else {
            said.push(format!(
                "The project \"{}\" has no {} value {}; its status stays.",
                item.project_title,
                project.field,
                targets
                    .iter()
                    .map(|target| format!("\"{target}\""))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
            continue;
        };
        if item.is(name) {
            continue;
        }
        said.push(match hub.set_status(&item, id) {
            Ok(()) => format!(
                "Moved #{number} to \"{name}\" on the project \"{}\".",
                item.project_title
            ),
            Err(why) => format!(
                "Could not move #{number} to \"{name}\" on the project \"{}\": {why}",
                item.project_title
            ),
        });
    }
}

/// What to pick up next, in order, and where the order came from. With a
/// project ([`Hub::board`]): its open issues in project order whose
/// status is a "ready" value and that no open issue blocks; when `label`
/// is given, only those with it, followed by labeled issues the project
/// does not hold. Without one: the issues labeled `label` (or
/// [`PICKUP_LABEL`]), oldest first.
///
/// # Errors
/// Neither the project nor the label can be read.
pub fn pickup<H: Hub + ?Sized>(
    hub: &H,
    repository: &str,
    project: &Project,
    label: Option<&str>,
) -> Result<(Vec<u64>, String), String> {
    let board = match hub.board(repository, project) {
        Ok(board) => board,
        Err(_) if label.is_some() => None,
        Err(why) => return Err(why),
    };
    let Some(board) = board else {
        let label = label.unwrap_or(PICKUP_LABEL);
        return Ok((
            hub.labeled(repository, label)?,
            format!("issues labeled `{label}`, oldest first"),
        ));
    };
    let ready = |item: &Queued| {
        item.open
            && !item.blocked
            && item.status.as_deref().is_some_and(|status| {
                project
                    .ready
                    .iter()
                    .any(|ready| ready.eq_ignore_ascii_case(status))
            })
            && label.is_none_or(|label| {
                item.labels
                    .iter()
                    .any(|have| have.eq_ignore_ascii_case(label))
            })
    };
    let mut numbers: Vec<u64> = board
        .iter()
        .filter(|item| ready(item))
        .map(|item| item.number)
        .collect();
    if let Some(label) = label {
        for number in hub.labeled(repository, label)? {
            if !board.iter().any(|item| item.number == number) && !numbers.contains(&number) {
                numbers.push(number);
            }
        }
    }
    Ok((
        numbers,
        format!(
            "the project's order, status {}, and blockedBy",
            project.ready.join("/")
        ),
    ))
}

/// The value of `key` in the claim marker of `body`, such as `task` in
/// `<!-- openagents-coder-claim task=abc session=s1 -->`. `None` when the
/// body has no marker or the marker no such field.
#[must_use]
pub fn marker_field<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let rest = body.split_once(CLAIM_MARK)?.1;
    let fields = rest.split_once("-->").map_or(rest, |(fields, _)| fields);
    fields
        .split_whitespace()
        .find_map(|word| word.strip_prefix(key)?.strip_prefix('='))
}

/// The session as a marker field: characters other than letters, digits,
/// `:`, `.`, `_`, and `-` become `_`, so it ends no comment.
#[must_use]
pub fn session_word(session: &str) -> String {
    session
        .chars()
        .take(128)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// A claim marker with `fields` (such as `cli` or `task=abc`) and the
/// claiming session.
#[must_use]
pub fn marker(fields: &str, session: &str) -> String {
    let fields = fields.trim();
    let space = if fields.is_empty() { "" } else { " " };
    format!(
        "{CLAIM_MARK}{space}{fields} session={} -->",
        session_word(session)
    )
}

/// Why a claim comment on GitHub refuses `session`'s claim, or `None`:
/// the latest unreleased claim is younger than `hours` and comes from
/// another session, or from no named session (a marker written before
/// the field, or a "Claimed" comment). `ended` is a session this
/// computer saw end, whose marker no longer counts.
#[must_use]
pub fn foreign_marker(
    number: u64,
    comments: &[Comment],
    now: u64,
    hours: u64,
    session: &str,
    ended: Option<&str>,
) -> Option<String> {
    let claim = active(comments)?;
    let age = now.saturating_sub(claim.at);
    if age >= hours * 3_600 {
        return None;
    }
    let ours = session_word(session);
    match marker_field(&claim.body, "session") {
        Some(theirs) if theirs == ours => None,
        Some(theirs) if ended.is_some_and(|ended| session_word(ended) == theirs) => None,
        Some(theirs) => Some(format!(
            "#{number} has a claim comment from session {theirs}, {} ago",
            ago(age)
        )),
        None => Some(format!(
            "#{number} has a claim comment that names no session, {} ago",
            ago(age)
        )),
    }
}

/// Claims `number` for `claimant`: refuses when another live session holds
/// it on this computer or a fresh claim comment from another session is
/// on the issue, unless `force`; then holds it under `lease_root` and
/// writes the claim ([`claim`]) with `body`, which should carry
/// [`marker`] with the claimant's session. `now` is in Unix seconds and
/// `hours` the claim window.
///
/// # Errors
/// Why the claim was refused.
#[allow(clippy::too_many_arguments)]
pub fn take<H: Hub + ?Sized>(
    hub: &H,
    lease_root: &std::path::Path,
    repository: &str,
    number: u64,
    body: &str,
    project: &Project,
    claimant: &coder_lease::claims::Claimant,
    now: u64,
    hours: u64,
    force: bool,
) -> Result<Vec<String>, String> {
    use coder_lease::claims::{self, Claimed};
    let veto = |replaced: Option<&claims::IssueClaim>| {
        let comments = hub.comments(repository, number).ok()?;
        foreign_marker(
            number,
            &comments,
            now,
            hours,
            &claimant.session,
            replaced.map(|claim| claim.session.as_str()),
        )
    };
    let taken = claims::claim(
        lease_root,
        repository,
        number,
        claimant,
        now.saturating_mul(1_000),
        std::time::Duration::from_secs(hours * 3_600),
        force,
        &veto,
    )
    .map_err(|refused| match refused {
        claims::Refused::Broker(error) => format!("The claim could not be recorded: {error}."),
        refused => format!("{refused}. Leave it, or pass --force to take it anyway."),
    })?;
    let session = &claimant.session;
    let mut said = vec![match taken {
        Claimed::New => format!("Holding #{number} for session {session}."),
        Claimed::Renewed => format!("Renewed session {session}'s hold on #{number}."),
        Claimed::TookOver(previous) => format!(
            "Took #{number} over from session {}, whose claim ended or was overridden.",
            previous.session
        ),
    }];
    said.extend(claim(hub, repository, number, body, project));
    Ok(said)
}

/// Releases `number` for `session`: drops this computer's hold when the
/// session holds it or its holder ended (or under `force`), then releases
/// the claim ([`release`]).
///
/// # Errors
/// Another live session holds it.
#[allow(clippy::too_many_arguments)]
pub fn give_back<H: Hub + ?Sized>(
    hub: &H,
    lease_root: &std::path::Path,
    repository: &str,
    number: u64,
    comment: Option<&str>,
    project: &Project,
    session: &str,
    now: u64,
    hours: u64,
    force: bool,
) -> Result<Vec<String>, String> {
    use coder_lease::claims;
    let dropped = claims::release(
        lease_root,
        repository,
        number,
        session,
        now.saturating_mul(1_000),
        std::time::Duration::from_secs(hours * 3_600),
        force,
    )
    .map_err(|refused| match refused {
        claims::Refused::Broker(error) => format!("The hold could not be released: {error}."),
        refused => format!("{refused}. Pass --force to release it anyway."),
    })?;
    let mut said = Vec::new();
    if let Some(dropped) = dropped {
        said.push(format!(
            "Dropped session {}'s hold on #{number}.",
            dropped.session
        ));
    }
    said.extend(release(hub, repository, number, comment, project));
    Ok(said)
}

fn ago(seconds: u64) -> String {
    match seconds {
        0..=119 => format!("{seconds} seconds"),
        120..=7_199 => format!("{} minutes", seconds / 60),
        _ => format!("{} hours", seconds / 3_600),
    }
}

fn clip(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Unix seconds of an ISO time such as `2026-09-30T12:00:00Z`.
#[must_use]
pub fn iso_seconds(text: &str) -> Option<u64> {
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hour * 3_600 + minute * 60 + second).ok()
}

// ---------------------------------------------------------------------------
// GitHub through `gh`.

/// GitHub through the `gh` CLI the person is signed in to.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gh;

/// Runs `gh` with `args`, in `dir` when given.
///
/// # Errors
/// `gh` is missing or failed; the sentence quotes its error.
pub fn gh(dir: Option<&std::path::Path>, args: &[&str]) -> Result<String, String> {
    let mut command = std::process::Command::new("gh");
    command
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null());
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let output = command.output().map_err(|_| {
        "cannot run gh; install the GitHub CLI and sign in with `gh auth login`".to_owned()
    })?;
    if !output.status.success() {
        let why = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "gh {}: {}",
            args[..2.min(args.len())].join(" "),
            clip(why.trim(), 400)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn json(text: &str) -> Result<Value, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| format!("unexpected gh output: {error}"))?;
    if let Some(errors) = value.get("errors").and_then(Value::as_array)
        && !errors.is_empty()
    {
        return Err(errors
            .iter()
            .filter_map(|error| error["message"].as_str())
            .collect::<Vec<_>>()
            .join("; "));
    }
    Ok(value)
}

fn split(repository: &str) -> Result<(&str, &str), String> {
    repository
        .split_once('/')
        .ok_or_else(|| format!("`{repository}` is not owner/name"))
}

const ITEMS_QUERY: &str = r"query($owner: String!, $name: String!, $number: Int!, $field: String!) {
  repository(owner: $owner, name: $name) {
    issue(number: $number) {
      projectItems(first: 20) {
        nodes {
          id
          project {
            id title closed
            field(name: $field) { ... on ProjectV2SingleSelectField { id options { id name } } }
          }
          fieldValueByName(name: $field) {
            ... on ProjectV2ItemFieldSingleSelectValue { name updatedAt }
          }
        }
      }
    }
  }
}";

const SET_STATUS: &str = r"mutation($project: ID!, $item: ID!, $field: ID!, $option: String!) {
  updateProjectV2ItemFieldValue(input: {projectId: $project, itemId: $item, fieldId: $field,
    value: {singleSelectOptionId: $option}}) { projectV2Item { id } }
}";

const ISSUE_ID: &str = r"query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) { issue(number: $number) { id } }
}";

const ADD_ITEM: &str = r"mutation($project: ID!, $content: ID!) {
  addProjectV2ItemById(input: {projectId: $project, contentId: $content}) { item { id } }
}";

const LINKED: &str = r"query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) { projectsV2(first: 20) { nodes { id closed } } }
}";

const NUMBERED: &str = r"query($owner: String!, $number: Int!) {
  repositoryOwner(login: $owner) {
    ... on Organization { projectV2(number: $number) { id } }
    ... on User { projectV2(number: $number) { id } }
  }
}";

const BOARD: &str = r"query($id: ID!, $field: String!, $cursor: String) {
  node(id: $id) {
    ... on ProjectV2 {
      items(first: 100, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        nodes {
          fieldValueByName(name: $field) { ... on ProjectV2ItemFieldSingleSelectValue { name } }
          content {
            __typename
            ... on Issue {
              number state
              repository { nameWithOwner }
              labels(first: 20) { nodes { name } }
              blockedBy(first: 20) { pageInfo { hasNextPage } nodes { state } }
            }
          }
        }
      }
    }
  }
}";

/// The most project pages pickup reads.
const BOARD_PAGES: usize = 10;

impl Gh {
    fn graphql_items(repository: &str, number: u64, field: &str) -> Result<Vec<Item>, String> {
        let (owner, name) = split(repository)?;
        let value = json(&gh(
            None,
            &[
                "api",
                "graphql",
                "-f",
                &format!("query={ITEMS_QUERY}"),
                "-f",
                &format!("owner={owner}"),
                "-f",
                &format!("name={name}"),
                "-F",
                &format!("number={number}"),
                "-f",
                &format!("field={field}"),
            ],
        )?)?;
        Ok(items_of(&value))
    }

    /// The issue's items over REST, for when the GraphQL limit is spent:
    /// each open project of the owner whose item search finds the issue,
    /// with `field`'s value. One search per project, two more on a hit.
    fn rest_items(repository: &str, number: u64, field: &str) -> Result<Vec<Item>, String> {
        let (owner, _) = split(repository)?;
        let (scope, pages) = ["orgs", "users"]
            .iter()
            .find_map(|scope| {
                let path = format!("/{scope}/{owner}/projectsV2?per_page=100");
                let text = gh(None, &["api", "--paginate", "--slurp", &path]).ok()?;
                Some((*scope, serde_json::from_str::<Value>(&text).ok()?))
            })
            .ok_or_else(|| format!("cannot list {owner}'s projects"))?;
        let projects = pages
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_array)
            .flatten()
            .filter(|project| project["state"].as_str() == Some("open"));
        let mut found = Vec::new();
        for project in projects {
            let Some(project_number) = project["number"].as_u64() else {
                continue;
            };
            let base = format!("/{scope}/{owner}/projectsV2/{project_number}");
            let search = |field_id: Option<&str>| -> Option<Value> {
                let items = format!("{base}/items");
                let q = format!("q={number}");
                let fields = field_id.map(|id| format!("fields[]={id}"));
                let mut args = vec!["api", "-X", "GET", &items, "-f", "per_page=100", "-f", &q];
                if let Some(fields) = &fields {
                    args.extend(["-f", fields]);
                }
                serde_json::from_str(&gh(None, &args).ok()?).ok()
            };
            let Some(items) = search(None) else { continue };
            if rest_item(&items, repository, number, field).is_none() {
                continue;
            }
            let Some((field_id, options)) = gh(None, &["api", &format!("{base}/fields")])
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|fields| rest_field(&fields, field))
            else {
                continue;
            };
            let items = search(Some(&field_id)).unwrap_or(items);
            let Some((item, status, status_at)) = rest_item(&items, repository, number, field)
            else {
                continue;
            };
            found.push(Item {
                project: format!("{REST_PROJECT}{base}"),
                project_title: project["title"].as_str().unwrap_or_default().into(),
                item,
                field: Some(field_id),
                options,
                status,
                status_at,
            });
        }
        Ok(found)
    }

    fn project_id(repository: &str, project: &Project) -> Result<Option<String>, String> {
        let (owner, name) = split(repository)?;
        if let Some(number) = project.number {
            let value = json(&gh(
                None,
                &[
                    "api",
                    "graphql",
                    "-f",
                    &format!("query={NUMBERED}"),
                    "-f",
                    &format!("owner={owner}"),
                    "-F",
                    &format!("number={number}"),
                ],
            )?)?;
            return Ok(value["data"]["repositoryOwner"]["projectV2"]["id"]
                .as_str()
                .map(str::to_owned));
        }
        let value = json(&gh(
            None,
            &[
                "api",
                "graphql",
                "-f",
                &format!("query={LINKED}"),
                "-f",
                &format!("owner={owner}"),
                "-f",
                &format!("name={name}"),
            ],
        )?)?;
        let open: Vec<&str> = value["data"]["repository"]["projectsV2"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|node| node["closed"].as_bool() != Some(true))
            .filter_map(|node| node["id"].as_str())
            .collect();
        Ok(match open.as_slice() {
            [one] => Some((*one).to_owned()),
            _ => None,
        })
    }
}

impl Hub for Gh {
    fn comment(&self, repository: &str, number: u64, body: &str) -> Result<(), String> {
        gh(
            None,
            &[
                "issue",
                "comment",
                &number.to_string(),
                "-R",
                repository,
                "--body",
                body,
            ],
        )
        .map(|_| ())
    }

    fn comments(&self, repository: &str, number: u64) -> Result<Vec<Comment>, String> {
        let value = json(&gh(
            None,
            &[
                "issue",
                "view",
                &number.to_string(),
                "-R",
                repository,
                "--json",
                "comments",
            ],
        )?)?;
        Ok(comments_of(&value))
    }

    fn labeled(&self, repository: &str, label: &str) -> Result<Vec<u64>, String> {
        let value = json(&gh(
            None,
            &[
                "issue", "list", "-R", repository, "--state", "open", "--label", label, "--limit",
                "100", "--json", "number",
            ],
        )?)?;
        let mut numbers: Vec<u64> = value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|issue| issue["number"].as_u64())
            .collect();
        numbers.sort_unstable();
        Ok(numbers)
    }

    fn viewer(&self) -> Result<String, String> {
        let login = gh(None, &["api", "user", "-q", ".login"])?;
        let login = login.trim();
        if login.is_empty() {
            return Err("gh names no signed-in user".into());
        }
        Ok(login.to_owned())
    }

    fn assign(&self, repository: &str, number: u64, login: &str, add: bool) -> Result<(), String> {
        gh(
            None,
            &[
                "api",
                "-X",
                if add { "POST" } else { "DELETE" },
                &format!("repos/{repository}/issues/{number}/assignees"),
                "-f",
                &format!("assignees[]={login}"),
            ],
        )
        .map(|_| ())
    }

    fn items(&self, repository: &str, number: u64, field: &str) -> Result<Vec<Item>, String> {
        Gh::graphql_items(repository, number, field).or_else(|graphql| {
            Gh::rest_items(repository, number, field)
                .map_err(|rest| format!("{graphql} (and over REST: {rest})"))
        })
    }

    fn set_status(&self, item: &Item, option: &str) -> Result<(), String> {
        let field = item
            .field
            .as_deref()
            .ok_or("the project has no status field")?;
        if let Some(base) = item.project.strip_prefix(REST_PROJECT) {
            return gh(
                None,
                &[
                    "api",
                    "-X",
                    "PATCH",
                    &format!("{base}/items/{}", item.item),
                    "-F",
                    &format!("fields[][id]={field}"),
                    "-f",
                    &format!("fields[][value]={option}"),
                ],
            )
            .map(|_| ());
        }
        json(&gh(
            None,
            &[
                "api",
                "graphql",
                "-f",
                &format!("query={SET_STATUS}"),
                "-f",
                &format!("project={}", item.project),
                "-f",
                &format!("item={}", item.item),
                "-f",
                &format!("field={field}"),
                "-f",
                &format!("option={option}"),
            ],
        )?)
        .map(|_| ())
    }

    fn add_item(&self, repository: &str, number: u64, project: &Project) -> Result<bool, String> {
        if project.number.is_none() {
            return Ok(false);
        }
        let Some(project_id) = Gh::project_id(repository, project)? else {
            return Ok(false);
        };
        let (owner, name) = split(repository)?;
        let issue = json(&gh(
            None,
            &[
                "api",
                "graphql",
                "-f",
                &format!("query={ISSUE_ID}"),
                "-f",
                &format!("owner={owner}"),
                "-f",
                &format!("name={name}"),
                "-F",
                &format!("number={number}"),
            ],
        )?)?;
        let content = issue["data"]["repository"]["issue"]["id"]
            .as_str()
            .ok_or("the issue has no id")?
            .to_owned();
        json(&gh(
            None,
            &[
                "api",
                "graphql",
                "-f",
                &format!("query={ADD_ITEM}"),
                "-f",
                &format!("project={project_id}"),
                "-f",
                &format!("content={content}"),
            ],
        )?)
        .map(|_| true)
    }

    fn board(&self, repository: &str, project: &Project) -> Result<Option<Vec<Queued>>, String> {
        let Some(id) = Gh::project_id(repository, project)? else {
            return Ok(None);
        };
        let mut queued = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..BOARD_PAGES {
            let mut args = vec![
                "api".to_owned(),
                "graphql".to_owned(),
                "-f".to_owned(),
                format!("query={BOARD}"),
                "-f".to_owned(),
                format!("id={id}"),
                "-f".to_owned(),
                format!("field={}", project.field),
            ];
            if let Some(cursor) = &cursor {
                args.push("-f".to_owned());
                args.push(format!("cursor={cursor}"));
            }
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let value = json(&gh(None, &args)?)?;
            let items = &value["data"]["node"]["items"];
            queued.extend(board_of(items, repository));
            cursor = items["pageInfo"]["endCursor"].as_str().map(str::to_owned);
            if items["pageInfo"]["hasNextPage"].as_bool() != Some(true) {
                break;
            }
        }
        Ok(Some(queued))
    }
}

/// The comments of a `gh issue view --json comments` answer.
#[must_use]
pub fn comments_of(value: &Value) -> Vec<Comment> {
    value["comments"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|comment| Comment {
            body: comment["body"].as_str().unwrap_or_default().to_owned(),
            at: comment["createdAt"]
                .as_str()
                .and_then(iso_seconds)
                .unwrap_or(0),
        })
        .collect()
}

/// The open-project items of an [`ITEMS_QUERY`] answer.
fn items_of(value: &Value) -> Vec<Item> {
    value["data"]["repository"]["issue"]["projectItems"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|node| node["project"]["closed"].as_bool() != Some(true))
        .map(|node| {
            let field = &node["project"]["field"];
            let status = &node["fieldValueByName"];
            Item {
                project: node["project"]["id"].as_str().unwrap_or_default().into(),
                project_title: node["project"]["title"].as_str().unwrap_or_default().into(),
                item: node["id"].as_str().unwrap_or_default().into(),
                field: field["id"].as_str().map(str::to_owned),
                options: field["options"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|option| {
                        Some((
                            option["name"].as_str()?.to_owned(),
                            option["id"].as_str()?.to_owned(),
                        ))
                    })
                    .collect(),
                status: status["name"].as_str().map(str::to_owned),
                status_at: status["updatedAt"]
                    .as_str()
                    .and_then(iso_seconds)
                    .unwrap_or(0),
            }
        })
        .collect()
}

/// Marks an [`Item`] found over REST: its `project` is this and then the
/// project's REST path, and its ids are REST's numeric ones.
const REST_PROJECT: &str = "rest:";

/// A REST name, which is `{"raw": ..}` or a plain string.
fn rest_name(value: &Value) -> Option<&str> {
    value["raw"].as_str().or_else(|| value.as_str())
}

/// The single-select `field` of a REST project's fields answer: its id
/// and its options (name, id). Names match without regard to case.
fn rest_field(fields: &Value, field: &str) -> Option<(String, Vec<(String, String)>)> {
    let found = fields.as_array()?.iter().find(|candidate| {
        rest_name(&candidate["name"]).is_some_and(|name| name.eq_ignore_ascii_case(field))
    })?;
    let id = match &found["id"] {
        Value::Number(id) => id.to_string(),
        Value::String(id) => id.clone(),
        _ => return None,
    };
    let options = found["options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|option| {
            Some((
                rest_name(&option["name"])?.to_owned(),
                option["id"].as_str()?.to_owned(),
            ))
        })
        .collect();
    Some((id, options))
}

/// The issue's item in a REST items answer: its id, `field`'s value when
/// the answer carries it, and when the item last changed. A search for
/// the number also finds issues that mention it and issues of other
/// repositories; those are skipped.
fn rest_item(
    items: &Value,
    repository: &str,
    number: u64,
    field: &str,
) -> Option<(String, Option<String>, u64)> {
    let suffix = format!("/repos/{}", repository.to_ascii_lowercase());
    let item = items.as_array()?.iter().find(|item| {
        item["content"]["number"].as_u64() == Some(number)
            && item["content"]["repository_url"]
                .as_str()
                .is_some_and(|url| url.to_ascii_lowercase().ends_with(&suffix))
    })?;
    let status = item["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|value| {
            rest_name(&value["name"]).is_some_and(|name| name.eq_ignore_ascii_case(field))
        })
        .and_then(|value| rest_name(&value["value"]["name"]))
        .map(str::to_owned);
    Some((
        item["id"].as_u64()?.to_string(),
        status,
        item["updated_at"]
            .as_str()
            .and_then(iso_seconds)
            .unwrap_or(0),
    ))
}

/// The repository's issues on one [`BOARD`] page, in project order.
fn board_of(items: &Value, repository: &str) -> Vec<Queued> {
    items["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|node| node["content"]["__typename"].as_str() == Some("Issue"))
        .filter(|node| {
            node["content"]["repository"]["nameWithOwner"]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(repository))
        })
        .filter_map(|node| {
            let issue = &node["content"];
            let blockers = &issue["blockedBy"];
            Some(Queued {
                number: issue["number"].as_u64()?,
                open: issue["state"].as_str() == Some("OPEN"),
                status: node["fieldValueByName"]["name"].as_str().map(str::to_owned),
                labels: issue["labels"]["nodes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|label| label["name"].as_str().map(str::to_owned))
                    .collect(),
                blocked: blockers["pageInfo"]["hasNextPage"].as_bool() == Some(true)
                    || blockers["nodes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|blocker| blocker["state"].as_str() != Some("CLOSED")),
            })
        })
        .collect()
}

pub mod fake;

#[cfg(test)]
#[path = "claim_tests.rs"]
mod tests;
