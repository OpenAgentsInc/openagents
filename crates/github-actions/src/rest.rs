//! Running a GitHub action over GitHub's REST API: as the signed-in person
//! from the web chat (#11167), or with this computer's token from the CLI
//! (#11166). Each step is public ([`create_issue`], [`comment`],
//! [`close_issue`], [`reopen_issue`], [`place`], [`open_pull_request`]) so
//! a caller can compose its own verbs; [`run`] runs one [`Action`].
//!
//! Only GitHub's REST API, as `scripts/dev/issue-board.sh` uses it, so a
//! board move keeps working when other tools have spent the GraphQL limit:
//! issues and comments under `/repos/{owner}/{name}`, pull requests under
//! `/repos/{owner}/{name}/pulls`, and boards under
//! `/orgs|users/{owner}/projectsV2/{number}` (`fields`, `items`).
//!
//! Before anything changes, [`run`] reads which access the person's GitHub
//! connection holds (`X-OAuth-Scopes` on `GET /user`): a write without
//! `repo` or `public_repo`, or a board change without `project`, stops
//! there with [`Failure::NeedsAccess`], so asking for more access never
//! leaves half a change behind. A GitHub App's user connection names no
//! scopes and goes straight on.

use std::future::Future;

use serde_json::{Value, json};

use crate::action::{Action, Board, CloseReason};

/// The REST API version every call names.
pub const API_VERSION: &str = "2022-11-28";
/// GitHub's API origin.
pub const API_BASE: &str = "https://api.github.com";

/// An HTTP method this module uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Patch,
}

impl Method {
    /// The method's name on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Patch => "PATCH",
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One answer from GitHub.
#[derive(Clone, Debug, Default)]
pub struct Answer {
    pub status: u16,
    /// What `X-OAuth-Scopes` said the connection holds; `None` when GitHub
    /// sent no such header (a GitHub App's user connection).
    pub scopes: Option<Vec<String>>,
    pub body: Value,
}

/// GitHub's REST API as the signed-in person.
pub trait Api: Sync {
    /// `method` on `path` (starting with `/`), with a JSON body.
    fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> impl Future<Output = Result<Answer, Failure>> + Send;
}

/// Why an action didn't finish. Each says what to do next in plain words
/// ([`Failure::text`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The GitHub connection needs more access first: `board` when it is
    /// the boards access that is missing.
    NeedsAccess { board: bool },
    /// GitHub stopped accepting the connection.
    AccessEnded,
    /// GitHub found no such thing, or the connection doesn't reach it.
    NotFound(String),
    /// GitHub said no, in its words when it gave any.
    Refused(String),
    /// GitHub is limiting requests.
    Limited,
    /// The board has no such status; these are the statuses it has.
    NoStatus {
        board: u32,
        status: String,
        options: Vec<String>,
    },
    /// GitHub didn't answer, or answered with something unreadable.
    Unreachable,
}

impl Failure {
    /// What the person reads.
    pub fn text(&self) -> String {
        match self {
            Failure::NeedsAccess { board: true } => {
                "GitHub needs to let OpenAgents change your boards first.".into()
            }
            Failure::NeedsAccess { board: false } => {
                "GitHub needs to let OpenAgents change issues and pull requests first.".into()
            }
            Failure::AccessEnded => "GitHub access ended. Connect GitHub again.".into(),
            Failure::NotFound(what) => format!("GitHub couldn't find {what}."),
            Failure::Refused(why) => why.clone(),
            Failure::Limited => {
                "GitHub is limiting requests right now. Try again in a few minutes.".into()
            }
            Failure::NoStatus {
                board,
                status,
                options,
            } => {
                if options.is_empty() {
                    format!("Board {board} has no Status field to set {status} in.")
                } else {
                    format!(
                        "Board {board} has no status {status}. Its statuses are {}.",
                        options.join(", ")
                    )
                }
            }
            Failure::Unreachable => "Couldn't reach GitHub. Try again.".into(),
        }
    }
}

/// What a finished action did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
    /// One sentence: what changed.
    pub summary: String,
    /// The issue, comment or pull request on GitHub.
    pub link: Option<String>,
    /// A later step that didn't happen, such as the board move after an
    /// issue was opened.
    pub problem: Option<String>,
}

