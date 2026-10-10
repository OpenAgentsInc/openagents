//! GitHub issues and Project (v2) boards over REST (#11166): create,
//! comment, close, reopen, list, and view issues; add an issue to a
//! board, set its status, and list a board's items by status.
//!
//! Everything goes through one [`Rest`] transport, so the same verbs run
//! with this computer's GitHub token (`openagents issue|project`), with a
//! signed-in person's token on the website (#11167), and against
//! [`fake::FakeGithub`] in tests. Boards use the REST `projectsV2`
//! endpoints, not GraphQL, because other tools spend the GraphQL limit
//! (see `scripts/dev/issue-board.sh`).

use serde::Serialize;
use serde_json::{Value, json};

pub mod fake;

/// The most pages a listing follows.
const PAGES: usize = 20;

/// One HTTP answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: Value,
    /// The `rel="next"` page, when the answer has one.
    pub next: Option<String>,
}

/// Sends one request to the GitHub API. `path` starts with `/` (relative
/// to the API root, query included) or is a full URL from a `next` link.
pub trait Rest: Send + Sync {
    /// # Errors
    /// The request could not be sent or read; an HTTP error status is a
    /// [`Reply`], not an error.
    fn send(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Reply, String>;
}

/// An issue as these verbs report it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    /// `open` or `closed`.
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_reason: Option<String>,
    pub url: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub comments: u64,
    #[serde(skip)]
    pub id: u64,
}

/// One comment, for `view`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Note {
    pub author: String,
    pub at: String,
    pub body: String,
}

/// A project board: where its REST endpoints live.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Board {
    pub number: u64,
    pub title: String,
    /// `/orgs/OWNER/projectsV2/N` or `/users/OWNER/projectsV2/N`.
    #[serde(skip)]
    pub base: String,
}

/// A board's single-select status field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub id: Value,
    pub name: String,
    /// (name, option id)
    pub options: Vec<(String, String)>,
}

impl Field {
    /// The option named `name`, without regard to case.
    #[must_use]
    pub fn option(&self, name: &str) -> Option<&(String, String)> {
        self.options
            .iter()
            .find(|(option, _)| option.eq_ignore_ascii_case(name.trim()))
    }
}

/// One item of a board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BoardItem {
    pub item: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u64>,
    pub title: String,
    /// `Issue`, `PullRequest`, or `DraftIssue`.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// What a status move did on one board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Moved {
    pub project: u64,
    pub title: String,
    /// The status it had, when it had one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub to: String,
    /// It was added to the board first.
    pub added: bool,
}

/// GitHub for one repository (`owner/name`).
pub struct Github<'a> {
    rest: &'a dyn Rest,
    repository: String,
    owner: String,
}

