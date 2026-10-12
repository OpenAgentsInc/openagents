//! `openagents projects` (#11471): the person's projects on openagents.com
//! (the Projects list in the website's left panel), from the command line,
//! through the website's `/v1/projects` routes.
//!
//! A project is one connected GitHub repository that groups chats; it can
//! also hold Drive sources. Calls run under the
//! account's own sign-in, as `openagents keys` does: `OPENAGENTS_TOKEN`
//! with `OPENAGENTS_ORIGIN`, else `--account-dir DIR`, else the sign-in
//! `openagents login` or `coder login` saved.
//!
//! GitHub Project boards are `openagents board`, a different thing.

use coder::cli_route::tree::{Declared, Effect};
use serde_json::{Value, json};

use crate::keys::Site;
use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents projects COMMAND [--account-dir DIR]
  list        Your openagents.com projects: name, repository, chats, and last
              activity.
  show PROJECT
              One project, by name, repository, or id, with its chats.
  threads PROJECT
              The project's chats, oldest activity first.
  add REPOSITORY
              Add a project from a connected GitHub repository (owner/name).
  remove PROJECT [--detach-threads]
              Remove a project. With chats in it, --detach-threads keeps them,
              outside any project.
These are the projects in the left panel of openagents.com, signed in as
openagents login or coder login saved them, or OPENAGENTS_TOKEN. GitHub
Project boards are openagents board.";

/// What each command does, for the chat router's command tree.
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("threads", Effect::ReadOnly),
    Declared::computer("add", Effect::Publishes),
    Declared::computer("remove", Effect::Publishes),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.is_empty() || matches!(words[0].as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return if words.is_empty() {
            crate::EXIT_USAGE
        } else {
            0
        };
    }
    let args = match Args::parse(words, &["detach-threads"]) {
        Ok(args) => args,
        Err(message) => return output.usage("projects", &message, USAGE),
    };
    let positional = args.positional();
    let command = positional.first().map(String::as_str).unwrap_or_default();
    let command = if command == "chats" {
        "threads"
    } else {
        command
    };
    let (known, needed): (&[&str], usize) = match command {
        "list" => (&["account-dir"], 0),
        "show" | "threads" | "add" => (&["account-dir"], 1),
        "remove" => (&["account-dir", "detach-threads"], 1),
        _ => return output.usage("projects", &format!("unknown command `{command}`"), USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !known.contains(name))
    {
        return output.usage("projects", &format!("unknown option `--{name}`"), USAGE);
    }
    if positional.len() != needed + 1 {
        return output.usage(
            "projects",
            &format!(
                "{command} takes {}",
                if needed == 0 {
                    "no arguments"
                } else {
                    "one argument"
                }
            ),
            USAGE,
        );
    }
    let target = positional.get(1).map(String::as_str).unwrap_or_default();
    let site = match Site::open(args.option("account-dir")) {
        Ok(site) => site,
        Err(message) => return output.fail("projects", &message),
    };
    let projects = Projects(&site);
    let result = match command {
        "list" => projects.list().map(|listed| {
            output.emit(&listed, list_text);
        }),
        "show" | "threads" => projects.show(target).map(|shown| {
            output.emit(&shown, show_text);
        }),
        "add" => projects.add(target).map(|added| {
            output.emit(&added, |value| {
                format!(
                    "Added {} ({}).",
                    value["name"].as_str().unwrap_or_default(),
                    value["repository"].as_str().unwrap_or_default()
                )
            });
        }),
        "remove" => {
            projects
                .remove(target, args.switch("detach-threads"))
                .map(|(removed, project)| {
                    output.emit(&removed, |_| {
                        let name = project["name"].as_str().unwrap_or_default();
                        if project["counts"]["threads"].as_u64().unwrap_or(0) > 0 {
                            format!("Removed {name}. Its chats are still in your chat list.")
                        } else {
                            format!("Removed {name}.")
                        }
                    });
                })
        }
        _ => unreachable!("checked above"),
    };
    match result {
        Ok(()) => 0,
        Err(message) => output.fail("projects", &message),
    }
}

struct Projects<'a>(&'a Site);

