//! Solver in the cloud (#11562, `docs/cloud/solver-on-gcp.md`): ask
//! openagents.com to take a GitHub issue to a pull request on a Google
//! Cloud box of its own, and follow the run.
//!
//! [`submit`] records a work run with engine `solver`
//! (`POST /v1/work {repo, issue, land: "pr", engine: "solver"}`); the box
//! dispatcher (`oa-solver-box --serve`) takes it, runs Solver on a box with
//! every hand on Gemini 4 Argon, and reports progress lines and the result.
//! [`follow`] reads them (`GET /v1/work/{id}?after=N&wait=S`) until the run
//! ends. The account's own token (`Bearer sess_…`) signs each request.

use std::time::Duration;

use serde_json::{Value, json};

/// How long one read waits for news on the server.
const WAIT_SECS: u64 = 25;

/// Where and as whom: the site's origin and the account's token.
#[derive(Clone)]
pub struct Account {
    pub origin: String,
    pub token: String,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

/// A run the site queued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    pub id: String,
    pub state: String,
    /// The run's page, from the site's root (`/work/{id}`).
    pub url: String,
}

/// The work-run request [`submit`] sends.
#[must_use]
pub fn request(repo: &str, issue: u64) -> Value {
    json!({"repo": repo, "issue": issue, "land": "pr", "engine": "solver"})
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(WAIT_SECS + 20))
        .build()
        .map_err(|_| "The connection to OpenAgents could not start.".to_owned())
}

fn base(account: &Account) -> String {
    account.origin.trim_end_matches('/').to_owned()
}

/// The site's refusal in its own words (`{error: {message}}`), else the
/// status.
fn refusal(status: reqwest::StatusCode, body: &Value) -> String {
    body["error"]["message"]
        .as_str()
        .or_else(|| body["message"].as_str())
        .map_or_else(
            || format!("OpenAgents answered {status}."),
            |message| format!("OpenAgents refused: {message}"),
        )
}

/// Asks the site to solve `repo`'s issue `issue` in the cloud.
///
/// # Errors
/// The site could not be reached or refused; the text never holds the
/// token.
pub async fn submit(account: &Account, repo: &str, issue: u64) -> Result<Queued, String> {
    let response = client()?
        .post(format!("{}/v1/work", base(account)))
        .bearer_auth(&account.token)
        .json(&request(repo, issue))
        .send()
        .await
        .map_err(|_| "OpenAgents could not be reached.".to_owned())?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(refusal(status, &body));
    }
    queued(&body).ok_or_else(|| "OpenAgents answered without a run.".to_owned())
}

/// The queued run in a `POST /v1/work` answer.
#[must_use]
pub fn queued(body: &Value) -> Option<Queued> {
    let id = body["id"].as_str().filter(|id| !id.is_empty())?;
    Some(Queued {
        id: id.to_owned(),
        state: body["state"].as_str().unwrap_or("waiting").to_owned(),
        url: body["url"]
            .as_str()
            .map_or_else(|| format!("/work/{id}"), str::to_owned),
    })
}

/// One read of run `id`: its view with the progress lines from `after`.
///
/// # Errors
/// The site could not be reached or refused.
pub async fn read(account: &Account, id: &str, after: usize, wait: u64) -> Result<Value, String> {
    let response = client()?
        .get(format!(
            "{}/v1/work/{id}?after={after}&wait={}",
            base(account),
            wait.min(WAIT_SECS)
        ))
        .bearer_auth(&account.token)
        .send()
        .await
        .map_err(|_| "OpenAgents could not be reached.".to_owned())?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(refusal(status, &body));
    }
    Ok(body)
}

/// Whether a run view is finished.
#[must_use]
pub fn finished(view: &Value) -> bool {
    view["finished_unix"].is_number()
}

/// Follows run `id` until it ends: `on_line` gets each progress line
/// (`{secs, phase, text}`; `phase` is the solver's kind: `box`, `stage`,
/// `briefing`, `thinking`, `tool`, `checks`, `pr`, `cost`, `notice`) as it
/// arrives. Returns the finished run's view (`state`, `result.pr`,
/// `result.cost_usd`, `why`).
///
/// # Errors
/// The site could not be reached three times in a row, or refused.
pub async fn follow(
    account: &Account,
    id: &str,
    mut on_line: impl FnMut(&Value),
) -> Result<Value, String> {
    let mut after = 0;
    let mut misses = 0;
    loop {
        let view = match read(account, id, after, WAIT_SECS).await {
            Ok(view) => {
                misses = 0;
                view
            }
            Err(why) if misses < 2 && why.starts_with("OpenAgents could not") => {
                misses += 1;
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
            Err(why) => return Err(why),
        };
        for line in view["lines"].as_array().into_iter().flatten() {
            on_line(line);
        }
        after = view["next"]
            .as_u64()
            .and_then(|next| usize::try_from(next).ok())
            .unwrap_or(after);
        if finished(&view) {
            return Ok(view);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_asks_for_a_solver_run_that_opens_a_pull_request() {
        assert_eq!(
            request("AtlantisPleb/solver-box-smoke", 3),
            json!({"repo": "AtlantisPleb/solver-box-smoke", "issue": 3, "land": "pr", "engine": "solver"})
        );
    }

    #[test]
    fn a_queued_answer_names_the_run_and_its_page() {
        let run = queued(&json!({"id": "w1", "state": "waiting", "url": "/work/w1", "events": "/v1/work/w1/events"}))
            .unwrap();
        assert_eq!(
            run,
            Queued {
                id: "w1".into(),
                state: "waiting".into(),
                url: "/work/w1".into()
            }
        );
        assert_eq!(queued(&json!({"state": "waiting"})), None);
    }

    #[test]
    fn a_run_is_finished_once_it_has_a_finish_time() {
        assert!(!finished(
            &json!({"state": "running", "finished_unix": null})
        ));
        assert!(finished(
            &json!({"state": "passed", "finished_unix": 1_791_951_479})
        ));
    }

    #[test]
    fn a_refusal_is_read_in_the_sites_words_and_the_account_never_prints_its_token() {
        let why = refusal(
            reqwest::StatusCode::FORBIDDEN,
            &json!({"error": {"code": "locked", "message": "Agent work is not on for this account."}}),
        );
        assert_eq!(
            why,
            "OpenAgents refused: Agent work is not on for this account."
        );
        let account = Account {
            origin: "https://openagents.com".into(),
            token: "sess_secret".into(),
        };
        assert!(!format!("{account:?}").contains("sess_secret"));
    }
}
