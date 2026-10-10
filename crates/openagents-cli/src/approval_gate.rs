//! The owner's gate for `openagents deploy` and `openagents pr` (#11169,
//! #11170): the approval policy ([`coder_new::risk_policy`]) says whether an
//! ability runs freely, waits for the owner, or never runs from a command.
//!
//! An ability that asks runs with `--approval ID`, an answer the owner gave
//! in Coder (the terminal, the phone, or the chat's page on
//! openagents.com), used once and only for exactly what it approved. Typed
//! here at a terminal, `yes` approves and is recorded the same way. Without
//! either, nothing runs.

use std::io::{BufRead, IsTerminal, Write};

use coder_new::risk_policy::{self, Decision, Policy, Record, Rule};

/// How a gated run was opened.
#[derive(Debug)]
pub(crate) enum Opened {
    /// The policy lets it run.
    Free,
    /// The owner approved it; the record is now used.
    Approved(Record),
}

impl Opened {
    /// The approval as a command's JSON reports it.
    pub(crate) fn json(&self) -> serde_json::Value {
        match self {
            Opened::Free => serde_json::Value::Null,
            Opened::Approved(record) => serde_json::json!({
                "id": record.id,
                "by": record.by,
                "via": record.via,
                "subject": record.subject,
                "at_unix": record.at_unix,
            }),
        }
    }
}

/// The policy for the checkout here, or the defaults outside one.
pub(crate) fn policy_here() -> Result<Policy, String> {
    let top = std::env::current_dir()
        .ok()
        .and_then(|here| coder::task::local::checkout(&here).ok())
        .map(|checkout| checkout.top);
    Policy::load(top.as_deref())
}

/// Opens one run of `ability` on `subject` under `policy`. `question` is
/// what a person at this terminal is asked.
///
/// # Errors
/// The policy denies it, the approval doesn't fit, the owner said no, or
/// nobody can be asked.
pub(crate) fn open(
    policy: &Policy,
    ability: &str,
    subject: &str,
    approval: Option<&str>,
    question: &str,
) -> Result<Opened, String> {
    let interactive = std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    let path = risk_policy::approvals_path();
    open_with(policy, ability, subject, approval, path.as_deref(), || {
        if !interactive {
            return None;
        }
        let mut stderr = std::io::stderr();
        let _ = write!(stderr, "{question}\nType yes to approve: ");
        let _ = stderr.flush();
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line).ok()?;
        Some(line.trim().eq_ignore_ascii_case("yes"))
    })
}

/// [`open`] with the approvals file at `path` and the terminal's answer
/// from `ask`: `None` when nobody can be asked.
pub(crate) fn open_with(
    policy: &Policy,
    ability: &str,
    subject: &str,
    approval: Option<&str>,
    path: Option<&std::path::Path>,
    ask: impl FnOnce() -> Option<bool>,
) -> Result<Opened, String> {
    match policy.rule(ability) {
        Rule::Allow => return Ok(Opened::Free),
        Rule::Deny => {
            return Err(format!(
                "The approval policy (.openagents/approvals.json) never lets this run: {ability}."
            ));
        }
        Rule::Ask => {}
    }
    let path = path.ok_or("This computer has no home folder to keep approvals in.")?;
    if let Some(id) = approval {
        return risk_policy::consume(path, id, ability, subject).map(Opened::Approved);
    }
    match ask() {
        Some(yes) => {
            let decision = if yes {
                Decision::Approved
            } else {
                Decision::Denied
            };
            let record = risk_policy::record(
                path,
                ability,
                subject,
                decision,
                &risk_policy::local_user(),
                "terminal",
            )?;
            if !yes {
                return Err("Not approved, so nothing changed.".into());
            }
            risk_policy::consume(path, &record.id, ability, subject).map(Opened::Approved)
        }
        None => Err(format!(
            "This waits for the owner's approval ({ability}). Ask for it in Coder, which asks \
             the owner in the terminal, on the phone, and on the chat's page; or run this in a \
             terminal and answer yes."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUBJECT: &str = "acme/app#7@abc123";

    #[test]
    fn a_free_ability_runs_and_an_asking_one_needs_the_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.jsonl");
        let policy = Policy::default();
        assert!(matches!(
            open_with(
                &policy,
                risk_policy::DEPLOY_STAGING,
                "origin/main",
                None,
                Some(&path),
                || None
            ),
            Ok(Opened::Free)
        ));
        let nobody = open_with(
            &policy,
            risk_policy::PR_MERGE,
            SUBJECT,
            None,
            Some(&path),
            || None,
        );
        assert!(nobody.unwrap_err().contains("approval"));
        let no = open_with(
            &policy,
            risk_policy::PR_MERGE,
            SUBJECT,
            None,
            Some(&path),
            || Some(false),
        );
        assert!(no.unwrap_err().contains("Not approved"));
        let yes = open_with(
            &policy,
            risk_policy::PR_MERGE,
            SUBJECT,
            None,
            Some(&path),
            || Some(true),
        )
        .unwrap();
        let Opened::Approved(record) = yes else {
            panic!("not approved");
        };
        assert_eq!(record.via, "terminal");
        assert!(record.used_unix.is_some());
        let records = risk_policy::records(&path).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].decision, Decision::Denied);
    }

    #[test]
    fn an_approval_from_coder_opens_one_run_of_its_subject() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.jsonl");
        let policy = Policy::default();
        let given = risk_policy::record(
            &path,
            risk_policy::PR_MERGE,
            SUBJECT,
            Decision::Approved,
            "chris",
            "phone",
        )
        .unwrap();
        let asked = || panic!("asked the terminal with an approval in hand");
        let other = open_with(
            &policy,
            risk_policy::PR_MERGE,
            "acme/app#7@moved",
            Some(&given.id),
            Some(&path),
            asked,
        );
        assert!(other.is_err());
        let opened = open_with(
            &policy,
            risk_policy::PR_MERGE,
            SUBJECT,
            Some(&given.id),
            Some(&path),
            || None,
        )
        .unwrap();
        assert_eq!(opened.json()["via"], "phone");
        assert!(
            open_with(
                &policy,
                risk_policy::PR_MERGE,
                SUBJECT,
                Some(&given.id),
                Some(&path),
                || None
            )
            .is_err()
        );
    }
}
