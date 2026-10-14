//! The box dispatcher (#11562, spec `docs/cloud/solver-on-gcp.md`): takes
//! solver work runs from openagents.com and runs each on a box of its own.
//!
//! It asks the site for the next solver run
//! (`POST /v1/work-hosts/{name}/claim {engines: ["solver"]}`), runs it with
//! [`crate::run_until`], and reports its progress lines and its result
//! (`POST /v1/work-hosts/{name}/runs/{id} {lines?, result?}`), every few
//! seconds and at least every 30 seconds while the run lives. The site's
//! `{cancel: true}` answer stops the run and deletes its box. The site
//! never reaches the dispatcher.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::{Credentials, GoogleToken, Progress, Run, Stop};

/// How often progress lines are sent while a run lives.
pub const REPORT_EVERY: Duration = Duration::from_secs(5);
/// How long a run may go without a report: the site fails a run after
/// 10 quiet minutes.
pub const HEARTBEAT: Duration = Duration::from_secs(30);
/// Seconds between claims while nothing is waiting.
pub const POLL: Duration = Duration::from_secs(10);
/// The engines this dispatcher runs.
pub const ENGINES: [&str; 1] = ["solver"];

/// One site the dispatcher takes runs from, as the work host `name`.
#[derive(Clone)]
pub struct Site {
    pub base: String,
    pub name: String,
    token: String,
    http: reqwest::Client,
}

impl std::fmt::Debug for Site {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Site")
            .field("base", &self.base)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// A run the site handed out.
#[derive(Clone, PartialEq, Eq)]
pub struct Claimed {
    pub id: String,
    pub repo: String,
    pub issue: u64,
    pub engine: String,
    /// The requester's GitHub token, when their account connected GitHub.
    pub github_token: Option<String>,
}

impl std::fmt::Debug for Claimed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Claimed")
            .field("id", &self.id)
            .field("repo", &self.repo)
            .field("issue", &self.issue)
            .field("engine", &self.engine)
            .field("github_token", &self.github_token.is_some())
            .finish()
    }
}

/// The run in a claim answer (`{run: {...}}` or `{run: null}`). The
/// requester's GitHub token is `run.github_token`, or `GH_TOKEN` in the
/// run's `env`.
#[must_use]
pub fn claimed(answer: &Value) -> Option<Claimed> {
    let run = answer.get("run").filter(|run| run.is_object())?;
    let repo = run["repo"].as_str()?;
    crate::parse_issue(&format!("{repo}#{}", run["issue"].as_u64()?))?;
    let token = run["github_token"]
        .as_str()
        .or_else(|| run["env"]["GH_TOKEN"].as_str())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_owned);
    Some(Claimed {
        id: run["id"].as_str().filter(|id| !id.is_empty())?.to_owned(),
        repo: repo.to_owned(),
        issue: run["issue"].as_u64()?,
        engine: run["engine"].as_str().unwrap_or("solver").to_owned(),
        github_token: token,
    })
}

/// A progress line as the site keeps it: `{secs, phase, text}`, `phase`
/// being the line's kind, plus its own fields as `data`.
#[must_use]
pub fn site_line(progress: &Progress, secs: f64) -> Value {
    let mut line = json!({"secs": (secs * 10.0).round() / 10.0, "phase": progress.kind, "text": progress.text});
    if !progress.data.is_null() {
        line["data"] = progress.data.clone();
    }
    line
}

