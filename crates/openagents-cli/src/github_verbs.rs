//! `openagents issue create|comment|close|reopen|list|view` and
//! `openagents project list|add|move` (#11166): GitHub issues and Project
//! boards over REST ([`github_actions::issues`]) with this computer's GitHub
//! sign-in: `GH_TOKEN` or `GITHUB_TOKEN` when set, else the GitHub CLI's
//! (`gh auth token`). Boards use the REST projectsV2 endpoints, so they
//! keep working when other tools have spent the GraphQL limit.

use std::time::Duration;

use coder::task::issue_run::Policy;
use github_actions::issues::{Github, Reply, Rest};
use serde_json::{Value, json};

use crate::Args;
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const PROJECT_USAGE: &str = "usage: openagents project COMMAND [OPTIONS]
  list [--project N] [--status NAME] [--every-repo] [--repo OWNER/NAME]
        The board's items in board order, with their status; --status keeps
        one status (\"none\" for items without one). Only this repository's
        items unless --every-repo.
  add ISSUE [--project N] [--status NAME] [--repo OWNER/NAME]
        Add the issue to the board, and set its status when --status names
        one.
  move ISSUE --status NAME [--project N] [--repo OWNER/NAME]
        Set the issue's status, such as \"In Progress\". With --project, on
        that board (adding it first); without, on every open board of the
        owner the issue is on.
The board is --project, else `project.number` in .openagents/coder-issues.json;
the status field is that file's `project.field` (\"Status\"). Uses GH_TOKEN,
GITHUB_TOKEN, or the GitHub CLI's sign-in; boards need the project scope
(`gh auth refresh -s project`).";

#[cfg(test)]
pub(crate) const PROJECT_EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("add", Effect::Publishes),
    Declared::computer("move", Effect::Publishes),
];

/// The issue commands this module runs.
pub(crate) const ISSUE_VERBS: &[&str] = &["create", "comment", "close", "reopen", "list", "view"];

/// GitHub's REST API with a token.
pub(crate) struct TokenRest {
    token: String,
    client: reqwest::blocking::Client,
}

impl TokenRest {
    /// This computer's GitHub sign-in.
    pub(crate) fn here() -> Result<Self, String> {
        let token = token().ok_or(
            "There's no GitHub sign-in on this computer. Run `gh auth login -s project`, \
             or set GH_TOKEN.",
        )?;
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("openagents-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("cannot start an HTTP client: {error}"))?;
        Ok(Self { token, client })
    }
}

fn token() -> Option<String> {
    ["GH_TOKEN", "GITHUB_TOKEN"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.trim().is_empty()))
        .or_else(|| {
            let output = std::process::Command::new("gh")
                .args(["auth", "token"])
                .env("GH_PROMPT_DISABLED", "1")
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()?;
            let token = String::from_utf8(output.stdout).ok()?.trim().to_owned();
            (output.status.success() && !token.is_empty()).then_some(token)
        })
}