/// Run `action`. Nothing changes unless the connection holds the access
/// it needs.
pub async fn run<A: Api>(api: &A, action: &Action) -> Result<Done, Failure> {
    let user = api.call(Method::Get, "/user", None).await?;
    checked(&user, "your GitHub account")?;
    if let Some(scopes) = &user.scopes {
        let holds = |name: &str| scopes.iter().any(|s| s == name);
        if !(holds("repo") || holds("public_repo")) {
            return Err(Failure::NeedsAccess { board: false });
        }
        if action.needs_board() && !holds("project") {
            return Err(Failure::NeedsAccess { board: true });
        }
    }
    match action {
        Action::CreateIssue {
            repository,
            title,
            body,
            labels,
            board,
        } => {
            let created = create_issue(api, repository, title, body, labels).await?;
            let number = created["number"].as_u64().ok_or(Failure::Unreachable)?;
            let link = link(&created);
            let mut done = Done {
                summary: format!("Opened #{number} in {repository}."),
                link,
                problem: None,
            };
            if let Some(board) = board {
                let content = created["id"].as_u64().map(|id| (id, "Issue"));
                match place(api, repository, number, content, board).await {
                    Ok(()) => {
                        done.summary = format!(
                            "Opened #{number} in {repository} and put it on board {} as {}.",
                            board.number, board.status
                        );
                    }
                    Err(failure) => {
                        done.problem = Some(format!(
                            "It isn't on board {} yet: {}",
                            board.number,
                            failure.text()
                        ));
                    }
                }
            }
            Ok(done)
        }
        Action::Comment {
            repository,
            number,
            body,
        } => {
            let comment = comment(api, repository, *number, body).await?;
            Ok(Done {
                summary: format!("Commented on #{number} in {repository}."),
                link: link(&comment),
                problem: None,
            })
        }
        Action::CloseIssue {
            repository,
            number,
            comment: text,
            reason,
        } => {
            if let Some(text) = text {
                comment(api, repository, *number, text).await?;
            }
            let closed = close_issue(api, repository, *number, *reason).await?;
            Ok(Done {
                summary: format!("Closed #{number} in {repository}."),
                link: link(&closed),
                problem: None,
            })
        }
        Action::MoveOnBoard {
            repository,
            number,
            board,
        } => {
            place(api, repository, *number, None, board).await?;
            Ok(Done {
                summary: format!(
                    "Set #{number} in {repository} to {} on board {}.",
                    board.status, board.number
                ),
                link: Some(format!("https://github.com/{repository}/issues/{number}")),
                problem: None,
            })
        }
        Action::OpenPullRequest {
            repository,
            head,
            base,
            title,
            body,
            draft,
        } => {
            let opened =
                open_pull_request(api, repository, head, base.as_deref(), title, body, *draft)
                    .await?;
            let number = opened["number"].as_u64().ok_or(Failure::Unreachable)?;
            Ok(Done {
                summary: format!("Opened pull request #{number} in {repository}."),
                link: link(&opened),
                problem: None,
            })
        }
    }
}

/// Open an issue; GitHub's issue back.
pub async fn create_issue<A: Api>(
    api: &A,
    repository: &str,
    title: &str,
    body: &str,
    labels: &[String],
) -> Result<Value, Failure> {
    let mut sent = json!({"title": title, "body": body});
    if !labels.is_empty() {
        sent["labels"] = json!(labels);
    }
    let created = api
        .call(
            Method::Post,
            &format!("/repos/{repository}/issues"),
            Some(sent),
        )
        .await?;
    checked(&created, repository)
}

/// Close an issue for `reason`; GitHub's issue back.
pub async fn close_issue<A: Api>(
    api: &A,
    repository: &str,
    number: u64,
    reason: CloseReason,
) -> Result<Value, Failure> {
    let closed = api
        .call(
            Method::Patch,
            &format!("/repos/{repository}/issues/{number}"),
            Some(json!({"state": "closed", "state_reason": reason.word()})),
        )
        .await?;
    checked(&closed, &format!("#{number} in {repository}"))
}

/// Reopen an issue; GitHub's issue back.
pub async fn reopen_issue<A: Api>(
    api: &A,
    repository: &str,
    number: u64,
) -> Result<Value, Failure> {
    let reopened = api
        .call(
            Method::Patch,
            &format!("/repos/{repository}/issues/{number}"),
            Some(json!({"state": "open"})),
        )
        .await?;
    checked(&reopened, &format!("#{number} in {repository}"))
}

/// Open a pull request from `head` into `base` (the repository's default
/// branch when `None`); GitHub's pull request back.
pub async fn open_pull_request<A: Api>(
    api: &A,
    repository: &str,
    head: &str,
    base: Option<&str>,
    title: &str,
    body: &str,
    draft: bool,
) -> Result<Value, Failure> {
    let base = match base {
        Some(base) => base.to_string(),
        None => {
            let repo = api
                .call(Method::Get, &format!("/repos/{repository}"), None)
                .await?;
            checked(&repo, repository)?["default_branch"]
                .as_str()
                .ok_or(Failure::Unreachable)?
                .to_string()
        }
    };
    let opened = api
        .call(
            Method::Post,
            &format!("/repos/{repository}/pulls"),
            Some(json!({
                "title": title,
                "head": head,
                "base": base,
                "body": body,
                "draft": draft,
            })),
        )
        .await?;
    checked(&opened, repository)
}

