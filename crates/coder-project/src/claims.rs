//! The project supervisor writes and honours the same claim record as
//! every other path (#10203, [`coder::claim`]), in addition to its local
//! reservation ledger: before an admitted task starts, an issue someone
//! else holds — a claim comment within the window, or "In progress" on
//! the project — is left alone; a task that starts claims its issue
//! (comment, assignee, project status); an attempt that ran nothing or
//! failed releases it. An attempt that finished keeps the claim while it
//! waits for review.

use coder::claim::{self, CLAIM_MARK, Hub, RELEASE_MARK};
use coder::task::issue_run::Policy;

/// Why `issue` is held by another claim, or `None` when it is free.
/// A claim record that cannot be read holds the issue: the supervisor
/// does not start what it cannot see is free.
#[must_use]
pub fn held(
    hub: &dyn Hub,
    repository: &str,
    issue: u64,
    policy: &Policy,
    now: u64,
) -> Option<String> {
    let comments = match hub.comments(repository, issue) {
        Ok(comments) => comments,
        Err(why) => return Some(format!("#{issue}'s claim record cannot be read: {why}")),
    };
    // Projects the credential cannot read leave the comments as the record.
    let items = hub
        .items(repository, issue, &policy.project.field)
        .unwrap_or_default();
    claim::held(
        issue,
        &comments,
        &items,
        now,
        policy.claim_hours,
        &policy.project,
    )
}

/// Claims `issue` for `attempt`.
pub fn take(
    hub: &dyn Hub,
    repository: &str,
    issue: u64,
    attempt: &str,
    policy: &Policy,
) -> Vec<String> {
    let body = format!(
        "Claimed: Coder's project supervisor is working on this (attempt `{attempt}`). Its \
         result waits for a separate review.\n\n{CLAIM_MARK} project={attempt} -->"
    );
    claim::claim(hub, repository, issue, &body, &policy.project)
}

/// Releases `issue`, saying `why`.
pub fn give_back(
    hub: &dyn Hub,
    repository: &str,
    issue: u64,
    why: &str,
    policy: &Policy,
) -> Vec<String> {
    let body = format!("Coder's project supervisor released its claim: {why}\n\n{RELEASE_MARK}");
    claim::release(hub, repository, issue, Some(&body), &policy.project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder::claim::fake::Fake;

    const REPO: &str = "acme/app";

    fn policy() -> Policy {
        Policy::default()
    }

    #[test]
    fn with_a_project_the_supervisor_claims_honours_and_releases_by_status() {
        let github = Fake::with_project("octo", &["Ready", "In progress", "Done"]);
        github.issue(1, Some("Ready"), &[], &[]);
        github.issue(2, Some("In progress"), &[], &[]);
        github.issues.lock().unwrap().get_mut(&2).unwrap().status_at = github.now - 60;
        let now = github.now;
        // Someone else moved #2 to In progress: held, though no comment says so.
        let why = held(&github, REPO, 2, &policy(), now).unwrap();
        assert!(why.contains("\"In progress\" on the project"), "{why}");
        assert_eq!(held(&github, REPO, 1, &policy(), now), None);

        take(&github, REPO, 1, "attempt-1", &policy());
        let state = github.state(1);
        assert!(state.comments[0].body.contains("project=attempt-1"));
        assert_eq!(state.assignees, ["octo"]);
        assert_eq!(state.status.as_deref(), Some("In progress"));
        // A second supervisor, or the chat flow, now sees it held.
        assert!(held(&github, REPO, 1, &policy(), now + 5).is_some());

        give_back(&github, REPO, 1, "the executor had no capacity.", &policy());
        let state = github.state(1);
        assert_eq!(state.status.as_deref(), Some("Ready"));
        assert!(state.assignees.is_empty());
        assert_eq!(held(&github, REPO, 1, &policy(), now + 10), None);
    }

    #[test]
    fn without_a_project_the_supervisor_claims_by_comment_and_assignee() {
        let github = Fake::plain("octo");
        github.issue(3, None, &[], &[]);
        let now = github.now;
        assert_eq!(held(&github, REPO, 3, &policy(), now), None);
        take(&github, REPO, 3, "attempt-3", &policy());
        assert_eq!(github.state(3).assignees, ["octo"]);
        let why = held(&github, REPO, 3, &policy(), now + 5).unwrap();
        assert!(why.starts_with("#3 was claimed"), "{why}");
        give_back(&github, REPO, 3, "the attempt failed.", &policy());
        assert!(github.state(3).assignees.is_empty());
        assert_eq!(held(&github, REPO, 3, &policy(), now + 10), None);
        // An unreadable record holds rather than admits.
        assert!(held(&github, REPO, 99, &policy(), now).is_some());
    }
}
