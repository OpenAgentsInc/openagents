//! An in-memory GitHub for the issue and board verbs' tests. It answers
//! the REST paths [`super::Github`] sends with GitHub's JSON shapes
//! (recorded from the live API) and keeps every write, so a test reads
//! back what a verb did. It never touches the network.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::{Value, json};

use super::{Reply, Rest};

/// One issue.
#[derive(Clone, Debug, Default)]
pub struct FakeIssue {
    pub title: String,
    pub body: String,
    pub open: bool,
    pub state_reason: Option<String>,
    pub labels: Vec<String>,
    pub comments: Vec<String>,
    pub pull_request: bool,
}

/// One board item.
#[derive(Clone, Debug)]
pub struct FakeItem {
    pub id: u64,
    pub repository: String,
    pub number: u64,
    pub status: Option<String>,
}

/// One board, with a Status field.
#[derive(Clone, Debug)]
pub struct FakeBoard {
    pub number: u64,
    pub title: String,
    pub open: bool,
    pub statuses: Vec<String>,
    pub items: Vec<FakeItem>,
}

#[derive(Debug, Default)]
struct World {
    issues: BTreeMap<u64, FakeIssue>,
    boards: Vec<FakeBoard>,
    next_item: u64,
    calls: Vec<String>,
}

/// GitHub for one repository and its owner's boards.
#[derive(Debug)]
pub struct FakeGithub {
    pub repository: String,
    /// The token may not read projects: every board path answers 404.
    pub no_project_scope: bool,
    world: Mutex<World>,
}

const STATUS_FIELD: u64 = 7;

impl FakeGithub {
    #[must_use]
    pub fn new(repository: &str) -> Self {
        Self {
            repository: repository.into(),
            no_project_scope: false,
            world: Mutex::new(World {
                next_item: 9000,
                ..World::default()
            }),
        }
    }

    fn world(&self) -> std::sync::MutexGuard<'_, World> {
        self.world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Adds issue `number`.
    pub fn issue(&self, number: u64, title: &str, open: bool) {
        self.world().issues.insert(
            number,
            FakeIssue {
                title: title.into(),
                open,
                ..FakeIssue::default()
            },
        );
    }

    /// Adds a board with `statuses` as its Status options.
    pub fn board(&self, number: u64, title: &str, statuses: &[&str]) {
        self.world().boards.push(FakeBoard {
            number,
            title: title.into(),
            open: true,
            statuses: statuses.iter().map(|s| (*s).to_owned()).collect(),
            items: Vec::new(),
        });
    }

    /// Puts issue `number` of `repository` on board `board` with `status`.
    pub fn place(&self, board: u64, repository: &str, number: u64, status: Option<&str>) {
        let mut world = self.world();
        world.next_item += 1;
        let id = world.next_item;
        let board = world
            .boards
            .iter_mut()
            .find(|b| b.number == board)
            .expect("board exists");
        board.items.push(FakeItem {
            id,
            repository: repository.into(),
            number,
            status: status.map(str::to_owned),
        });
    }

    #[must_use]
    pub fn issue_state(&self, number: u64) -> Option<FakeIssue> {
        self.world().issues.get(&number).cloned()
    }

    /// The issue's status on `board`: `None` off the board, `Some(None)`
    /// on it with no status.
    #[must_use]
    pub fn status(&self, board: u64, number: u64) -> Option<Option<String>> {
        self.world()
            .boards
            .iter()
            .find(|b| b.number == board)?
            .items
            .iter()
            .find(|item| item.number == number && item.repository == self.repository)
            .map(|item| item.status.clone())
    }

    /// Every request so far, as `METHOD path`.
    #[must_use]
    pub fn calls(&self) -> Vec<String> {
        self.world().calls.clone()
    }

    /// The requests that changed something.
    #[must_use]
    pub fn writes(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter(|call| !call.starts_with("GET "))
            .collect()
    }
}

fn reply(status: u16, body: Value) -> Result<Reply, String> {
    Ok(Reply {
        status,
        body,
        next: None,
    })
}

fn not_found() -> Result<Reply, String> {
    reply(
        404,
        json!({"message": "Not Found", "documentation_url": "https://docs.github.com/rest"}),
    )
}

fn option_id(name: &str) -> String {
    format!("opt-{}", name.to_ascii_lowercase().replace(' ', "-"))
}

impl FakeGithub {
    fn issue_json(&self, number: u64, issue: &FakeIssue) -> Value {
        let mut value = json!({
            "id": 100_000 + number,
            "number": number,
            "title": issue.title,
            "body": issue.body,
            "state": if issue.open { "open" } else { "closed" },
            "state_reason": issue.state_reason,
            "html_url": format!("https://github.com/{}/issues/{number}", self.repository),
            "labels": issue.labels.iter().map(|name| json!({"name": name})).collect::<Vec<_>>(),
            "assignees": [],
            "comments": issue.comments.len(),
        });
        if issue.pull_request {
            value["pull_request"] = json!({"url": "x"});
        }
        value
    }