/// Comment on an issue or pull request; GitHub's comment back.
pub async fn comment<A: Api>(
    api: &A,
    repository: &str,
    number: u64,
    body: &str,
) -> Result<Value, Failure> {
    let answer = api
        .call(
            Method::Post,
            &format!("/repos/{repository}/issues/{number}/comments"),
            Some(json!({"body": body})),
        )
        .await?;
    checked(&answer, &format!("#{number} in {repository}"))
}

/// Set `number`'s Status on the repository owner's board, adding it to the
/// board first when it isn't there. `content` is the issue's id and kind
/// when the caller already has them (an issue it just opened).
pub async fn place<A: Api>(
    api: &A,
    repository: &str,
    number: u64,
    content: Option<(u64, &'static str)>,
    board: &Board,
) -> Result<(), Failure> {
    let owner = repository.split('/').next().unwrap_or_default();
    let base = find_board(api, owner, board.number).await?;
    let fields = api
        .call(Method::Get, &format!("{base}/fields?per_page=100"), None)
        .await?;
    let fields = checked(&fields, &format!("board {}", board.number))?;
    let (field, option) = status_option(&fields, board)?;
    let mut item = None;
    if content.is_none() {
        let items = api
            .call(
                Method::Get,
                &format!("{base}/items?per_page=100&q={number}"),
                None,
            )
            .await?;
        let items = checked(&items, &format!("board {}", board.number))?;
        item = find_item(&items, repository, number);
    }
    let item = match item {
        Some(item) => item,
        None => {
            let (id, kind) = match content {
                Some(content) => content,
                None => content_of(api, repository, number).await?,
            };
            let added = api
                .call(
                    Method::Post,
                    &format!("{base}/items"),
                    Some(json!({"type": kind, "id": id})),
                )
                .await?;
            let added = checked(&added, &format!("board {}", board.number))?;
            added["id"].as_u64().ok_or(Failure::Unreachable)?
        }
    };
    let set = api
        .call(
            Method::Patch,
            &format!("{base}/items/{item}"),
            Some(json!({"fields": [{"id": field, "value": option}]})),
        )
        .await?;
    checked(&set, &format!("board {}", board.number))?;
    Ok(())
}

/// The board's REST path: an organization's, else a person's.
pub async fn find_board<A: Api>(api: &A, owner: &str, number: u32) -> Result<String, Failure> {
    for scope in ["orgs", "users"] {
        let base = format!("/{scope}/{owner}/projectsV2/{number}");
        let answer = api.call(Method::Get, &base, None).await?;
        match answer.status {
            404 => continue,
            _ => {
                checked(&answer, &format!("board {number}"))?;
                return Ok(base);
            }
        }
    }
    Err(Failure::NotFound(format!("board {number} for {owner}")))
}

/// The Status field's id and the id of the option named `board.status`
/// (any case).
fn status_option(fields: &Value, board: &Board) -> Result<(Value, Value), Failure> {
    let no_status = |options: Vec<String>| Failure::NoStatus {
        board: board.number,
        status: board.status.clone(),
        options,
    };
    let field = fields
        .as_array()
        .into_iter()
        .flatten()
        .find(|field| {
            field["name"]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("status"))
        })
        .ok_or_else(|| no_status(Vec::new()))?;
    let options = field["options"].as_array().cloned().unwrap_or_default();
    let name = |option: &Value| {
        option["name"]["raw"]
            .as_str()
            .or_else(|| option["name"].as_str())
            .unwrap_or_default()
            .to_string()
    };
    let wanted = board.status.trim().to_lowercase();
    match options.iter().find(|o| name(o).to_lowercase() == wanted) {
        Some(option) if !field["id"].is_null() && !option["id"].is_null() => {
            Ok((field["id"].clone(), option["id"].clone()))
        }
        _ => Err(no_status(
            options.iter().map(name).filter(|n| !n.is_empty()).collect(),
        )),
    }
}

/// The board item that holds `repository`'s `number`.
fn find_item(items: &Value, repository: &str, number: u64) -> Option<u64> {
    let suffix = format!("/repos/{}", repository.to_lowercase());
    items.as_array()?.iter().find_map(|item| {
        let content = &item["content"];
        (content["number"].as_u64() == Some(number)
            && content["repository_url"]
                .as_str()
                .is_some_and(|url| url.to_lowercase().ends_with(&suffix)))
        .then(|| item["id"].as_u64())
        .flatten()
    })
}