impl Projects<'_> {
    fn get(&self, path: &str) -> Result<Value, String> {
        let site = self.0;
        Site::answer(
            site.http
                .get(site.url(path))
                .bearer_auth(site.saved.token())
                .send(),
        )
    }

    /// Every project, following the list's pages.
    fn list(&self) -> Result<Value, String> {
        let mut data = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let path = match &after {
                Some(after) => format!("/v1/projects?limit=200&after={after}"),
                None => "/v1/projects?limit=200".to_owned(),
            };
            let page = self.get(&path)?;
            data.extend(page["data"].as_array().cloned().unwrap_or_default());
            match page["next"].as_str() {
                Some(next) if !next.is_empty() && after.as_deref() != Some(next) => {
                    after = Some(safe(next)?.to_owned());
                }
                _ => break,
            }
        }
        Ok(json!({"data": data}))
    }

    /// The one project `target` names.
    fn find(&self, target: &str) -> Result<Value, String> {
        let listed = self.list()?;
        let projects = listed["data"].as_array().cloned().unwrap_or_default();
        find_project(&projects, target).cloned()
    }

    /// The project and its threads.
    fn show(&self, target: &str) -> Result<Value, String> {
        let project = self.find(target)?;
        let id = id(&project)?.to_owned();
        let mut threads = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let path = match &after {
                Some(after) => format!("/v1/threads?project={id}&limit=200&after={after}"),
                None => format!("/v1/threads?project={id}&limit=200"),
            };
            let page = self.get(&path)?;
            threads.extend(page["data"].as_array().cloned().unwrap_or_default());
            match page["next"].as_str() {
                Some(next) if !next.is_empty() && after.as_deref() != Some(next) => {
                    after = Some(safe(next)?.to_owned());
                }
                _ => break,
            }
        }
        Ok(json!({"project": project, "threads": threads}))
    }

    fn add(&self, repository: &str) -> Result<Value, String> {
        if repository.split('/').count() != 2 || repository.split('/').any(str::is_empty) {
            return Err(format!(
                "Name the repository as owner/name, not `{repository}`."
            ));
        }
        let site = self.0;
        Site::answer(
            site.http
                .post(site.url("/v1/projects"))
                .bearer_auth(site.saved.token())
                .json(&json!({"repository": repository}))
                .send(),
        )
    }

    fn remove(&self, target: &str, detach: bool) -> Result<(Value, Value), String> {
        let project = self.find(target)?;
        let site = self.0;
        let query = if detach { "?detach_threads=true" } else { "" };
        let removed = Site::answer(
            site.http
                .delete(site.url(&format!("/v1/projects/{}{query}", id(&project)?)))
                .bearer_auth(site.saved.token())
                .send(),
        )
        .map_err(|message| {
            if message.contains("detach_threads") {
                "This project has chats in it. Run it again with --detach-threads to keep them, outside any project.".to_owned()
            } else {
                message
            }
        })?;
        Ok((removed, project))
    }
}

/// A cursor or id safe to put in a path or query.
fn safe(value: &str) -> Result<&str, String> {
    if value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Ok(value)
    } else {
        Err("The website answered with a cursor that isn't valid.".into())
    }
}