    fn board_json(board: &FakeBoard) -> Value {
        json!({"id": 50 + board.number, "number": board.number, "title": board.title,
            "state": if board.open { "open" } else { "closed" }})
    }

    fn fields_json(board: &FakeBoard) -> Value {
        json!([
            {"id": 1, "name": "Title", "data_type": "title"},
            {"id": STATUS_FIELD, "name": "Status", "data_type": "single_select",
             "options": board.statuses.iter().map(|name| json!({
                 "id": option_id(name), "name": {"raw": name, "html": name}
             })).collect::<Vec<_>>()}
        ])
    }

    fn item_json(world: &World, item: &FakeItem, with_status: bool) -> Value {
        let title = world
            .issues
            .get(&item.number)
            .map(|issue| issue.title.clone())
            .unwrap_or_default();
        let open = world
            .issues
            .get(&item.number)
            .is_none_or(|issue| issue.open);
        let mut fields = vec![json!({"id": 1, "name": "Title", "data_type": "title",
            "value": {"raw": title, "html": title}})];
        if with_status {
            fields.push(
                json!({"id": STATUS_FIELD, "name": "Status", "data_type": "single_select",
                "value": item.status.as_ref().map(|name| json!({
                    "id": option_id(name), "name": {"raw": name, "html": name}
                }))}),
            );
        }
        json!({
            "id": item.id,
            "node_id": format!("PVTI_{}", item.id),
            "content_type": "Issue",
            "content": {
                "number": item.number,
                "title": title,
                "state": if open { "open" } else { "closed" },
                "repository_url": format!("https://api.github.com/repos/{}", item.repository),
            },
            "fields": fields,
        })
    }
}