/// The id and kind a board adds `number` by: a pull request's own id, or
/// the issue's.
async fn content_of<A: Api>(
    api: &A,
    repository: &str,
    number: u64,
) -> Result<(u64, &'static str), Failure> {
    let what = format!("#{number} in {repository}");
    let issue = api
        .call(
            Method::Get,
            &format!("/repos/{repository}/issues/{number}"),
            None,
        )
        .await?;
    let issue = checked(&issue, &what)?;
    if issue.get("pull_request").is_some_and(|v| !v.is_null()) {
        let pull = api
            .call(
                Method::Get,
                &format!("/repos/{repository}/pulls/{number}"),
                None,
            )
            .await?;
        let pull = checked(&pull, &what)?;
        return Ok((
            pull["id"].as_u64().ok_or(Failure::Unreachable)?,
            "PullRequest",
        ));
    }
    Ok((issue["id"].as_u64().ok_or(Failure::Unreachable)?, "Issue"))
}

fn link(body: &Value) -> Option<String> {
    body["html_url"]
        .as_str()
        .filter(|url| url.starts_with("https://"))
        .map(str::to_string)
}

/// The answer's body when GitHub said yes; otherwise why not, about `what`.
fn checked(answer: &Answer, what: &str) -> Result<Value, Failure> {
    match answer.status {
        200..=299 => Ok(answer.body.clone()),
        401 => Err(Failure::AccessEnded),
        429 => Err(Failure::Limited),
        403 if github_message(&answer.body)
            .to_lowercase()
            .contains("rate limit") =>
        {
            Err(Failure::Limited)
        }
        403 => Err(Failure::Refused(format!(
            "GitHub didn't allow this on {what}."
        ))),
        404 | 410 => Err(Failure::NotFound(what.to_string())),
        422 => {
            let detail = answer.body["errors"]
                .as_array()
                .into_iter()
                .flatten()
                .find_map(|e| e["message"].as_str())
                .map(str::to_string)
                .unwrap_or_else(|| github_message(&answer.body));
            Err(Failure::Refused(if detail.is_empty() {
                format!("GitHub didn't accept this change to {what}.")
            } else {
                format!("GitHub didn't accept this change to {what}: {detail}")
            }))
        }
        500..=599 => Err(Failure::Refused(
            "GitHub had a problem answering. Try again in a minute.".into(),
        )),
        _ => Err(Failure::Unreachable),
    }
}

fn github_message(body: &Value) -> String {
    body["message"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(300)
        .collect()
}

/// GitHub over HTTPS with a token: the web chat's person's connection,
/// read for one request and never kept, or the CLI's sign-in.
#[cfg(feature = "http")]
pub struct Http {
    base: String,
    token: String,
}

#[cfg(feature = "http")]
impl Http {
    pub fn new(base: &str, token: String) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            token,
        }
    }
}

#[cfg(feature = "http")]
fn client() -> Result<&'static reqwest::Client, Failure> {
    use std::time::Duration;
    static CLIENT: std::sync::OnceLock<Result<reqwest::Client, reqwest::Error>> =
        std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("OpenAgents-github-actions")
                .build()
        })
        .as_ref()
        .map_err(|_| Failure::Unreachable)
}

#[cfg(feature = "http")]
impl Api for Http {
    fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> impl Future<Output = Result<Answer, Failure>> + Send {
        let url = format!("{}{path}", self.base);
        let token = self.token.clone();
        async move {
            let method = match method {
                Method::Get => reqwest::Method::GET,
                Method::Post => reqwest::Method::POST,
                Method::Patch => reqwest::Method::PATCH,
            };
            let mut request = client()?
                .request(method, url)
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", API_VERSION)
                .bearer_auth(token);
            if let Some(body) = body {
                request = request.json(&body);
            }
            let mut response = request.send().await.map_err(|_| Failure::Unreachable)?;
            let status = response.status().as_u16();
            let scopes = response
                .headers()
                .get("x-oauth-scopes")
                .and_then(|value| value.to_str().ok())
                .map(|value| {
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect()
                });
            const BODY_MAX: usize = 2 * 1024 * 1024;
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Unreachable)? {
                if bytes.len().saturating_add(chunk.len()) > BODY_MAX {
                    return Err(Failure::Unreachable);
                }
                bytes.extend_from_slice(&chunk);
            }
            let body = if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).unwrap_or(Value::Null)
            };
            Ok(Answer {
                status,
                scopes,
                body,
            })
        }
    }
}