impl<'a> Github<'a> {
    /// # Errors
    /// `repository` is not `owner/name`.
    pub fn new(rest: &'a dyn Rest, repository: &str) -> Result<Self, String> {
        let (owner, name) = repository
            .split_once('/')
            .filter(|(owner, name)| !owner.is_empty() && !name.is_empty() && !name.contains('/'))
            .ok_or_else(|| format!("`{repository}` is not OWNER/NAME"))?;
        Ok(Self {
            rest,
            repository: format!("{owner}/{name}"),
            owner: owner.to_owned(),
        })
    }

    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }

    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let reply = self.rest.send(method, path, body)?;
        if (200..300).contains(&reply.status) {
            Ok(reply.body)
        } else {
            Err(refusal(&reply, path))
        }
    }

    /// Every page of a listing, up to [`PAGES`].
    fn all(&self, path: &str) -> Result<Vec<Value>, String> {
        let mut out = Vec::new();
        let mut next = Some(path.to_owned());
        for _ in 0..PAGES {
            let Some(path) = next.take() else { break };
            let reply = self.rest.send("GET", &path, None)?;
            if !(200..300).contains(&reply.status) {
                return Err(refusal(&reply, &path));
            }
            out.extend(reply.body.as_array().cloned().unwrap_or_default());
            next = reply.next;
        }
        Ok(out)
    }

    // ----------------------------------------------------------- issues

    /// # Errors
    /// GitHub refused it.
    pub fn create(&self, title: &str, body: &str, labels: &[String]) -> Result<Issue, String> {
        if title.trim().is_empty() {
            return Err("an issue needs a title".into());
        }
        let mut request = json!({"title": title, "body": body});
        if !labels.is_empty() {
            request["labels"] = json!(labels);
        }
        let value = self.call(
            "POST",
            &format!("/repos/{}/issues", self.repository),
            Some(&request),
        )?;
        Ok(issue_of(&value, false))
    }

    /// Posts `body` as a comment; returns the comment's link.
    ///
    /// # Errors
    /// GitHub refused it.
    pub fn comment(&self, number: u64, body: &str) -> Result<String, String> {
        if body.trim().is_empty() {
            return Err("a comment needs some text".into());
        }
        let value = self.call(
            "POST",
            &format!("/repos/{}/issues/{number}/comments", self.repository),
            Some(&json!({"body": body})),
        )?;
        Ok(value["html_url"].as_str().unwrap_or_default().to_owned())
    }

    /// Closes the issue as `completed` or `not_planned`, after `comment`
    /// when given.
    ///
    /// # Errors
    /// An unknown reason, or GitHub refused it.
    pub fn close(&self, number: u64, reason: &str, comment: Option<&str>) -> Result<Issue, String> {
        let reason = match reason {
            "completed" | "done" => "completed",
            "not_planned" | "not-planned" | "wontfix" => "not_planned",
            other => {
                return Err(format!(
                    "the reason is completed or not_planned, not `{other}`"
                ));
            }
        };
        if let Some(comment) = comment.filter(|text| !text.trim().is_empty()) {
            self.comment(number, comment)?;
        }
        self.state(number, json!({"state": "closed", "state_reason": reason}))
    }

    /// Reopens the issue, after `comment` when given.
    ///
    /// # Errors
    /// GitHub refused it.
    pub fn reopen(&self, number: u64, comment: Option<&str>) -> Result<Issue, String> {
        if let Some(comment) = comment.filter(|text| !text.trim().is_empty()) {
            self.comment(number, comment)?;
        }
        self.state(number, json!({"state": "open"}))
    }

    fn state(&self, number: u64, change: Value) -> Result<Issue, String> {
        let value = self.call(
            "PATCH",
            &format!("/repos/{}/issues/{number}", self.repository),
            Some(&change),
        )?;
        Ok(issue_of(&value, false))
    }

    /// The repository's issues (pull requests left out), newest first.
    ///
    /// # Errors
    /// GitHub refused it.
    pub fn list(&self, state: &str, labels: &[String], limit: usize) -> Result<Vec<Issue>, String> {
        let state = match state {
            "open" | "closed" | "all" => state,
            other => return Err(format!("--state is open, closed, or all, not `{other}`")),
        };
        let limit = limit.clamp(1, 1000);
        let mut path = format!(
            "/repos/{}/issues?state={state}&per_page={}",
            self.repository,
            limit.min(100)
        );
        if !labels.is_empty() {
            path.push_str(&format!("&labels={}", query(&labels.join(","))));
        }
        let mut out = Vec::new();
        let mut next = Some(path);
        for _ in 0..PAGES {
            let Some(page) = next.take() else { break };
            let reply = self.rest.send("GET", &page, None)?;
            if !(200..300).contains(&reply.status) {
                return Err(refusal(&reply, &page));
            }
            out.extend(
                reply
                    .body
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|value| value.get("pull_request").is_none())
                    .map(|value| issue_of(value, false)),
            );
            if out.len() >= limit {
                break;
            }
            next = reply.next;
        }
        out.truncate(limit);
        Ok(out)
    }

    /// One issue with its body and comments.
    ///
    /// # Errors
    /// GitHub refused it.
    pub fn view(&self, number: u64) -> Result<(Issue, Vec<Note>), String> {
        let value = self.call(
            "GET",
            &format!("/repos/{}/issues/{number}", self.repository),
            None,
        )?;
        let issue = issue_of(&value, true);
        let notes = if issue.comments == 0 {
            Vec::new()
        } else {
            self.all(&format!(
                "/repos/{}/issues/{number}/comments?per_page=100",
                self.repository
            ))?
            .iter()
            .map(|value| Note {
                author: value["user"]["login"].as_str().unwrap_or_default().into(),
                at: value["created_at"].as_str().unwrap_or_default().into(),
                body: value["body"].as_str().unwrap_or_default().into(),
            })
            .collect()
        };
        Ok((issue, notes))
    }

    // ----------------------------------------------------------- boards

    /// The owner's board numbered `number` (an organization's, else a
    /// user's).
    ///
    /// # Errors
    /// Neither has it, or the token may not read projects.
    pub fn board(&self, number: u64) -> Result<Board, String> {
        let mut why = String::new();
        for scope in ["orgs", "users"] {
            let base = format!("/{scope}/{}/projectsV2/{number}", self.owner);
            let reply = self.rest.send("GET", &base, None)?;
            if (200..300).contains(&reply.status) {
                return Ok(Board {
                    number,
                    title: reply.body["title"].as_str().unwrap_or_default().into(),
                    base,
                });
            }
            why = refusal(&reply, &base);
        }
        Err(why)
    }

    /// The owner's open boards.
    ///
    /// # Errors
    /// The token may not list them.
    pub fn open_boards(&self) -> Result<Vec<Board>, String> {
        let mut why = String::new();
        for scope in ["orgs", "users"] {
            let path = format!("/{scope}/{}/projectsV2?per_page=100", self.owner);
            match self.all(&path) {
                Ok(projects) => {
                    return Ok(projects
                        .iter()
                        .filter(|project| project["state"].as_str() != Some("closed"))
                        .filter_map(|project| {
                            let number = project["number"].as_u64()?;
                            Some(Board {
                                number,
                                title: project["title"].as_str().unwrap_or_default().into(),
                                base: format!("/{scope}/{}/projectsV2/{number}", self.owner),
                            })
                        })
                        .collect());
                }
                Err(error) => why = error,
            }
        }
        Err(why)
    }

    /// The board's single-select field named `name`.
    ///
    /// # Errors
    /// The board has no such field, or it cannot be read.
    pub fn field(&self, board: &Board, name: &str) -> Result<Field, String> {
        let fields = self.all(&format!("{}/fields?per_page=100", board.base))?;
        let found = fields
            .iter()
            .find(|field| {
                plain_name(&field["name"]).is_some_and(|field| field.eq_ignore_ascii_case(name))
            })
            .ok_or_else(|| format!("project {} has no \"{name}\" field", board.number))?;
        Ok(Field {
            id: found["id"].clone(),
            name: plain_name(&found["name"]).unwrap_or(name).to_owned(),
            options: found["options"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|option| {
                    let id = match &option["id"] {
                        Value::String(id) => id.clone(),
                        Value::Number(id) => id.to_string(),
                        _ => return None,
                    };
                    Some((plain_name(&option["name"])?.to_owned(), id))
                })
                .collect(),
        })
    }

    /// The issue's item on `board`, with `field`'s value when given.
    ///
    /// # Errors
    /// The board cannot be read.
    pub fn find(
        &self,
        board: &Board,
        number: u64,
        field: Option<&Field>,
    ) -> Result<Option<BoardItem>, String> {
        let mut path = format!("{}/items?per_page=100&q={number}", board.base);
        if let Some(field) = field {
            path.push_str(&format!("&fields[]={}", id_text(&field.id)));
        }
        let items = self.call("GET", &path, None)?;
        Ok(items
            .as_array()
            .into_iter()
            .flatten()
            .map(|item| item_of(item, field))
            .find(|item| {
                item.number == Some(number)
                    && item
                        .repository
                        .as_deref()
                        .is_some_and(|repo| repo.eq_ignore_ascii_case(&self.repository))
            }))
    }

    /// Adds the issue to `board`; returns its item id. An issue already
    /// on the board keeps its item.
    ///
    /// # Errors
    /// GitHub refused it.
    pub fn add(&self, board: &Board, number: u64) -> Result<u64, String> {
        let issue = self.call(
            "GET",
            &format!("/repos/{}/issues/{number}", self.repository),
            None,
        )?;
        let id = issue["id"]
            .as_u64()
            .ok_or_else(|| format!("#{number} has no id"))?;
        let item = self.call(
            "POST",
            &format!("{}/items", board.base),
            Some(&json!({"type": "Issue", "id": id})),
        )?;
        item["id"]
            .as_u64()
            .ok_or_else(|| "GitHub returned no item id".to_owned())
    }

    /// Sets the item's `field` to the option named `status`.
    ///
    /// # Errors
    /// The field has no such option, or GitHub refused it.
    pub fn set(&self, board: &Board, item: u64, field: &Field, status: &str) -> Result<(), String> {
        let (_, option) = field
            .option(status)
            .ok_or_else(|| missing(board, field, status))?;
        self.call(
            "PATCH",
            &format!("{}/items/{item}", board.base),
            Some(&json!({"fields": [{"id": field.id, "value": option}]})),
        )
        .map(|_| ())
    }

    /// Moves the issue to `status` on `board`, adding it first when `add`
    /// and it is not on the board. `Ok(None)` when it is not on the board
    /// and `add` is false.
    ///
    /// # Errors
    /// The board, its field, or the move failed.
    pub fn move_on(
        &self,
        board: &Board,
        number: u64,
        field_name: &str,
        status: &str,
        add: bool,
    ) -> Result<Option<Moved>, String> {
        let field = self.field(board, field_name)?;
        if field.option(status).is_none() {
            return Err(missing(board, &field, status));
        }
        let (item, from, added) = match self.find(board, number, Some(&field))? {
            Some(item) => (item.item, item.status, false),
            None if add => (self.add(board, number)?, None, true),
            None => return Ok(None),
        };
        self.set(board, item, &field, status)?;
        Ok(Some(Moved {
            project: board.number,
            title: board.title.clone(),
            from,
            to: field
                .option(status)
                .map_or_else(|| status.to_owned(), |(name, _)| name.clone()),
            added,
        }))
    }

    /// Moves the issue to `status` on every open board of the owner it
    /// is on (#11108). A board that cannot be read or has no such status
    /// is reported in the second list and skipped.
    ///
    /// # Errors
    /// The boards cannot be listed.
    pub fn move_everywhere(
        &self,
        number: u64,
        field_name: &str,
        status: &str,
    ) -> Result<(Vec<Moved>, Vec<String>), String> {
        let mut moved = Vec::new();
        let mut skipped = Vec::new();
        for board in self.open_boards()? {
            match self.move_on(&board, number, field_name, status, false) {
                Ok(Some(done)) => moved.push(done),
                Ok(None) => {}
                Err(why) => skipped.push(why),
            }
        }
        Ok((moved, skipped))
    }

    /// The board's items with `field`'s value, in board order; only those
    /// with status `status` when given, and only this repository's issues
    /// and pull requests unless `every_repository`.
    ///
    /// # Errors
    /// The board cannot be read.
    pub fn items(
        &self,
        board: &Board,
        field_name: &str,
        status: Option<&str>,
        every_repository: bool,
    ) -> Result<Vec<BoardItem>, String> {
        let field = self.field(board, field_name)?;
        if let Some(status) = status
            && !status.eq_ignore_ascii_case("none")
            && field.option(status).is_none()
        {
            return Err(missing(board, &field, status));
        }
        let items = self.all(&format!(
            "{}/items?per_page=100&fields[]={}",
            board.base,
            id_text(&field.id)
        ))?;
        Ok(items
            .iter()
            .map(|item| item_of(item, Some(&field)))
            .filter(|item| {
                every_repository
                    || item.kind == "DraftIssue"
                    || item
                        .repository
                        .as_deref()
                        .is_some_and(|repo| repo.eq_ignore_ascii_case(&self.repository))
            })
            .filter(|item| match status {
                None => true,
                Some(want) if want.eq_ignore_ascii_case("none") => item.status.is_none(),
                Some(want) => item
                    .status
                    .as_deref()
                    .is_some_and(|have| have.eq_ignore_ascii_case(want)),
            })
            .collect())
    }
}