impl Rest for FakeGithub {
    #[allow(clippy::too_many_lines)]
    fn send(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Reply, String> {
        let path = path.trim_start_matches("https://api.github.com");
        self.world().calls.push(format!("{method} {path}"));
        let (route, query) = path.split_once('?').unwrap_or((path, ""));
        let params: Vec<(&str, &str)> = query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .collect();
        let param = |name: &str| {
            params
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value)
        };
        let words: Vec<&str> = route.trim_matches('/').split('/').collect();
        let repo = format!("repos/{}", self.repository);
        let body = body.cloned().unwrap_or(Value::Null);

        // Issues.
        if route.starts_with(&format!("/{repo}/issues")) {
            let rest = &words[4..];
            let mut world = self.world();
            return match (method, rest) {
                ("POST", []) => {
                    let number = world.issues.keys().max().copied().unwrap_or(0).max(1000) + 1;
                    let issue = FakeIssue {
                        title: body["title"].as_str().unwrap_or_default().into(),
                        body: body["body"].as_str().unwrap_or_default().into(),
                        open: true,
                        labels: body["labels"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect(),
                        ..FakeIssue::default()
                    };
                    let value = self.issue_json(number, &issue);
                    world.issues.insert(number, issue);
                    reply(201, value)
                }
                ("GET", []) => {
                    let state = param("state").unwrap_or("open");
                    let per_page: usize =
                        param("per_page").and_then(|n| n.parse().ok()).unwrap_or(30);
                    let page: usize = param("page").and_then(|n| n.parse().ok()).unwrap_or(1);
                    let labels: Vec<String> = param("labels")
                        .map(|text| {
                            text.replace("%20", " ")
                                .split(',')
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default();
                    let all: Vec<Value> = world
                        .issues
                        .iter()
                        .rev()
                        .filter(|(_, issue)| match state {
                            "all" => true,
                            "closed" => !issue.open,
                            _ => issue.open,
                        })
                        .filter(|(_, issue)| {
                            labels.iter().all(|label| issue.labels.contains(label))
                        })
                        .map(|(number, issue)| self.issue_json(*number, issue))
                        .collect();
                    let start = (page - 1) * per_page;
                    let chunk: Vec<Value> =
                        all.iter().skip(start).take(per_page).cloned().collect();
                    let next = (start + per_page < all.len()).then(|| {
                        format!(
                            "/{repo}/issues?state={state}&per_page={per_page}&page={}",
                            page + 1
                        )
                    });
                    Ok(Reply {
                        status: 200,
                        body: Value::Array(chunk),
                        next,
                    })
                }
                (_, [number, tail @ ..]) => {
                    let Ok(number) = number.parse::<u64>() else {
                        return not_found();
                    };
                    let Some(issue) = world.issues.get_mut(&number) else {
                        return not_found();
                    };
                    match (method, tail) {
                        ("GET", []) => {
                            let issue = issue.clone();
                            reply(200, self.issue_json(number, &issue))
                        }
                        ("PATCH", []) => {
                            if let Some(state) = body["state"].as_str() {
                                issue.open = state == "open";
                                issue.state_reason = body["state_reason"]
                                    .as_str()
                                    .map(str::to_owned)
                                    .or_else(|| (state == "open").then(|| "reopened".into()));
                            }
                            let issue = issue.clone();
                            reply(200, self.issue_json(number, &issue))
                        }
                        ("POST", ["comments"]) => {
                            issue
                                .comments
                                .push(body["body"].as_str().unwrap_or_default().into());
                            let id = issue.comments.len();
                            reply(
                                201,
                                json!({"id": id, "html_url": format!(
                                    "https://github.com/{}/issues/{number}#issuecomment-{id}",
                                    self.repository)}),
                            )
                        }
                        ("GET", ["comments"]) => reply(
                            200,
                            Value::Array(
                                issue
                                    .comments
                                    .iter()
                                    .map(|text| {
                                        json!({"user": {"login": "octo"},
                                        "created_at": "2026-10-09T12:00:00Z", "body": text})
                                    })
                                    .collect(),
                            ),
                        ),
                        _ => not_found(),
                    }
                }
                _ => not_found(),
            };
        }

        // Boards: /orgs/OWNER/projectsV2[/N[/fields|/items[/ID]]].
        let owner = self.repository.split('/').next().unwrap_or_default();
        if words.len() >= 3 && words[2] == "projectsV2" {
            if self.no_project_scope || words[0] != "orgs" || words[1] != owner {
                return not_found();
            }
            let mut world = self.world();
            if words.len() == 3 {
                return reply(
                    200,
                    Value::Array(world.boards.iter().map(Self::board_json).collect()),
                );
            }
            let Ok(number) = words[3].parse::<u64>() else {
                return not_found();
            };
            let Some(index) = world.boards.iter().position(|b| b.number == number) else {
                return not_found();
            };
            return match (method, &words[4..]) {
                ("GET", []) => reply(200, Self::board_json(&world.boards[index])),
                ("GET", ["fields"]) => reply(200, Self::fields_json(&world.boards[index])),
                ("GET", ["items"]) => {
                    let with_status = param("fields[]")
                        .or_else(|| param("fields%5B%5D"))
                        .is_some_and(|id| id == STATUS_FIELD.to_string());
                    let q = param("q");
                    let items: Vec<Value> = world.boards[index]
                        .items
                        .iter()
                        .filter(|item| {
                            q.is_none_or(|q| {
                                item.number.to_string().contains(q)
                                    || world
                                        .issues
                                        .get(&item.number)
                                        .is_some_and(|issue| issue.title.contains(q))
                            })
                        })
                        .map(|item| Self::item_json(&world, item, with_status))
                        .collect();
                    reply(200, Value::Array(items))
                }
                ("POST", ["items"]) => {
                    let Some(id) = body["id"].as_u64() else {
                        return reply(422, json!({"message": "Validation Failed"}));
                    };
                    let issue = id.saturating_sub(100_000);
                    if body["type"] != "Issue" || !world.issues.contains_key(&issue) {
                        return reply(422, json!({"message": "Validation Failed"}));
                    }
                    let repository = self.repository.clone();
                    if let Some(item) = world.boards[index]
                        .items
                        .iter()
                        .find(|item| item.number == issue && item.repository == repository)
                    {
                        let value = Self::item_json(&world, item, false);
                        return reply(201, value);
                    }
                    world.next_item += 1;
                    let item = FakeItem {
                        id: world.next_item,
                        repository,
                        number: issue,
                        status: None,
                    };
                    let value = Self::item_json(&world, &item, false);
                    world.boards[index].items.push(item);
                    reply(201, value)
                }
                ("PATCH", ["items", id]) => {
                    let Ok(id) = id.parse::<u64>() else {
                        return not_found();
                    };
                    let change = &body["fields"][0];
                    if change["id"] != json!(STATUS_FIELD) {
                        return reply(422, json!({"message": "Validation Failed"}));
                    }
                    let option = change["value"].as_str().unwrap_or_default().to_owned();
                    let board = &mut world.boards[index];
                    let Some(name) = board
                        .statuses
                        .iter()
                        .find(|name| option_id(name) == option)
                        .cloned()
                    else {
                        return reply(422, json!({"message": "Validation Failed"}));
                    };
                    let Some(item) = board.items.iter_mut().find(|item| item.id == id) else {
                        return not_found();
                    };
                    item.status = Some(name);
                    let item = item.clone();
                    let value = Self::item_json(&world, &item, true);
                    reply(200, value)
                }
                _ => not_found(),
            };
        }
        not_found()
    }
}
