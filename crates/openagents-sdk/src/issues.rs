//! Issue claims linked to account runs, and GitHub project board status.

use crate::{CancellationToken, Client, Error, Session};
use reqwest::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// A GitHub issue's latest unreleased claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// GitHub repository as `owner/name`.
    pub repo: String,
    /// Positive issue number.
    pub issue: u64,
    /// Chat or harness session in the GitHub comment, or `an agent` for an older unnamed claim.
    pub holder: String,
    /// The account's run ID, when a matching run is known.
    pub run: Option<String>,
    /// Unix milliseconds when the claim was made.
    pub claimed_at: i64,
    /// Latest known output or progress, in Unix milliseconds; heartbeats alone do not advance it.
    pub last_progress_at: Option<i64>,
}

/// Options for taking an issue claim. A chat holds one issue or an explicit batch of at most sixteen.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRequest {
    /// Run ID or harness session, such as `claude-code:session-id`.
    pub holder: String,
    /// A live run to use when the holder matches more than one run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Every issue in this active batch, including the issue being claimed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch: Vec<u64>,
    /// Replace another holder. This does not expand the batch limit.
    #[serde(default)]
    pub force: bool,
}

impl ClaimRequest {
    /// Checks identifiers and the named batch before sending an issue claim.
    pub fn valid_for(&self, issue: u64) -> bool {
        crate::runs::valid_id(&self.holder)
            && self.run.as_deref().is_none_or(crate::runs::valid_id)
            && issue > 0
            && (self.batch.is_empty()
                || (self.batch.len() <= 3
                    && self.batch.contains(&issue)
                    && self.batch.iter().all(|n| *n > 0)
                    && self
                        .batch
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == self.batch.len()))
    }
}

/// The status to set on each of an issue's GitHub projects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardStatus {
    /// Ready to start; uses a project's `Ready` or `Todo` option.
    Ready,
    /// Work is in progress.
    InProgress,
    /// Work is blocked.
    Blocked,
    /// Work is done. This moves the board status; it does not close the issue.
    Done,
}

#[derive(Deserialize)]
struct Listing {
    claims: Vec<Claim>,
}

/// Issue coordination using the signed-in account's GitHub connection.
pub struct Issues<'a> {
    client: &'a Client,
    session: &'a Session,
}

impl Client {
    /// Access issue claims and project board status for this signed-in session.
    pub fn issues<'a>(&'a self, session: &'a Session) -> Result<Issues<'a>, Error> {
        self.check_session(session)?;
        Ok(Issues {
            client: self,
            session,
        })
    }
}

fn repository(repo: &str) -> Result<(&str, &str), Error> {
    let safe = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && !matches!(s, "." | "..")
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    };
    repo.split_once('/')
        .filter(|(owner, name)| safe(owner) && safe(name))
        .ok_or(Error::InvalidReference)
}

impl Issues<'_> {
    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        body: Option<Value>,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        self.client.check_session(self.session)?;
        tokio::select! { biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.client.request(method, path, &[], body, Some(self.session.token())) => result,
        }
    }

    /// Read the repository's unreleased claims and join them to this account's run progress.
    pub async fn claims(
        &self,
        repo: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<Claim>, Error> {
        let (owner, name) = repository(repo)?;
        let listing: Listing = self
            .send(Method::GET, &["repos", owner, name, "claims"], None, cancel)
            .await?;
        Ok(listing.claims)
    }

    /// Claim an issue for a holder with one matching live run. Use `claim_with` to name a run or batch.
    pub async fn claim(
        &self,
        repo: &str,
        issue: u64,
        holder: &str,
        cancel: &CancellationToken,
    ) -> Result<Claim, Error> {
        self.claim_with(
            repo,
            issue,
            &ClaimRequest {
                holder: holder.into(),
                ..ClaimRequest::default()
            },
            cancel,
        )
        .await
    }

    /// Claim an issue for a live run, optionally naming a batch or replacing another holder.
    /// An exact retry keeps the existing claim. Force never bypasses the batch limit.
    pub async fn claim_with(
        &self,
        repo: &str,
        issue: u64,
        request: &ClaimRequest,
        cancel: &CancellationToken,
    ) -> Result<Claim, Error> {
        let (owner, name) = repository(repo)?;
        if !request.valid_for(issue) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::PUT,
            &["repos", owner, name, "issues", &issue.to_string(), "claim"],
            Some(json!(request)),
            cancel,
        )
        .await
    }

    /// Release an issue claim, remove the GitHub assignee, and move its projects to Ready or Todo.
    pub async fn release(
        &self,
        repo: &str,
        issue: u64,
        note: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        let (owner, name) = repository(repo)?;
        if issue == 0 || note.is_some_and(|n| n.len() > 2000) {
            return Err(Error::InvalidReference);
        }
        let _: Value = self
            .send(
                Method::DELETE,
                &["repos", owner, name, "issues", &issue.to_string(), "claim"],
                Some(json!({"note":note})),
                cancel,
            )
            .await?;
        Ok(())
    }

    /// Set each project's Status option. This does not close the GitHub issue.
    pub async fn board(
        &self,
        repo: &str,
        issue: u64,
        status: BoardStatus,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        let (owner, name) = repository(repo)?;
        if issue == 0 {
            return Err(Error::InvalidReference);
        }
        let _: Value = self
            .send(
                Method::PUT,
                &["repos", owner, name, "issues", &issue.to_string(), "board"],
                Some(json!({"status":status})),
                cancel,
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batches_are_named_positive_distinct_and_bounded() {
        let mut r = ClaimRequest {
            holder: "codex:session".into(),
            ..ClaimRequest::default()
        };
        assert!(r.valid_for(1));
        r.batch = vec![1, 2, 3];
        assert!(r.valid_for(2));
        for batch in [vec![1, 2, 3, 4], vec![1, 1], vec![0, 1], vec![2, 3]] {
            r.batch = batch;
            assert!(!r.valid_for(1));
        }
        assert!(repository("owner/repo").is_ok());
        for repo in [
            "owner/../repo",
            "../repo",
            "/repo",
            "owner/",
            "owner/repo?query",
        ] {
            assert!(repository(repo).is_err());
        }
    }
}