impl Site {
    /// The site at `base` (`https://openagents.com`), as host `name`, with
    /// the work host token.
    ///
    /// # Errors
    /// The HTTP client could not start.
    pub fn new(base: &str, name: &str, token: &str) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("the HTTP client did not start: {e}"))?;
        Ok(Self {
            base: base.trim_end_matches('/').to_owned(),
            name: name.to_owned(),
            token: token.to_owned(),
            http,
        })
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        let response = self
            .http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("{} did not answer: {e}", self.base))?;
        let status = response.status();
        let answer: Value = response.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            Ok(answer)
        } else {
            Err(format!(
                "{} answered {status}: {}",
                self.base,
                answer["error"]["message"].as_str().unwrap_or("no reason")
            ))
        }
    }

    /// The next waiting solver run, if any.
    ///
    /// # Errors
    /// The site did not answer or refused.
    pub async fn claim(&self) -> Result<Option<Claimed>, String> {
        let answer = self
            .post(
                &format!("/v1/work-hosts/{}/claim", self.name),
                &json!({"engines": ENGINES}),
            )
            .await?;
        Ok(claimed(&answer))
    }

    /// Sends `body` for run `id`; `true` when the site asks to stop it.
    ///
    /// # Errors
    /// The site did not answer or refused.
    pub async fn report(&self, id: &str, body: &Value) -> Result<bool, String> {
        let answer = self
            .post(&format!("/v1/work-hosts/{}/runs/{id}", self.name), body)
            .await?;
        Ok(answer["cancel"] == true)
    }
}

/// What the dispatcher runs with: the Box service, Google tokens, the
/// pool's GitHub token for requesters without one, and the run's model and
/// image.
#[derive(Clone)]
pub struct Dispatcher {
    pub boat: boat::Client,
    pub google: GoogleToken,
    /// The pool's git token: used when the requester has not connected
    /// GitHub.
    pub pool_github_token: Option<String>,
    pub model: Option<String>,
    pub image: Option<String>,
}

