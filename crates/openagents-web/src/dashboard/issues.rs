//! Bounded GitHub reads using only the caller's connected credential.
use super::*;
use std::{collections::BTreeSet, time::Duration};

fn unavailable() -> Response {
    refused(
        StatusCode::BAD_GATEWAY,
        "github_unavailable",
        "Couldn't load issues. Try again.",
    )
}
async fn github(
    client: &reqwest::Client,
    token: &str,
    url: &str,
    query: &[(&str, String)],
    body: Option<Value>,
) -> Result<Value, Response> {
    let request = if let Some(body) = body {
        client.post(url).json(&body)
    } else {
        client.get(url).query(query)
    };
    let mut response = request
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| unavailable())?;
    let status = response.status();
    if !status.is_success() {
        let rate = status.as_u16() == 429
            || status.as_u16() == 403
                && response
                    .headers()
                    .get("x-ratelimit-remaining")
                    .is_some_and(|v| v == "0");
        let mut out = if rate {
            refused(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "GitHub is busy. Try again later.",
            )
        } else if matches!(status.as_u16(), 401 | 403 | 404) {
            refused(
                StatusCode::FORBIDDEN,
                "github_refused",
                "Reconnect GitHub to see these issues.",
            )
        } else {
            unavailable()
        };
        if rate {
            if let Some(v) = response.headers().get("retry-after") {
                out.headers_mut().insert("retry-after", v.clone());
            }
        }
        return Err(out);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
        if bytes.len().saturating_add(chunk.len()) > 1024 * 1024 {
            return Err(unavailable());
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
    if value.get("errors").is_some() {
        return Err(unavailable());
    }
    Ok(value)
}
pub(super) fn query(filter: &IssueFilter, repositories: &[String], login: Option<&str>) -> String {
    let mut q = String::from("is:issue is:open");
    for r in repositories {
        q.push_str(&format!(" repo:{r}"));
    }
    if let Some(login) = login {
        q.push_str(&format!(" assignee:{login}"));
    }
    for label in &filter.labels {
        q.push_str(&format!(" label:\"{label}\""));
    }
    q
}
pub(super) async fn summary(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<IssueFilter>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(mut filter)) = body else {
        return invalid("Choose issue filters again.");
    };
    if !(1..=100).contains(&filter.page)
        || filter.scope.validate().is_err()
        || filter.labels.len() > 10
        || filter
            .labels
            .iter()
            .any(|l| !short(l, 80) || l.contains(['"', '\\']))
        || filter.assignee.as_ref().is_some_and(|a| !id(a))
    {
        return invalid("Choose issue filters again.");
    }
    let (service, credential, _) = match crate::projects::api::caller(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let status = match crate::projects::api::status(service, &credential).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let login = match &status.access {
        oa_auth::repos::Access::Connected { login, .. }
        | oa_auth::repos::Access::Installed { login, .. } => login.clone(),
        _ => {
            return refused(
                StatusCode::CONFLICT,
                "github_not_connected",
                "Connect GitHub to see issues.",
            );
        }
    };
    if let Some(project) = &filter.scope.project {
        let project = match crate::projects::api::named(&status.projects, project) {
            Ok(v) => v,
            Err(r) => return r,
        };
        if filter.scope.repositories.is_empty() {
            filter.scope.repositories = project
                .repositories
                .iter()
                .map(|r| r.repository.clone())
                .collect();
        }
        if filter.scope.repositories.iter().any(|r| {
            !project
                .repositories
                .iter()
                .any(|p| p.repository.eq_ignore_ascii_case(r))
        }) {
            return refused(
                StatusCode::FORBIDDEN,
                "project_role",
                "Choose a repository in this project.",
            );
        }
    }
    filter.scope.repositories.sort();
    filter.scope.repositories.dedup();
    if filter.scope.repositories.is_empty() {
        return invalid("Choose a project with repositories or add a repository.");
    }
    let token = match service.github_token_as(&credential).await {
        Ok(v) => v,
        Err(e) => return crate::projects::api::failed(e),
    };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("OpenAgents-Home")
        .build()
    {
        Ok(v) => v,
        Err(_) => return unavailable(),
    };
    let assignee = if filter.mine {
        Some(login.as_str())
    } else {
        filter.assignee.as_deref()
    };
    let mut out = IssueSummary {
        complete: true,
        observed_at: crate::chat_store::now_unix(),
        query: query(&filter, &filter.scope.repositories, assignee),
        ..IssueSummary::default()
    };
    if let Some(board) = &filter.scope.board {
        return match board_summary(
            &client,
            &token,
            &filter,
            board,
            assignee,
            out,
            "https://api.github.com/graphql",
        )
        .await
        {
            Ok(v) => answer(StatusCode::OK, json!(v)),
            Err(r) => r,
        };
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(14);
    for (index, repo) in filter.scope.repositories.iter().enumerate() {
        let result = tokio::time::timeout_at(deadline, async {
            // A missing repository must not be reported as a zero count.
            github(
                &client,
                &token,
                &format!("https://api.github.com/repos/{repo}"),
                &[],
                None,
            )
            .await?;
            github(
                &client,
                &token,
                "https://api.github.com/search/issues",
                &[
                    ("q", query(&filter, std::slice::from_ref(repo), assignee)),
                    ("per_page", "10".into()),
                    ("sort", "updated".into()),
                    ("page", filter.page.to_string()),
                ],
                None,
            )
            .await
        })
        .await;
        let found = match result {
            Ok(Ok(value)) => value,
            Ok(Err(response)) => return response,
            Err(_) => {
                out.complete = false;
                out.unavailable
                    .extend(filter.scope.repositories[index..].iter().cloned());
                break;
            }
        };
        let Some(total) = found["total_count"].as_u64() else {
            return unavailable();
        };
        out.total = out.total.saturating_add(total);
        if total > u64::from(filter.page) * 10 && filter.page < 100 {
            out.next_page = Some(filter.page + 1);
        }
        out.complete &= found["incomplete_results"].as_bool() == Some(false);
        let Some(items) = found["items"].as_array() else {
            return unavailable();
        };
        for item in items.iter().take(10) {
            if let (Some(number), Some(title)) = (item["number"].as_u64(), item["title"].as_str()) {
                if out.items.len() < 200 {
                    out.items.push(Issue {
                        repository: repo.clone(),
                        number,
                        title: title.chars().take(240).collect(),
                    });
                }
            }
        }
    }
    answer(StatusCode::OK, json!(out))
}
async fn board_summary(
    client: &reqwest::Client,
    token: &str,
    filter: &IssueFilter,
    board: &Board,
    assignee: Option<&str>,
    mut out: IssueSummary,
    endpoint: &str,
) -> Result<IssueSummary, Response> {
    let query = r#"query($owner:String!,$number:Int!,$after:String){repositoryOwner(login:$owner){... on Organization{projectV2(number:$number){url items(first:100,after:$after){pageInfo{hasNextPage endCursor} nodes{content{... on Issue{id number title state repository{nameWithOwner} assignees(first:100){totalCount nodes{login}} labels(first:100){totalCount nodes{name}}}}}}} ... on User{projectV2(number:$number){url items(first:100,after:$after){pageInfo{hasNextPage endCursor} nodes{content{... on Issue{id number title state repository{nameWithOwner} assignees(first:100){totalCount nodes{login}} labels(first:100){totalCount nodes{name}}}}}}}}}"#;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(14);
    let mut after: Option<String> = None;
    let mut seen = BTreeSet::new();
    for page in 0..20 {
        let body = match tokio::time::timeout_at(deadline, github(client,token,endpoint,&[],Some(json!({"query":query,"variables":{"owner":board.owner,"number":board.number,"after":after}})))).await {
            Ok(value) => value?,
            Err(_) => { out.complete = false; break; }
        };
        let project = &body["data"]["repositoryOwner"]["projectV2"];
        out.board_url = project["url"].as_str().map(str::to_owned);
        let connection = &project["items"];
        let Some(nodes) = connection["nodes"].as_array() else {
            return Err(refused(
                StatusCode::FORBIDDEN,
                "github_refused",
                "GitHub couldn't open this board. Check its access.",
            ));
        };
        for node in nodes {
            let issue = &node["content"];
            if issue["state"] != "OPEN" {
                continue;
            }
            let Some(repo) = issue["repository"]["nameWithOwner"].as_str() else {
                continue;
            };
            if !filter
                .scope
                .repositories
                .iter()
                .any(|r| r.eq_ignore_ascii_case(repo))
            {
                continue;
            }
            let assigned = issue["assignees"]["nodes"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default();
            let labels = issue["labels"]["nodes"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default();
            if assigned.len() as u64 != issue["assignees"]["totalCount"].as_u64().unwrap_or(0)
                || labels.len() as u64 != issue["labels"]["totalCount"].as_u64().unwrap_or(0)
            {
                out.complete = false;
            }
            if assignee.is_some_and(|a| {
                !assigned.iter().any(|n| {
                    n["login"]
                        .as_str()
                        .is_some_and(|v| v.eq_ignore_ascii_case(a))
                })
            }) || filter
                .labels
                .iter()
                .any(|l| !labels.iter().any(|n| n["name"].as_str() == Some(l)))
            {
                continue;
            }
            let (Some(number), Some(title), Some(id)) = (
                issue["number"].as_u64(),
                issue["title"].as_str(),
                issue["id"].as_str(),
            ) else {
                continue;
            };
            if seen.insert(id.to_owned()) {
                out.total += 1;
                if out.total > u64::from(filter.page - 1) * 50 && out.items.len() < 50 {
                    out.items.push(Issue {
                        repository: repo.into(),
                        number,
                        title: title.chars().take(240).collect(),
                    });
                }
            }
        }
        if connection["pageInfo"]["hasNextPage"] == false {
            break;
        }
        let next = connection["pageInfo"]["endCursor"]
            .as_str()
            .map(str::to_owned);
        if next.is_none() || next == after {
            return Err(unavailable());
        }
        after = next;
        if page == 19 || tokio::time::Instant::now() >= deadline {
            out.complete = false;
            break;
        }
    }
    if out.total > u64::from(filter.page) * 50 {
        out.next_page = Some(filter.page + 1);
    }
    // The app opens the same board-scoped results rather than a broader search.
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn board_filters_deduplicate_and_qualify_incomplete_counts_and_rate_limits() {
        let issue = json!({"id":"issue-one","number":1,"title":"Fix the fixture","state":"OPEN","repository":{"nameWithOwner":"org/repo"},"assignees":{"totalCount":1,"nodes":[{"login":"octo"}]},"labels":{"totalCount":101,"nodes":[{"name":"bug"}]}});
        let mut closed = issue.clone();
        closed["id"] = json!("closed");
        closed["state"] = json!("CLOSED");
        let mut other = issue.clone();
        other["id"] = json!("other");
        other["repository"]["nameWithOwner"] = json!("private/elsewhere");
        let body = json!({"data":{"repositoryOwner":{"projectV2":{"url":"https://github.com/orgs/org/projects/19","items":{"pageInfo":{"hasNextPage":false},"nodes":[{"content":issue.clone()},{"content":issue},{"content":closed},{"content":other},{"content":{"title":"Draft"}}]}}}}});
        let router = Router::new()
            .route(
                "/board",
                post(move || {
                    let body = body.clone();
                    async move { Json(body) }
                }),
            )
            .route(
                "/limited",
                get(|| async {
                    (
                        StatusCode::TOO_MANY_REQUESTS,
                        [("retry-after", "120")],
                        "busy",
                    )
                }),
            )
            .route("/revoked", get(|| async { StatusCode::FORBIDDEN }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = reqwest::Client::new();
        let filter = IssueFilter {
            scope: Scope {
                repositories: vec!["org/repo".into()],
                ..Scope::default()
            },
            mine: true,
            assignee: None,
            labels: vec!["bug".into()],
            page: 1,
        };
        let board = Board {
            owner: "org".into(),
            number: 19,
        };
        let summary = board_summary(
            &client,
            "fixture",
            &filter,
            &board,
            Some("octo"),
            IssueSummary {
                complete: true,
                ..IssueSummary::default()
            },
            &format!("{origin}/board"),
        )
        .await
        .unwrap();
        assert_eq!(summary.total, 1);
        assert_eq!(summary.items.len(), 1);
        assert!(!summary.complete);
        assert_eq!(
            summary.board_url.as_deref(),
            Some("https://github.com/orgs/org/projects/19")
        );
        let empty = board_summary(
            &client,
            "fixture",
            &filter,
            &board,
            Some("nobody"),
            IssueSummary {
                complete: true,
                ..IssueSummary::default()
            },
            &format!("{origin}/board"),
        )
        .await
        .unwrap();
        assert_eq!(empty.total, 0);
        assert!(empty.items.is_empty());
        let limited = github(&client, "fixture", &format!("{origin}/limited"), &[], None)
            .await
            .unwrap_err();
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(limited.headers()["retry-after"], "120");
        assert_eq!(
            github(&client, "fixture", &format!("{origin}/revoked"), &[], None)
                .await
                .unwrap_err()
                .status(),
            StatusCode::FORBIDDEN
        );
        task.abort();
    }
}