/// What to say for an HTTP error `reply` to `path`, with a hint when a
/// board refused because the token lacks the `project` scope.
fn refusal(reply: &Reply, path: &str) -> String {
    let message = reply.body["message"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| reply.body.as_str().map(str::to_owned))
        .unwrap_or_default();
    let details: Vec<String> = reply.body["errors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|error| {
            error["message"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| error["code"].as_str().map(str::to_owned))
        })
        .collect();
    let mut text = format!("GitHub answered {}", reply.status);
    if !message.is_empty() {
        text.push_str(&format!(": {message}"));
    }
    if !details.is_empty() {
        text.push_str(&format!(" ({})", details.join("; ")));
    }
    if path.contains("projectsV2") && matches!(reply.status, 401 | 403 | 404) {
        text.push_str(
            ". Boards need a token with the project scope; with the GitHub CLI run \
             `gh auth refresh -s project`.",
        );
    } else if reply.status == 401 {
        text.push_str(". Sign in again with `gh auth login`.");
    }
    text
}

/// Why `status` cannot be set: the board's field has no such option.
#[must_use]
pub fn missing(board: &Board, field: &Field, status: &str) -> String {
    format!(
        "project {} has no {} \"{status}\"; it has {}",
        board.number,
        field.name,
        field
            .options
            .iter()
            .map(|(name, _)| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn issue_of(value: &Value, with_body: bool) -> Issue {
    Issue {
        number: value["number"].as_u64().unwrap_or_default(),
        title: value["title"].as_str().unwrap_or_default().into(),
        state: value["state"].as_str().unwrap_or_default().into(),
        state_reason: value["state_reason"].as_str().map(str::to_owned),
        url: value["html_url"].as_str().unwrap_or_default().into(),
        labels: value["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|label| label["name"].as_str().or_else(|| label.as_str()))
            .map(str::to_owned)
            .collect(),
        assignees: value["assignees"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|user| user["login"].as_str().map(str::to_owned))
            .collect(),
        body: with_body.then(|| value["body"].as_str().unwrap_or_default().to_owned()),
        comments: value["comments"].as_u64().unwrap_or_default(),
        id: value["id"].as_u64().unwrap_or_default(),
    }
}

/// A REST name, which is `{"raw": ..}` or a plain string.
fn plain_name(value: &Value) -> Option<&str> {
    value["raw"].as_str().or_else(|| value.as_str())
}

fn id_text(id: &Value) -> String {
    match id {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn item_of(item: &Value, field: Option<&Field>) -> BoardItem {
    let content = &item["content"];
    let repository = content["repository_url"]
        .as_str()
        .and_then(|url| url.split("/repos/").nth(1))
        .map(str::to_owned);
    let status = field.and_then(|field| {
        item["fields"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|value| {
                value["id"] == field.id
                    || plain_name(&value["name"])
                        .is_some_and(|name| name.eq_ignore_ascii_case(&field.name))
            })
            .and_then(|value| {
                plain_name(&value["value"]["name"]).or_else(|| value["value"].as_str())
            })
            .map(str::to_owned)
    });
    BoardItem {
        item: item["id"].as_u64().unwrap_or_default(),
        number: content["number"].as_u64(),
        title: plain_name(&content["title"]).unwrap_or_default().to_owned(),
        kind: item["content_type"].as_str().unwrap_or("Issue").to_owned(),
        state: content["state"].as_str().map(str::to_owned),
        repository,
        status,
    }
}

/// `text` percent-encoded for a query value.
fn query(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

#[cfg(test)]
#[path = "issues_tests.rs"]
mod tests;