impl Dispatcher {
    /// Runs one claimed run to its end and reports it; returns the result.
    pub async fn work(&self, site: &Site, claimed: &Claimed, log: impl Fn(&str)) -> Value {
        let started = Instant::now();
        if claimed.engine != "solver" {
            let why = "This computer only runs Solver in the cloud.";
            let _ = site.report(&claimed.id, &json!({"failed": why})).await;
            return json!({"ok": false, "error": why});
        }
        let Some(github_token) = claimed
            .github_token
            .clone()
            .or_else(|| self.pool_github_token.clone())
        else {
            let why = format!("Connect GitHub with access to {}.", claimed.repo);
            let _ = site.report(&claimed.id, &json!({"failed": why})).await;
            return json!({"ok": false, "error": why});
        };
        let mut run = Run::new(&claimed.repo, claimed.issue);
        if let Some(model) = &self.model {
            run.model.clone_from(model);
        }
        if let Some(image) = &self.image {
            run.image.clone_from(image);
        }
        let credentials = Credentials {
            github_token,
            openagents_token: None,
        };
        let stop = Stop::default();
        let pending: Arc<Mutex<Vec<Value>>> = Arc::default();
        let reporter = {
            let (site, id, pending, stop) = (
                site.clone(),
                claimed.id.clone(),
                pending.clone(),
                stop.clone(),
            );
            tokio::spawn(async move {
                let mut last = Instant::now();
                loop {
                    tokio::time::sleep(REPORT_EVERY).await;
                    let lines: Vec<Value> = std::mem::take(
                        &mut *pending.lock().unwrap_or_else(PoisonError::into_inner),
                    );
                    if lines.is_empty() && last.elapsed() < HEARTBEAT {
                        continue;
                    }
                    match site.report(&id, &json!({"lines": lines})).await {
                        Ok(cancel) => {
                            last = Instant::now();
                            if cancel {
                                stop.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                        // Kept for the next report.
                        Err(_) => {
                            pending
                                .lock()
                                .unwrap_or_else(PoisonError::into_inner)
                                .splice(0..0, lines);
                        }
                    }
                }
            })
        };
        let result = crate::run_until(
            &self.boat,
            &run,
            &credentials,
            &self.google,
            &stop,
            |progress| {
                log(&format!("{} {}", progress.kind, progress.text));
                pending
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(site_line(progress, started.elapsed().as_secs_f64()));
            },
        )
        .await;
        reporter.abort();
        let lines: Vec<Value> =
            std::mem::take(&mut *pending.lock().unwrap_or_else(PoisonError::into_inner));
        let mut body = json!({"result": result});
        if !lines.is_empty() {
            body["lines"] = json!(lines);
        }
        // The result is the run's end: it is tried a few times.
        for attempt in 0..5 {
            match site.report(&claimed.id, &body).await {
                Ok(_) => break,
                Err(e) => {
                    log(&format!("the result was not sent ({e})"));
                    tokio::time::sleep(Duration::from_secs(5 * (attempt + 1))).await;
                }
            }
        }
        result
    }

    /// Takes runs from `site` until `once` and nothing is waiting, at most
    /// `slots` at a time.
    pub async fn serve(&self, site: &Site, slots: usize, once: bool) {
        let slots = Arc::new(tokio::sync::Semaphore::new(slots.max(1)));
        let mut running = tokio::task::JoinSet::new();
        loop {
            let Ok(permit) = slots.clone().acquire_owned().await else {
                break;
            };
            match site.claim().await {
                Ok(Some(claimed)) => {
                    // Which GitHub token the run uses (never the token).
                    eprintln!(
                        "{}: run {} ({}#{}) with {}",
                        site.name,
                        claimed.id,
                        claimed.repo,
                        claimed.issue,
                        if claimed.github_token.is_some() {
                            "the requester's GitHub token"
                        } else {
                            "the pool's GitHub token"
                        }
                    );
                    let (this, site) = (self.clone(), site.clone());
                    running.spawn(async move {
                        let id = claimed.id.clone();
                        let result = this
                            .work(&site, &claimed, |text| eprintln!("run {id}: {text}"))
                            .await;
                        eprintln!(
                            "run {id}: {} {}",
                            if result["ok"] == true {
                                "pull request"
                            } else {
                                "ended without a pull request"
                            },
                            result["landed"]["url"]
                                .as_str()
                                .or_else(|| result["error"].as_str())
                                .unwrap_or_default()
                        );
                        drop(permit);
                    });
                }
                Ok(None) => {
                    drop(permit);
                    if once {
                        break;
                    }
                    tokio::time::sleep(POLL).await;
                }
                Err(e) => {
                    drop(permit);
                    eprintln!("{}: {e}", site.name);
                    if once {
                        break;
                    }
                    tokio::time::sleep(POLL).await;
                }
            }
            while running.try_join_next().is_some() {}
        }
        while running.join_next().await.is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claim_reads_the_run_and_the_requesters_github_token() {
        let run = claimed(&json!({"run": {
            "id": "w9", "repo": "AtlantisPleb/solver-box-smoke", "issue": 3,
            "land": "pr", "engine": "solver", "github_token": "gho_secret"
        }}))
        .unwrap();
        assert_eq!(run.id, "w9");
        assert_eq!(run.repo, "AtlantisPleb/solver-box-smoke");
        assert_eq!(run.issue, 3);
        assert_eq!(run.github_token.as_deref(), Some("gho_secret"));
        assert!(!format!("{run:?}").contains("gho_secret"));
        let from_env = claimed(&json!({"run": {
            "id": "w1", "repo": "o/r", "issue": 1, "engine": "solver", "env": {"GH_TOKEN": "t"}
        }}))
        .unwrap();
        assert_eq!(from_env.github_token.as_deref(), Some("t"));
        assert_eq!(claimed(&json!({"run": null})), None);
        assert_eq!(
            claimed(&json!({"run": {"id": "w1", "repo": "o r", "issue": 1}})),
            None
        );
    }

    #[test]
    fn progress_goes_to_the_site_as_its_kind() {
        let line = site_line(
            &Progress {
                kind: "tool",
                text: "Run cargo test -p textkit".into(),
                data: json!({"tool": "Run"}),
            },
            12.345,
        );
        assert_eq!(
            line,
            json!({"secs": 12.3, "phase": "tool", "text": "Run cargo test -p textkit", "data": {"tool": "Run"}})
        );
    }
}
