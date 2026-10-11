//! Jobs for this Mac, from the account's cloud environments (#11223).
//!
//! The same pull channel as the own coding runs ([`crate::own_runs`],
//! #11080): the computer reports what it can do to the website's
//! `POST /v1/computers/{name}/mac-jobs` and the answer is the jobs waiting
//! for it, each handed out once. It reports each job's log lines, the
//! commit it checked out, the owner's question when the job waits for an
//! approval, and how it ended to `POST /v1/computers/{name}/mac-jobs/{id}`;
//! the answer says whether the job was cancelled, and carries the owner's
//! answer once they gave it. What the job made goes up in parts to
//! `PUT /v1/computers/{name}/mac-jobs/{id}/artifacts/{file}?part=N`.
//!
//! Everything runs under this computer's own sign-in, so the website hands
//! it only its own account's jobs. The website never reaches the Mac.

use openagents_login::Saved;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub use mac_jobs::{Capabilities, Spec};

use crate::activity::segment;
use crate::{Answer, call};

/// The largest artifact part, in bytes.
pub const PART_BYTES: usize = 8 * 1024 * 1024;

/// A job handed to this Mac.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Taken {
    pub id: String,
    pub spec: Spec,
}

/// Where this Mac claims jobs when the website runs them as actors
/// (#11253): the account's workspace, the queue, and this Mac's target.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ActorQueue {
    pub workspace: String,
    pub queue: String,
    pub target: String,
}

/// What a report of this Mac's capabilities is answered with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Took {
    /// Jobs from the website's own store, each handed out once.
    pub jobs: Vec<Taken>,
    /// With actors on, where to claim the rest.
    pub actors: Option<ActorQueue>,
}

/// Report what this Mac can do; the answer is the jobs waiting for it.
pub async fn take(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    capabilities: &Capabilities,
) -> Result<Vec<Taken>, Answer> {
    take_all(http, saved, computer, capabilities)
        .await
        .map(|took| took.jobs)
}

/// [`take`], with where to claim actor jobs when the website says.
pub async fn take_all(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    capabilities: &Capabilities,
) -> Result<Took, Answer> {
    let body = json!({"capabilities": capabilities});
    match call(
        http,
        saved,
        reqwest::Method::POST,
        &format!("/v1/computers/{}/mac-jobs", segment(computer)),
        Some(&body),
    )
    .await
    {
        (Answer::Done, body) => Ok(Took {
            jobs: body["jobs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|job| serde_json::from_value::<Taken>(job.clone()).ok())
                .filter(|job| mac_jobs::valid_job_id(&job.id) && job.spec.check().is_ok())
                .collect(),
            actors: serde_json::from_value(body["actors"].clone()).ok(),
        }),
        (answer, _) => Err(answer),
    }
}

/// The owner's question, while the job waits for an approval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Ask {
    /// The question's id; the answer names it.
    pub id: String,
    pub text: String,
    /// Exactly what is approved: the repository at its commit, the recipe,
    /// and its arguments.
    pub subject: String,
}

/// How a job ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    Done { summary: String },
    Failed { why: String },
}

/// One report on a job.
#[derive(Clone, Debug, Default)]
pub struct Report<'a> {
    pub lines: &'a [String],
    /// The commit the ref resolved to, once checked out.
    pub commit: Option<&'a str>,
    pub ask: Option<&'a Ask>,
    pub end: Option<&'a End>,
}

/// The owner's answer to a job's question.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Approval {
    pub question: String,
    /// `approved` or `denied`.
    pub decision: String,
    /// Where they answered: `web` or `phone`.
    pub via: String,
    #[serde(default)]
    pub at_unix: u64,
}

/// What the website answered a report with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Heard {
    /// The job was cancelled: stop it.
    pub cancel: bool,
    /// The owner answered the job's question (handed out once).
    pub approval: Option<Approval>,
}

