//! GitHub access under the requesting account's connection.

use super::{BoardStatus, Claim, Failure};
use crate::App;
use axum::http::StatusCode;
use coder::claim::{self, Comment};
use futures_util::{StreamExt, TryStreamExt};
use github_actions::rest::{Api, Http, Method};
use serde_json::{Value, json};

pub(super) struct Github {
    http: Http,
}
pub(super) struct Observation {
    pub claim: Claim,
    pub batch: Vec<u64>,
}
pub(super) struct Issue {
    pub open: bool,
    pub comments: Vec<Comment>,
    assignees: Vec<String>,
}

impl Github {
    pub fn new(app: &App, token: String) -> Self {
        let base = app
            .config
            .github
            .as_ref()
            .map_or("https://api.github.com", |g| g.endpoints.api_url.as_str());
        Self {
            http: Http::new(base, token),
        }
    }
    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, Failure> {
        let answer = self.http.call(method, path, body).await.map_err(|_| {
            Failure::new(
                StatusCode::BAD_GATEWAY,
                "github_unavailable",
                "GitHub did not answer. Try again.",
            )
        })?;
        match answer.status {
            200..=299 => Ok(answer.body),
            401 | 403 => Err(Failure::new(
                StatusCode::FORBIDDEN,
                "github_denied",
                "Your GitHub connection needs access to this repository and its projects.",
            )),
            404 => Err(Failure::new(
                StatusCode::NOT_FOUND,
                "not_found",
                "No such repository or issue is available to this GitHub connection.",
            )),
            429 => Err(Failure::new(
                StatusCode::TOO_MANY_REQUESTS,
                "github_rate_limited",
                "GitHub is limiting requests. Try again later.",
            )),
            _ => Err(Failure::new(
                StatusCode::BAD_GATEWAY,
                "github_refused",
                "GitHub could not complete this change. Try again.",
            )),
        }
    }
    async fn comments(&self, repo: &str, n: u64, count: u64) -> Result<Vec<Comment>, Failure> {
        let last = count.div_ceil(100).max(1);
        let omitted = count.saturating_sub(1000);
        let first = (omitted / 100 + 1).max(1);
        let mut comments = Vec::new();
        for page in (first..=last).rev() {
            let value = self
                .call(
                    Method::Get,
                    &format!("/repos/{repo}/issues/{n}/comments?per_page=100&page={page}"),
                    None,
                )
                .await?;
            let rows = value.as_array().ok_or_else(Failure::unavailable)?;
            let skip = if page == first {
                (omitted % 100) as usize
            } else {
                0
            };
            let mut before: Vec<_> = rows
                .iter()
                .skip(skip)
                .map(|row| {
                    let at = row["created_at"]
                        .as_str()
                        .and_then(oa_auth::app::github_time)
                        .unwrap_or(0);
                    Comment {
                        body: row["body"].as_str().unwrap_or_default().into(),
                        at,
                    }
                })
                .collect();
            let found = before.iter().any(|c| {
                c.body.contains(claim::CLAIM_MARK)
                    || c.body.contains(claim::RELEASE_MARK)
                    || c.body.trim_start().starts_with("Claimed:")
            });
            before.append(&mut comments);
            comments = before;
            if found || (page == 1 && skip == 0) {
                return Ok(comments);
            }
        }
        Err(Failure::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "claim_history_too_long",
            "The latest claim is too far back in this issue's comments. Add a fresh claim or release comment first.",
        ))
    }
    pub async fn issue(&self, repo: &str, n: u64) -> Result<Issue, Failure> {
        let value = self
            .call(Method::Get, &format!("/repos/{repo}/issues/{n}"), None)
            .await?;
        if value.get("pull_request").is_some() {
            return Err(Failure::invalid());
        }
        Ok(Issue {
            open: value["state"] == "open",
            comments: self
                .comments(repo, n, value["comments"].as_u64().unwrap_or(0))
                .await?,
            assignees: value["assignees"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v["login"].as_str().map(str::to_owned))
                .collect(),
        })
    }
    pub async fn claims(&self, repo: &str) -> Result<Vec<Observation>, Failure> {
        let mut issues = Vec::new();
        for page in 1..=11 {
            let value = self
                .call(
                    Method::Get,
                    &format!("/repos/{repo}/issues?state=open&per_page=100&page={page}"),
                    None,
                )
                .await?;
            let rows = value.as_array().ok_or_else(Failure::unavailable)?;
            if page == 11 && !rows.is_empty() {
                return Err(Failure::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "repository_too_large",
                    "This repository has more than 1,000 open issues and pull requests. Read claims for a smaller repository.",
                ));
            }
            issues.extend(
                rows.iter()
                    .filter(|v| {
                        v.get("pull_request").is_none() && v["comments"].as_u64().unwrap_or(0) > 0
                    })
                    .filter_map(|v| Some((v["number"].as_u64()?, v["comments"].as_u64()?))),
            );
            if rows.len() < 100 {
                break;
            }
        }
        let observed: Vec<Option<Observation>> = futures_util::stream::iter(issues)
            .map(|(n, count)| async move {
                let comments = self.comments(repo, n, count).await?;
                Ok::<_, Failure>(observation(repo, n, &comments))
            })
            .buffer_unordered(8)
            .try_collect()
            .await?;
        let mut claims: Vec<_> = observed.into_iter().flatten().collect();
        claims.sort_by_key(|c| c.claim.issue);
        Ok(claims)
    }
    pub async fn comment(&self, repo: &str, n: u64, body: &str) -> Result<(), Failure> {
        self.call(
            Method::Post,
            &format!("/repos/{repo}/issues/{n}/comments"),
            Some(json!({"body":body})),
        )
        .await?;
        Ok(())
    }
    pub async fn assign(
        &self,
        repo: &str,
        n: u64,
        issue: &Issue,
        add: bool,
    ) -> Result<(), Failure> {
        let viewer = self.call(Method::Get, "/user", None).await?;
        let login = viewer["login"].as_str().ok_or_else(Failure::unavailable)?;
        let mut assignees = issue.assignees.clone();
        assignees.retain(|who| who != login);
        if add {
            assignees.push(login.into());
        }
        self.call(
            Method::Patch,
            &format!("/repos/{repo}/issues/{n}"),
            Some(json!({"assignees":assignees})),
        )
        .await?;
        Ok(())
    }
    pub async fn board(&self, repo: &str, n: u64, status: BoardStatus) -> Result<(), Failure> {
        let (owner, name) = repo.split_once('/').ok_or_else(Failure::invalid)?;
        let query = "query($owner:String!,$name:String!,$n:Int!){repository(owner:$owner,name:$name){issue(number:$n){projectItems(first:100){pageInfo{hasNextPage} nodes{id project{id fields(first:100){pageInfo{hasNextPage} nodes{... on ProjectV2SingleSelectField{id name options{id name}}}}}}}}}}";
        let value = self
            .graphql(query, json!({"owner":owner,"name":name,"n":n}))
            .await?;
        let items = &value["repository"]["issue"]["projectItems"];
        if items["pageInfo"]["hasNextPage"] == true {
            return Err(Failure::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "too_many_projects",
                "This issue belongs to more than 100 projects.",
            ));
        }
        let nodes = items["nodes"].as_array().ok_or_else(Failure::unavailable)?;
        let names: &[&str] = match status {
            BoardStatus::Ready => &["Ready", "Todo"],
            BoardStatus::InProgress => &["In progress"],
            BoardStatus::Blocked => &["Blocked"],
            BoardStatus::Done => &["Done"],
        };
        for item in nodes {
            let project = &item["project"];
            if project["fields"]["pageInfo"]["hasNextPage"] == true {
                return Err(Failure::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "too_many_fields",
                    "This project has more than 100 fields.",
                ));
            }
            let Some(field) = project["fields"]["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|f| {
                    f["name"]
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case("Status"))
                })
            else {
                continue;
            };
            let option = names.iter().find_map(|name| {
                field["options"].as_array()?.iter().find(|v| {
                    v["name"]
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case(name))
                })
            });
            let Some(option) = option else {
                return Err(Failure::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "board_status_missing",
                    "One of this issue's projects does not have that status.",
                ));
            };
            let query = "mutation($p:ID!,$i:ID!,$f:ID!,$o:String!){updateProjectV2ItemFieldValue(input:{projectId:$p,itemId:$i,fieldId:$f,value:{singleSelectOptionId:$o}}){projectV2Item{id}}}";
            self.graphql(
                query,
                json!({"p":project["id"],"i":item["id"],"f":field["id"],"o":option["id"]}),
            )
            .await?;
        }
        Ok(())
    }
    async fn graphql(&self, query: &str, variables: Value) -> Result<Value, Failure> {
        let answer = self
            .call(
                Method::Post,
                "/graphql",
                Some(json!({"query":query,"variables":variables})),
            )
            .await?;
        if answer.get("errors").is_some() {
            return Err(Failure::new(
                StatusCode::FORBIDDEN,
                "github_projects_denied",
                "Your GitHub connection could not read or change these projects. Check its project access.",
            ));
        }
        answer.get("data").cloned().ok_or_else(Failure::unavailable)
    }
}

pub(super) fn observation(repo: &str, n: u64, comments: &[Comment]) -> Option<Observation> {
    let active = claim::active(comments)?;
    let holder = claim::marker_field(&active.body, "session")
        .unwrap_or("an agent")
        .chars()
        .take(128)
        .collect();
    let run = claim::marker_field(&active.body, "run")
        .filter(|id| openagents_sdk::runs::valid_id(id))
        .map(str::to_owned);
    let batch = claim::marker_field(&active.body, "batch")
        .and_then(|s| {
            s.split(',')
                .map(str::parse)
                .collect::<Result<Vec<u64>, _>>()
                .ok()
        })
        .filter(|v| v.len() <= 3 && v.contains(&n) && v.iter().all(|n| *n > 0))
        .unwrap_or_default();
    let at = active.at.min(i64::MAX as u64 / 1000) as i64 * 1000;
    let last = comments
        .iter()
        .filter(|c| c.at >= active.at)
        .map(|c| c.at.min(i64::MAX as u64 / 1000) as i64 * 1000)
        .max();
    Some(Observation {
        claim: Claim {
            repo: repo.into(),
            issue: n,
            holder,
            run,
            claimed_at: at,
            last_progress_at: last,
        },
        batch,
    })
}