fn id(project: &Value) -> Result<&str, String> {
    project["id"]
        .as_str()
        .filter(|id| id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        .ok_or_else(|| "The website answered without the project's id.".to_owned())
}

/// The project `target` names: its id, its name, or its repository
/// (`owner/name`); names and repositories match without regard to case.
fn find_project<'a>(projects: &'a [Value], target: &str) -> Result<&'a Value, String> {
    if let Some(project) = projects.iter().find(|p| p["id"] == target) {
        return Ok(project);
    }
    let same = |field: &str, p: &Value| {
        p[field]
            .as_str()
            .is_some_and(|value| value.eq_ignore_ascii_case(target))
    };
    let mut found: Vec<&Value> = projects.iter().filter(|p| same("name", p)).collect();
    if found.is_empty() {
        found = projects.iter().filter(|p| same("repository", p)).collect();
    }
    match found.as_slice() {
        [project] => Ok(project),
        [] => Err(format!(
            "You have no project named `{target}`. See yours with: openagents projects list"
        )),
        many => Err(format!(
            "More than one project is named `{target}`: {}. Use the repository or the id.",
            many.iter()
                .map(|p| p["repository"].as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// An RFC 3339 time as its calendar date, `2026-10-12`.
fn day(value: &Value) -> String {
    value
        .as_str()
        .and_then(|text| text.get(..10))
        .unwrap_or("-")
        .to_owned()
}

fn project_line(project: &Value) -> String {
    let chats = project["counts"]["threads"].as_u64().unwrap_or(0);
    format!(
        "{}  {}  {} chat{}  last activity {}  ({})",
        project["name"].as_str().unwrap_or_default(),
        project["repository"].as_str().unwrap_or_default(),
        chats,
        if chats == 1 { "" } else { "s" },
        day(&project["updated_at"]),
        project["id"].as_str().unwrap_or_default(),
    )
}

fn list_text(listed: &Value) -> String {
    let projects = listed["data"].as_array().cloned().unwrap_or_default();
    if projects.is_empty() {
        return "You have no projects on openagents.com yet. Add one with: openagents projects add owner/name".into();
    }
    projects
        .iter()
        .map(project_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn show_text(shown: &Value) -> String {
    let threads = shown["threads"].as_array().cloned().unwrap_or_default();
    let mut lines = vec![project_line(&shown["project"])];
    let sources = shown["project"]["counts"]["sources"].as_u64().unwrap_or(0);
    if sources > 0 {
        lines.push(format!(
            "{sources} Drive source{} (see them on the project's Sources page)",
            if sources == 1 { "" } else { "s" }
        ));
    }
    if threads.is_empty() {
        lines.push("No chats in this project yet.".into());
    }
    for thread in &threads {
        lines.push(format!(
            "  {}  {}{}  {}",
            day(&thread["updated_at"]),
            thread["title"].as_str().unwrap_or("Untitled"),
            if thread["archived"] == json!(true) {
                " (archived)"
            } else {
                ""
            },
            thread["id"].as_str().unwrap_or_default(),
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projects() -> Vec<Value> {
        vec![
            json!({"id": "prj_aaaaaaaaaaaaaaaa", "name": "finances", "repository": "chris/finances", "counts": {"threads": 2, "sources": 0}, "updated_at": "2026-10-09T12:00:00Z"}),
            json!({"id": "prj_bbbbbbbbbbbbbbbb", "name": "openagents", "repository": "OpenAgentsInc/openagents", "counts": {"threads": 1}, "updated_at": "2026-10-12T08:00:00Z"}),
        ]
    }

    #[test]
    fn a_project_is_found_by_id_name_or_repository() {
        let all = projects();
        assert_eq!(
            find_project(&all, "finances").unwrap()["id"],
            "prj_aaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            find_project(&all, "Finances").unwrap()["id"],
            "prj_aaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            find_project(&all, "openagentsinc/openagents").unwrap()["id"],
            "prj_bbbbbbbbbbbbbbbb"
        );
        assert_eq!(
            find_project(&all, "prj_bbbbbbbbbbbbbbbb").unwrap()["name"],
            "openagents"
        );
        assert!(
            find_project(&all, "taxes")
                .unwrap_err()
                .contains("openagents projects list")
        );
    }

    #[test]
    fn the_listing_reads_as_one_line_a_project() {
        let text = list_text(&json!({"data": projects()}));
        assert_eq!(
            text,
            "finances  chris/finances  2 chats  last activity 2026-10-09  (prj_aaaaaaaaaaaaaaaa)\n\
             openagents  OpenAgentsInc/openagents  1 chat  last activity 2026-10-12  (prj_bbbbbbbbbbbbbbbb)"
        );
        assert!(list_text(&json!({"data": []})).contains("openagents projects add"));
    }

    #[test]
    fn a_project_shows_its_chats() {
        let shown = json!({
            "project": projects()[0],
            "threads": [{"id": "c2", "title": "Taxes", "updated_at": "2026-10-09T10:00:00Z"}],
        });
        let text = show_text(&shown);
        assert!(text.starts_with("finances  chris/finances"), "{text}");
        assert!(text.contains("  2026-10-09  Taxes  c2"), "{text}");
    }

    #[test]
    fn a_cursor_with_other_characters_is_refused() {
        assert!(safe("prj_0123").is_ok());
        assert!(safe("x&y=1").is_err());
    }
}