impl Rest for TokenRest {
    fn send(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Reply, String> {
        let url = if path.starts_with("https://") {
            path.to_owned()
        } else {
            format!("{}{path}", github_actions::rest::API_BASE)
        };
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| format!("`{method}` is not an HTTP method"))?;
        let mut request = self
            .client
            .request(method, &url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", github_actions::rest::API_VERSION);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .map_err(|error| format!("cannot reach GitHub: {}", error.without_url()))?;
        let status = response.status().as_u16();
        let next = response
            .headers()
            .get("link")
            .and_then(|value| value.to_str().ok())
            .and_then(next_link);
        let text = response
            .text()
            .map_err(|error| format!("cannot read GitHub's answer: {}", error.without_url()))?;
        let body = if text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        Ok(Reply { status, body, next })
    }
}

/// The `rel="next"` URL of a `Link` header.
fn next_link(header: &str) -> Option<String> {
    header.split(',').find_map(|part| {
        let (url, rel) = part.split_once(';')?;
        rel.contains("rel=\"next\"").then(|| {
            url.trim()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_owned()
        })
    })
}

/// A body from `--NAME TEXT` or `--NAME-file FILE` (`-` is standard input).
fn text(args: &Args, name: &str) -> Result<Option<String>, String> {
    let file = format!("{name}-file");
    match (args.option(name), args.option(&file)) {
        (Some(_), Some(_)) => Err(format!("pass --{name} or --{file}, not both")),
        (Some(text), None) => Ok(Some(text.to_owned())),
        (None, Some("-")) => {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                .map_err(|error| format!("cannot read standard input: {error}"))?;
            Ok(Some(text))
        }
        (None, Some(path)) => std::fs::read_to_string(path)
            .map(Some)
            .map_err(|error| format!("cannot read {path}: {error}")),
        (None, None) => Ok(None),
    }
}

fn issue_number(args: &Args, command: &str) -> Result<u64, String> {
    args.positional()
        .first()
        .ok_or_else(|| format!("the issue number is required for `{command}`"))?
        .trim_start_matches('#')
        .parse()
        .map_err(|_| "the issue is a number, such as 11166".to_owned())
}

/// The board's number: `--project`, else the policy's.
fn board_number(args: &Args, policy: &Policy) -> Result<Option<u64>, String> {
    match args.option("project") {
        Some(text) => text
            .trim_start_matches('#')
            .parse()
            .map(Some)
            .map_err(|_| format!("--project takes a board number, not `{text}`")),
        None => Ok(policy.project.number),
    }
}

/// Runs `openagents issue COMMAND` for one of [`ISSUE_VERBS`].
pub(crate) fn issue(
    rest: &dyn Rest,
    command: &str,
    args: &Args,
    repository: &str,
    policy: &Policy,
) -> Result<Value, String> {
    let github = Github::new(rest, repository)?;
    let project = &policy.project;
    match command {
        "create" => {
            let title = args
                .option("title")
                .ok_or("--title is required for `create`")?;
            let body = text(args, "body")?.unwrap_or_default();
            let labels: Vec<String> = args
                .options("label")
                .iter()
                .flat_map(|labels| labels.split(','))
                .map(|label| label.trim().to_owned())
                .filter(|label| !label.is_empty())
                .collect();
            let status = args.option("status");
            let board = match (args.option("project"), status) {
                (None, None) => None,
                _ => Some(board_number(args, policy)?.ok_or(
                    "--status needs --project N (or project.number in .openagents/coder-issues.json)",
                )?),
            };
            // Read the board before creating anything, so a wrong number or
            // status stops here.
            let board = board.map(|number| github.board(number)).transpose()?;
            if let (Some(board), Some(status)) = (&board, status) {
                let field = github.field(board, &project.field)?;
                if field.option(status).is_none() {
                    return Err(github_actions::issues::missing(board, &field, status));
                }
            }
            let issue = github.create(title, &body, &labels)?;
            let mut said = json!({"command": "create", "repository": repository, "issue": issue});
            if let Some(board) = &board {
                let placed = match status {
                    Some(status) => github
                        .move_on(board, issue.number, &project.field, status, true)
                        .map(|moved| json!(moved)),
                    None => github.add(board, issue.number).map(
                        |item| json!({"project": board.number, "title": board.title, "item": item}),
                    ),
                };
                match placed {
                    Ok(placed) => said["board"] = placed,
                    Err(why) => said["board_error"] = json!(why),
                }
            }
            Ok(said)
        }
        "comment" => {
            let number = issue_number(args, command)?;
            let body = text(args, "body")?.ok_or("--body TEXT or --body-file FILE is required")?;
            let link = github.comment(number, &body)?;
            Ok(
                json!({"command": "comment", "repository": repository, "number": number, "url": link}),
            )
        }
        "close" => {
            let number = issue_number(args, command)?;
            let comment = text(args, "comment")?;
            let reason = args.option("reason").unwrap_or("completed");
            let issue = github.close(number, reason, comment.as_deref())?;
            let mut said = json!({"command": "close", "repository": repository, "issue": issue});
            // Keep the board in step (#11108): Done wherever it is.
            let moved = match args.option("project") {
                Some(_) => board_number(args, policy)?
                    .map(|n| github.board(n))
                    .transpose()
                    .and_then(|board| match board {
                        Some(board) => github
                            .move_on(&board, number, &project.field, &project.done, false)
                            .map(|moved| (moved.into_iter().collect(), Vec::new())),
                        None => Ok((Vec::new(), Vec::new())),
                    }),
                None => github.move_everywhere(number, &project.field, &project.done),
            };
            match moved {
                Ok((moved, skipped)) => {
                    said["moved"] = json!(moved);
                    if !skipped.is_empty() {
                        said["board_skipped"] = json!(skipped);
                    }
                }
                Err(why) => said["board_error"] = json!(why),
            }
            Ok(said)
        }
        "reopen" => {
            let number = issue_number(args, command)?;
            let comment = text(args, "comment")?;
            let issue = github.reopen(number, comment.as_deref())?;
            Ok(json!({"command": "reopen", "repository": repository, "issue": issue}))
        }
        "list" => {
            let labels: Vec<String> = args
                .options("label")
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            let limit = args.number("limit", 30usize)?;
            let issues = github.list(args.option("state").unwrap_or("open"), &labels, limit)?;
            Ok(json!({"command": "list", "repository": repository, "issues": issues}))
        }
        "view" => {
            let number = issue_number(args, command)?;
            let (issue, comments) = github.view(number)?;
            Ok(
                json!({"command": "view", "repository": repository, "issue": issue,
                "comments_list": comments}),
            )
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

/// Runs `openagents project COMMAND`.
pub(crate) fn project(
    rest: &dyn Rest,
    command: &str,
    args: &Args,
    repository: &str,
    policy: &Policy,
) -> Result<Value, String> {
    let github = Github::new(rest, repository)?;
    let field = &policy.project.field;
    let named_board = || -> Result<_, String> {
        let number = board_number(args, policy)?.ok_or(
            "name the board with --project N (or set project.number in .openagents/coder-issues.json)",
        )?;
        github.board(number)
    };
    match command {
        "list" => {
            let board = named_board()?;
            let items = github.items(
                &board,
                field,
                args.option("status"),
                args.switch("every-repo"),
            )?;
            Ok(
                json!({"command": "list", "repository": repository, "project": board.number,
                "title": board.title, "field": field, "items": items}),
            )
        }
        "add" => {
            let number = issue_number(args, command)?;
            let board = named_board()?;
            let said = match args.option("status") {
                Some(status) => json!(github.move_on(&board, number, field, status, true)?),
                None => {
                    let item = github.add(&board, number)?;
                    json!({"project": board.number, "title": board.title, "item": item})
                }
            };
            Ok(json!({"command": "add", "repository": repository, "number": number, "board": said}))
        }
        "move" => {
            let number = issue_number(args, command)?;
            let status = args
                .option("status")
                .ok_or("--status NAME is required for `move`")?;
            let (moved, skipped) = if args.option("project").is_some() {
                let board = named_board()?;
                (
                    github
                        .move_on(&board, number, field, status, true)?
                        .into_iter()
                        .collect(),
                    Vec::new(),
                )
            } else {
                github.move_everywhere(number, field, status)?
            };
            if moved.is_empty() {
                let mut why = format!(
                    "#{number} is on no open board with a {field} \"{status}\"; pass --project N to add it"
                );
                if !skipped.is_empty() {
                    why.push_str(&format!(" ({})", skipped.join("; ")));
                }
                return Err(why);
            }
            Ok(
                json!({"command": "move", "repository": repository, "number": number,
                "moved": moved, "board_skipped": skipped}),
            )
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

/// The plain text of a [`issue`] or [`project`] answer.
pub(crate) fn render(value: &Value) -> String {
    let issue = &value["issue"];
    let line = |issue: &Value| {
        format!(
            "#{} {} [{}]{}",
            issue["number"],
            issue["title"].as_str().unwrap_or(""),
            issue["state"].as_str().unwrap_or(""),
            labels(issue)
        )
    };
    let mut lines: Vec<String> = Vec::new();
    match value["command"].as_str() {
        Some("create") => lines.push(format!(
            "Created #{}: {}",
            issue["number"],
            issue["url"].as_str().unwrap_or("")
        )),
        Some("comment") => lines.push(format!(
            "Commented on #{}: {}",
            value["number"],
            value["url"].as_str().unwrap_or("")
        )),
        Some("close") => lines.push(format!(
            "Closed #{} ({}).",
            issue["number"],
            issue["state_reason"]
                .as_str()
                .unwrap_or("completed")
                .replace('_', " ")
        )),
        Some("reopen") => lines.push(format!("Reopened #{}.", issue["number"])),
        Some("list") if value.get("issues").is_some() => {
            let issues = value["issues"].as_array().cloned().unwrap_or_default();
            if issues.is_empty() {
                lines.push("No issues.".into());
            }
            lines.extend(issues.iter().map(line));
        }
        Some("view") => {
            lines.push(line(issue));
            lines.push(issue["url"].as_str().unwrap_or("").to_owned());
            let body = issue["body"].as_str().unwrap_or("").trim();
            if !body.is_empty() {
                lines.push(String::new());
                lines.push(body.to_owned());
            }
            for comment in value["comments_list"].as_array().into_iter().flatten() {
                lines.push(String::new());
                lines.push(format!(
                    "{} on {}:",
                    comment["author"].as_str().unwrap_or(""),
                    comment["at"].as_str().unwrap_or("")
                ));
                lines.push(comment["body"].as_str().unwrap_or("").trim().to_owned());
            }
        }
        Some("list") => {
            let items = value["items"].as_array().cloned().unwrap_or_default();
            lines.push(format!(
                "{} (project {}): {} item{}",
                value["title"].as_str().unwrap_or(""),
                value["project"],
                items.len(),
                if items.len() == 1 { "" } else { "s" }
            ));
            for item in &items {
                let name = match item["number"].as_u64() {
                    Some(number) => format!("#{number}"),
                    None => "draft".into(),
                };
                lines.push(format!(
                    "  {:<14} {name} {}",
                    item["status"].as_str().unwrap_or("(no status)"),
                    item["title"].as_str().unwrap_or("")
                ));
            }
        }
        Some("add") => {
            let board = &value["board"];
            lines.push(format!(
                "Added #{} to {}{}.",
                value["number"],
                board["title"].as_str().unwrap_or("the board"),
                board["to"]
                    .as_str()
                    .map(|to| format!(" as {to}"))
                    .unwrap_or_default()
            ));
        }
        _ => {}
    }
    let created_move = (value["command"] == "create" && value["board"]["to"].is_string())
        .then_some(&value["board"]);
    for moved in value["moved"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(created_move)
    {
        let number = value["number"]
            .as_u64()
            .or(issue["number"].as_u64())
            .unwrap_or(0);
        lines.push(format!(
            "#{number} -> {} on {}{}.",
            moved["to"].as_str().unwrap_or(""),
            moved["title"].as_str().unwrap_or("the board"),
            moved["from"]
                .as_str()
                .map(|from| format!(" (was {from})"))
                .unwrap_or_default()
        ));
    }
    if value["command"] == "create" && value["board"]["item"].is_u64() {
        lines.push(format!(
            "Added to {}.",
            value["board"]["title"].as_str().unwrap_or("the board")
        ));
    }
    for skipped in value["board_skipped"].as_array().into_iter().flatten() {
        lines.push(format!("Board skipped: {}", skipped.as_str().unwrap_or("")));
    }
    if let Some(why) = value["board_error"].as_str() {
        lines.push(format!("The board was not changed: {why}"));
    }
    lines.join("\n")
}

fn labels(issue: &Value) -> String {
    let labels: Vec<&str> = issue["labels"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if labels.is_empty() {
        String::new()
    } else {
        format!(" ({})", labels.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use github_actions::issues::fake::FakeGithub;

    const REPO: &str = "acme/app";

    fn words(text: &str) -> Args {
        let words: Vec<String> = text.split(' ').map(str::to_owned).collect();
        Args::parse(&words, &["every-repo"]).unwrap()
    }

    fn policy(number: Option<u64>) -> Policy {
        let mut policy = Policy::default();
        policy.project.number = number;
        policy
    }

    fn fake() -> FakeGithub {
        let fake = FakeGithub::new(REPO);
        fake.board(22, "V1 Launch", &["Todo", "In Progress", "Done"]);
        fake
    }

    #[test]
    fn create_puts_the_new_issue_on_the_board_with_its_status() {
        let fake = fake();
        let value = issue(
            &fake,
            "create",
            &words("--title Ship --body Text --label a,b --project 22 --status Todo"),
            REPO,
            &policy(None),
        )
        .unwrap();
        let number = value["issue"]["number"].as_u64().unwrap();
        assert_eq!(fake.status(22, number), Some(Some("Todo".into())));
        assert_eq!(fake.issue_state(number).unwrap().labels, ["a", "b"]);
        let text = render(&value);
        assert!(text.starts_with(&format!("Created #{number}")), "{text}");
        assert!(text.contains("-> Todo on V1 Launch"), "{text}");
        // A wrong status refuses before anything is created.
        let before = fake.writes().len();
        let wrong = issue(
            &fake,
            "create",
            &words("--title X --project 22 --status Shipped"),
            REPO,
            &policy(None),
        );
        assert!(wrong.unwrap_err().contains("Shipped"));
        assert_eq!(fake.writes().len(), before);
    }

    #[test]
    fn close_comments_closes_and_moves_the_board_to_done() {
        let fake = fake();
        fake.issue(5, "Five", true);
        fake.place(22, REPO, 5, Some("In Progress"));
        let value = issue(
            &fake,
            "close",
            &words("5 --comment Landed --reason completed"),
            REPO,
            &policy(None),
        )
        .unwrap();
        let state = fake.issue_state(5).unwrap();
        assert!(!state.open);
        assert_eq!(state.comments, ["Landed"]);
        assert_eq!(fake.status(22, 5), Some(Some("Done".into())));
        let text = render(&value);
        assert!(text.contains("Closed #5 (completed)."), "{text}");
        assert!(
            text.contains("#5 -> Done on V1 Launch (was In Progress)."),
            "{text}"
        );
        let value = issue(&fake, "reopen", &words("#5"), REPO, &policy(None)).unwrap();
        assert_eq!(render(&value), "Reopened #5.");
        assert!(fake.issue_state(5).unwrap().open);
    }

    #[test]
    fn comment_list_and_view_read_back() {
        let fake = fake();
        fake.issue(3, "Three", true);
        let value = issue(
            &fake,
            "comment",
            &words("3 --body Hello"),
            REPO,
            &policy(None),
        )
        .unwrap();
        assert!(render(&value).starts_with("Commented on #3"));
        let value = issue(&fake, "list", &words(""), REPO, &policy(None)).unwrap();
        assert_eq!(render(&value), "#3 Three [open]");
        let value = issue(&fake, "view", &words("3"), REPO, &policy(None)).unwrap();
        assert!(render(&value).contains("octo on"));
        assert!(issue(&fake, "comment", &words("3"), REPO, &policy(None)).is_err());
    }

    #[test]
    fn project_add_move_and_list_by_status() {
        let fake = fake();
        fake.issue(9, "Nine", true);
        fake.issue(10, "Ten", true);
        let value = project(
            &fake,
            "add",
            &words("9 --status Todo"),
            REPO,
            &policy(Some(22)),
        )
        .unwrap();
        assert!(render(&value).contains("Added #9 to V1 Launch as Todo."));
        project(&fake, "add", &words("10"), REPO, &policy(Some(22))).unwrap();
        assert_eq!(fake.status(22, 10), Some(None));
        let moved = project(
            &fake,
            "move",
            &words("9 --status done"),
            REPO,
            &policy(None),
        )
        .unwrap();
        assert!(render(&moved).contains("#9 -> Done on V1 Launch (was Todo)."));
        let listed = project(
            &fake,
            "list",
            &words("--project 22 --status none"),
            REPO,
            &policy(None),
        )
        .unwrap();
        assert_eq!(listed["items"][0]["number"], 10);
        assert!(render(&listed).contains("(no status)    #10 Ten"));
        // Not on any board and no --project: said, not silently done.
        fake.issue(11, "Eleven", true);
        let refused = project(
            &fake,
            "move",
            &words("11 --status Todo"),
            REPO,
            &policy(None),
        );
        assert!(refused.unwrap_err().contains("--project"));
        assert!(project(&fake, "list", &words(""), REPO, &policy(None)).is_err());
    }

    #[test]
    fn a_link_header_names_the_next_page() {
        let header = r#"<https://api.github.com/x?page=2>; rel="next", <https://api.github.com/x?page=5>; rel="last""#;
        assert_eq!(
            next_link(header).as_deref(),
            Some("https://api.github.com/x?page=2")
        );
        assert_eq!(next_link(r#"<https://a>; rel="prev""#), None);
    }
}