/// Report on job `id`.
pub async fn report(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    id: &str,
    sent: &Report<'_>,
) -> Result<Heard, Answer> {
    if !mac_jobs::valid_job_id(id) {
        return Err(Answer::Unknown);
    }
    let mut body = json!({"lines": sent.lines});
    if let Some(commit) = sent.commit {
        body["commit"] = json!(commit);
    }
    if let Some(ask) = sent.ask {
        body["ask"] = json!(ask);
    }
    match sent.end {
        Some(End::Done { summary }) => body["done"] = json!({"summary": summary}),
        Some(End::Failed { why }) => body["failed"] = json!({"why": why}),
        None => {}
    }
    match call(
        http,
        saved,
        reqwest::Method::POST,
        &format!("/v1/computers/{}/mac-jobs/{id}", segment(computer)),
        Some(&body),
    )
    .await
    {
        (Answer::Done, body) => Ok(Heard {
            cancel: body["cancel"] == Value::Bool(true),
            approval: serde_json::from_value(body["approval"].clone()).ok(),
        }),
        (answer, _) => Err(answer),
    }
}

/// Upload part `part` of artifact `name` of job `id`; `last` on its final
/// part.
#[allow(clippy::too_many_arguments)]
pub async fn upload_part(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    id: &str,
    name: &str,
    part: u32,
    last: bool,
    bytes: Vec<u8>,
) -> Result<(), Answer> {
    if !mac_jobs::valid_job_id(id) || bytes.len() > PART_BYTES {
        return Err(Answer::Refused("That part can't be sent.".into()));
    }
    upload_to(http, saved, computer, id, name, (part, last), None, bytes).await
}

/// [`upload_part`] for an actor job (#11253), fenced by the Mac's claim of
/// it: `fence` is the claim's work item and epoch.
#[allow(clippy::too_many_arguments)]
pub async fn upload_part_fenced(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    id: &str,
    name: &str,
    part: u32,
    last: bool,
    fence: (&str, u64),
    bytes: Vec<u8>,
) -> Result<(), Answer> {
    if !mac_jobs::valid_job_id(id) || bytes.len() > PART_BYTES {
        return Err(Answer::Refused("That part can't be sent.".into()));
    }
    upload_to(
        http,
        saved,
        computer,
        id,
        name,
        (part, last),
        Some(fence),
        bytes,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn upload_to(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    id: &str,
    name: &str,
    (part, last): (u32, bool),
    fence: Option<(&str, u64)>,
    bytes: Vec<u8>,
) -> Result<(), Answer> {
    let mut url = format!(
        "{}/v1/computers/{}/mac-jobs/{id}/artifacts/{}?part={part}&last={}",
        saved.origin,
        segment(computer),
        segment(name),
        u8::from(last)
    );
    if let Some((item, epoch)) = fence {
        url.push_str(&format!("&item={}&epoch={epoch}", segment(item)));
    }
    let Ok(response) = http
        .put(url)
        .bearer_auth(saved.token())
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .body(bytes)
        .send()
        .await
    else {
        return Err(Answer::Retry);
    };
    let status = response.status().as_u16();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    match status {
        200 => Ok(()),
        401 => Err(Answer::SignedOut),
        404 => Err(Answer::Unknown),
        400..=499 if status != 408 && status != 429 => Err(Answer::Refused(
            body["error"]["message"]
                .as_str()
                .unwrap_or("The website refused this file.")
                .to_owned(),
        )),
        _ => Err(Answer::Retry),
    }
}

/// A client for these calls, or `None` when one can't be built.
#[must_use]
pub fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_and_answers_read_the_websites_words() {
        let job: Taken = serde_json::from_value(json!({
            "id": "mjob0123456789abcdef0123456789abcdef",
            "spec": {"repo": "OpenAgentsInc/openagents", "ref": "main",
                     "recipe": "ios-release-gate", "args": []}
        }))
        .unwrap();
        assert_eq!(job.spec.recipe, mac_jobs::Recipe::IosReleaseGate);
        let approval: Approval = serde_json::from_value(json!({
            "question": "1", "decision": "approved", "via": "phone", "at_unix": 5
        }))
        .unwrap();
        assert_eq!(approval.via, "phone");
        let ask = Ask {
            id: "1".into(),
            text: "Upload?".into(),
            subject: "o/r@abc ios-testflight".into(),
        };
        assert_eq!(
            serde_json::to_value(&ask).unwrap()["subject"],
            "o/r@abc ios-testflight"
        );
    }
}
