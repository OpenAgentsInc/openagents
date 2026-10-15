//! `openagents projects` (#11471): the person's projects on openagents.com
//! (the Projects list in the website's left panel), from the command line,
//! through the website's `/v1/projects` routes.
//!
//! A project groups chats and keeps instructions, files, and notes. It
//! needs only a name: it can have no GitHub repository, one, or several
//! with one of them the primary (the one a chat started in the project
//! uses). The same repository can be in several projects, and two projects
//! can share a name; a name that matches more than one project is never
//! guessed. It can also hold Drive sources. Calls run under the
//! account's own sign-in, as `openagents keys` does: `OPENAGENTS_TOKEN`
//! with `OPENAGENTS_ORIGIN`, else `--account-dir DIR`, else the sign-in
//! `openagents login` or `coder login` saved.
//!
//! A project also keeps private files and text documents (`files`,
//! `notes`). In a project whose protection is `only_me`, `files add` locks
//! each file on this computer with the person's vault (`openagents vault
//! add --project`) before anything is sent; the website records only that
//! the locked file belongs to the project.
//!
//! GitHub Project boards are `openagents board`, a different thing.

mod transfer;

use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use serde_json::{Value, json};

use crate::keys::Site;
use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents projects COMMAND [--account-dir DIR]
  list        Your openagents.com projects: name, repositories, chats, and
              last activity.
  show PROJECT
              One project, by name, repository, or id, with its repositories
              and chats.
  threads PROJECT
              The project's chats, oldest activity first.
  read PROJECT [CHAT]
              Every chat in the project, message by message, or only CHAT
              (its id, a unique id prefix, or its title). One chat also
              reads with openagents chat read --thread ID.
  create NAME [--repo OWNER/NAME]... [--instructions TEXT] [--protection standard|only_me]
              Make a new project. It needs only a name. Each --repo adds a
              connected GitHub repository; the first is the primary.
  add REPOSITORY [--protection standard|only_me]
              Older form of create: a new project named after the
              repository (owner/name), with it as the primary.
  edit PROJECT [--name NAME] [--instructions TEXT] [--protection standard|only_me]
              Rename the project, set its instructions (\"\" clears them), or
              choose who can open its new files. standard: openagents.com
              keeps them for your account. only_me: each file is locked on
              your computer before it's sent, and only your devices can open
              it (set up once with openagents vault setup).
  repo add PROJECT REPOSITORY [--primary]
              Add a connected GitHub repository (owner/name) to the project.
              Its first repository is the primary; --primary makes this one
              the primary.
  repo remove PROJECT REPOSITORY
              Take a repository out of the project. When it was the primary,
              the next one becomes the primary.
  repo primary PROJECT REPOSITORY
              Make one of the project's repositories the primary. A chat
              started in the project uses its primary repository.
  move CHAT PROJECT
              Move a chat into a project. CHAT is its id, a unique id
              prefix, or its title; PROJECT its name, repository, or id.
  move CHAT --none
              Take a chat out of its project.
  delete PROJECT [--yes] [--detach-threads]
              Delete a project for good. It asks you to type the project's
              name first; --yes skips that. With chats in it,
              --detach-threads keeps them, outside any project.
  members PROJECT
              Who is in the project and what each can do. The owner also
              sees the invitations nobody has used yet.
  invite PROJECT [--github USERNAME] [--role editor|viewer] [--ttl DURATION]
              Make a link that lets one person join, shown only this once.
              With --github, the invitation also waits on that GitHub
              person's Projects page on openagents.com, with Join and
              Decline, so there's no link to send.
              An editor (the default) adds chats, files, and notes; a viewer
              reads them. --ttl is how long the link works, like 30m, 12h,
              or 7d (the default), from 10 minutes to 30 days. Only the
              owner invites.
  join LINK
              Join a project with the invitation link (or its code)
              someone sent you.
  role PROJECT ACCOUNT ROLE
              Change what a member can do: ROLE is editor, viewer, or owner.
              ACCOUNT is their account id, or their login or name as members
              shows it. owner gives them the project, and you become an
              editor.
  remove PROJECT ACCOUNT
              Take a member out of the project. Without ACCOUNT it does
              nothing; delete removes the whole project.
  leave PROJECT
              Leave a project someone shared with you.
  files list PROJECT
              The project's files, notes, and Drive sources.
  files add PROJECT PATH... [--protection standard|only_me]
              Add files. Large uploads resume when the same command is run
              again, within your account's configured limits. They follow
              the project's protection unless you pass --protection only_me.
  files get PROJECT FILE [--output PATH]
              Save one of the project's files or notes here.
  files remove PROJECT FILE
              Delete a file or note from the project for good.
  notes add PROJECT --title TITLE (--text TEXT | --file PATH)
              Add a text note to the project.
  sources list PROJECT
              The Google Drive folders and files the project's chats read
              from to answer. Also: openagents projects sources PROJECT
  sources add PROJECT LINK
              Attach a Drive folder or file by its link or id. Connect Google
              first with openagents connections connect google; find one with
              openagents connections drive.
  sources remove PROJECT SOURCE
              Detach a Drive folder or file (its id from sources list). The
              file stays in your Drive.
Start a new chat inside a project with openagents chat send MESSAGE
--project PROJECT; OpenAgents answers it there, with the project's
instructions and files and its primary repository when it has one, and it
shows in the project. A project can be in no repository, and one repository
can be in several projects; when a name matches more than one project, use
the id.
These are the projects in the left panel of openagents.com, signed in as
openagents login or coder login saved them, or OPENAGENTS_TOKEN. Ask about
only-me files with openagents vault ask FILE --project ID. Projects
someone shared with you list here too, with your role. GitHub Project
boards are openagents board.";

/// What each command does, for the chat router's command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("threads", Effect::ReadOnly),
    Declared::computer("read", Effect::ReadOnly),
    Declared::computer("create", Effect::Publishes),
    Declared::computer("add", Effect::Publishes),
    Declared::computer("edit", Effect::Publishes),
    Declared::computer("repo add", Effect::Publishes),
    Declared::computer("repo remove", Effect::Publishes),
    Declared::computer("repo primary", Effect::Publishes),
    Declared::computer("move", Effect::Publishes),
    Declared::computer("delete", Effect::Publishes),
    // Sharing changes who may open the project. These are `Publishes`,
    // not `Grants`: on the phone a grant opens the computers screen,
    // which has nothing to do with projects.
    Declared::computer("members", Effect::ReadOnly),
    Declared::computer("invite", Effect::Publishes),
    Declared::computer("join", Effect::Publishes),
    Declared::computer("role", Effect::Publishes),
    Declared::computer("remove", Effect::Publishes),
    Declared::computer("leave", Effect::Publishes),
    Declared::computer("files list", Effect::ReadOnly),
    Declared::computer("files add", Effect::Publishes),
    Declared::computer("files get", Effect::LocalWrite),
    Declared::computer("files remove", Effect::Publishes),
    Declared::computer("notes add", Effect::Publishes),
    Declared::computer("sources list", Effect::ReadOnly),
    Declared::computer("sources add", Effect::Publishes),
    Declared::computer("sources remove", Effect::Publishes),
];

/// Larger files use the resumable project upload API.
const MAX_FILE: u64 = 10 * 1024 * 1024;
/// How long one upload or download may take.
const TRANSFER: Duration = Duration::from_secs(300);

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.is_empty() || matches!(words[0].as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return if words.is_empty() {
            crate::EXIT_USAGE
        } else {
            0
        };
    }
    let args = match Args::parse(words, &["detach-threads", "none", "primary", "yes"]) {
        Ok(args) => args,
        Err(message) => return output.usage("projects", &message, USAGE),
    };
    let positional = sources_order(args.positional());
    let command = positional.first().map(String::as_str).unwrap_or_default();
    let command = if command == "chats" {
        "threads"
    } else {
        command
    };
    // `files add` and `notes add` are two words; their arguments follow.
    let (command, rest): (String, &[String]) = match command {
        "files" | "notes" | "repo" | "sources" => match positional.get(1) {
            Some(sub) => (format!("{command} {sub}"), &positional[2..]),
            None => {
                return output.usage(
                    "projects",
                    &format!("{command} takes a command: see openagents projects --help"),
                    USAGE,
                );
            }
        },
        other => (other.to_owned(), &positional[1..]),
    };
    // (options, fewest arguments, most arguments)
    let (known, fewest, most): (&[&str], usize, usize) = match command.as_str() {
        "list" => (&["account-dir"], 0, 0),
        "show" | "threads" => (&["account-dir"], 1, 1),
        "read" => (&["account-dir"], 1, 2),
        "create" => (&["account-dir", "repo", "instructions", "protection"], 1, 1),
        "add" => (&["account-dir", "protection"], 1, 1),
        "edit" => (&["account-dir", "name", "instructions", "protection"], 1, 1),
        "repo add" => (&["account-dir"], 2, 2),
        "repo remove" | "repo primary" => (&["account-dir"], 2, 2),
        "move" if args.switch("none") => (&["account-dir", "none"], 1, 1),
        "move" => (&["account-dir"], 2, 2),
        "remove" if positional.len() == 2 => {
            // `remove PROJECT` once deleted the whole project; forgetting
            // the member's account must never do that (#11623).
            return output.refuse("projects", &remove_hint(&positional[1]));
        }
        "remove" => (&["account-dir"], 2, 2),
        "delete" => (&["account-dir", "detach-threads", "yes"], 1, 1),
        "members" | "leave" => (&["account-dir"], 1, 1),
        "invite" => (&["account-dir", "github", "role", "ttl"], 1, 1),
        "join" => (&["account-dir"], 1, 1),
        "role" => (&["account-dir"], 3, 3),
        "files list" => (&["account-dir"], 1, 1),
        "files add" => (&["account-dir", "protection"], 2, usize::MAX),
        "files get" => (&["account-dir", "output", "out"], 2, 2),
        "files remove" => (&["account-dir"], 2, 2),
        "notes add" => (&["account-dir", "title", "text", "file"], 1, 1),
        "sources list" => (&["account-dir"], 1, 1),
        "sources add" | "sources remove" => (&["account-dir"], 2, 2),
        _ => return output.usage("projects", &format!("unknown command `{command}`"), USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !known.contains(name))
    {
        return output.usage("projects", &format!("unknown option `--{name}`"), USAGE);
    }
    if rest.len() < fewest || rest.len() > most {
        let wanted = match (fewest, most) {
            (0, 0) => "no arguments".to_owned(),
            (1, 1) => "one argument".to_owned(),
            (1, 2) => "a project and at most one chat".to_owned(),
            (2, 2) if command == "move" => "a chat and a project, or a chat and --none".to_owned(),
            (2, 2) if command.starts_with("repo ") => "a project and a repository".to_owned(),
            (2, 2) if command == "sources add" => "a project and a Drive link".to_owned(),
            (2, 2) if command == "sources remove" => "a project and a source".to_owned(),
            (2, 2) if command == "remove" => "a project and a member's account".to_owned(),
            (3, 3) => "a project, a member's account, and editor, viewer, or owner".to_owned(),
            (2, 2) => "a project and a file".to_owned(),
            _ => "a project and one or more files".to_owned(),
        };
        return output.usage("projects", &format!("{command} takes {wanted}"), USAGE);
    }
    let protection = match args.option("protection") {
        None => None,
        Some(word @ ("standard" | "only_me")) => Some(word),
        Some(other) => {
            return output.usage(
                "projects",
                &format!("--protection is standard or only_me, not `{other}`"),
                USAGE,
            );
        }
    };
    let invite_role = match args.option("role") {
        None => None,
        Some(word @ ("editor" | "viewer")) => Some(word),
        Some(other) => {
            return output.usage(
                "projects",
                &format!("--role is editor or viewer, not `{other}`"),
                USAGE,
            );
        }
    };
    let ttl = match args.option("ttl").map(duration_seconds).transpose() {
        Ok(ttl) => ttl,
        Err(message) => return output.usage("projects", &message, USAGE),
    };
    if command == "role" && !matches!(rest[2].as_str(), "editor" | "viewer" | "owner") {
        return output.usage(
            "projects",
            &format!("The role is editor, viewer, or owner, not `{}`", rest[2]),
            USAGE,
        );
    }
    if args.switch("primary") && command != "repo add" {
        return output.usage("projects", "--primary goes with repo add", USAGE);
    }
    if command == "edit"
        && protection.is_none()
        && args.option("name").is_none()
        && args.option("instructions").is_none()
    {
        return output.usage(
            "projects",
            "edit takes --name NAME, --instructions TEXT, or --protection standard|only_me",
            USAGE,
        );
    }
    if command == "create" && rest.first().is_some_and(|name| name.trim().is_empty()) {
        return output.usage("projects", "create takes the project's name", USAGE);
    }
    for repository in args.options("repo") {
        if !repository_name(repository) {
            return output.usage(
                "projects",
                &format!("--repo is a GitHub repository as owner/name, not `{repository}`"),
                USAGE,
            );
        }
    }
    if command.starts_with("repo ") && !repository_name(&rest[1]) {
        return output.usage(
            "projects",
            &format!("Name the repository as owner/name, not `{}`.", rest[1]),
            USAGE,
        );
    }
    let target = rest.first().map(String::as_str).unwrap_or_default();
    let site = match Site::open(args.option("account-dir")) {
        Ok(site) => site,
        Err(message) => return output.fail("projects", &message),
    };
    let projects = Projects(&site);
    let account_dir = args.option("account-dir");
    let result = match command.as_str() {
        "list" => projects.list().map(|listed| {
            output.emit(&listed, list_text);
        }),
        "show" | "threads" => projects.show(target).map(|shown| {
            output.emit(&shown, show_text);
        }),
        "read" => projects
            .read(target, rest.get(1).map(String::as_str))
            .map(|read| {
                output.emit(&read, read_text);
            }),
        "create" => projects
            .create(target, args.option("instructions"), &args.options("repo"))
            .and_then(|created| match protection {
                Some("only_me") => projects.edit(id(&created)?, &edit_body(None, None, protection)),
                _ => Ok(created),
            })
            .map(|created| {
                output.emit(&created, |value| {
                    format!(
                        "Made the project {} ({}).",
                        value["name"].as_str().unwrap_or_default(),
                        repositories_text(value)
                    )
                });
            }),
        "add" => projects
            .add(target)
            .and_then(|added| match protection {
                Some("only_me") => projects.edit(id(&added)?, &edit_body(None, None, protection)),
                _ => Ok(added),
            })
            .map(|added| {
                output.emit(&added, |value| {
                    format!(
                        "Added {} ({}).",
                        value["name"].as_str().unwrap_or_default(),
                        repositories_text(value)
                    )
                });
            }),
        "edit" => {
            let body = edit_body(args.option("name"), args.option("instructions"), protection);
            projects
                .find(target)
                .and_then(|project| projects.edit(id(&project)?, &body))
                .map(|edited| {
                    output.emit(&edited, |value| {
                        let name = value["name"].as_str().unwrap_or_default();
                        if protection.is_none() {
                            format!("Changed {name}.")
                        } else if value["protection"] == "only_me" {
                            format!("Only you can open {name}'s new files now. They're locked on your computer before they're sent.")
                        } else {
                            format!("openagents.com keeps {name}'s new files for your account now.")
                        }
                    });
                })
        }
        "repo add" | "repo remove" | "repo primary" => {
            let repository = rest[1].as_str();
            let change = match command.as_str() {
                "repo add" => RepoChange::Add {
                    primary: args.switch("primary"),
                },
                "repo remove" => RepoChange::Remove,
                _ => RepoChange::Primary,
            };
            projects
                .change_repository(target, repository, change)
                .map(|changed| {
                    output.emit(&changed, |value| {
                        let name = value["name"].as_str().unwrap_or_default();
                        match change {
                            RepoChange::Remove => format!(
                                "Took {repository} out of {name}. It now has {}.",
                                repositories_text(value)
                            ),
                            _ => format!("{name} now has {}.", repositories_text(value)),
                        }
                    });
                })
        }
        "move" => projects
            .move_chat(target, rest.get(1).map(String::as_str))
            .map(|moved| {
                output.emit(&moved, moved_text);
            }),
        "members" => projects.members(target).map(|listed| {
            output.emit(&listed, members_text);
        }),
        "invite" => projects
            .invite(target, args.option("github"), invite_role, ttl)
            .map(|made| {
                output.emit(&made, invited_text);
            }),
        "join" => projects.join(target).map(|joined| {
            output.emit(&joined, joined_text);
        }),
        "role" => projects
            .set_role(target, &rest[1], &rest[2])
            .map(|(changed, member)| {
                output.emit(&changed, |value| role_text(value, &member, &rest[2]));
            }),
        "remove" => projects
            .remove_member(target, &rest[1])
            .map(|(removed, member, project)| {
                output.emit(&removed, |_| {
                    format!(
                        "Took {} out of {}.",
                        member_name(&member),
                        project["name"].as_str().unwrap_or(target)
                    )
                });
            }),
        "leave" => projects.leave(target).map(|(left, project)| {
            output.emit(&left, |_| {
                format!("You left {}.", project["name"].as_str().unwrap_or(target))
            });
        }),
        "delete" => {
            let yes = args.switch("yes");
            projects
                .find(target)
                .and_then(|project| {
                    if !yes {
                        confirm_delete(&project)?;
                    }
                    projects.delete(project, args.switch("detach-threads"))
                })
                .map(|(removed, project)| {
                    output.emit(&removed, |_| {
                        let name = project["name"].as_str().unwrap_or_default();
                        if project["counts"]["threads"].as_u64().unwrap_or(0) > 0 {
                            format!("Deleted {name}. Its chats are still in your chat list.")
                        } else {
                            format!("Deleted {name}.")
                        }
                    });
                })
        }
        "files list" => projects.files(target, account_dir).map(|listed| {
            output.emit(&listed, files_text);
        }),
        "files add" => {
            let paths: Vec<PathBuf> = rest[1..].iter().map(PathBuf::from).collect();
            projects
                .add_files(target, &paths, protection, account_dir)
                .map(|added| {
                    output.emit(&added, added_text);
                })
        }
        "files get" => projects
            .get_file(
                target,
                &rest[1],
                args.option("output").or(args.option("out")),
                account_dir,
            )
            .map(|saved| {
                output.emit(&saved, |value| {
                    format!("Saved {}.", value["path"].as_str().unwrap_or_default())
                });
            }),
        "files remove" => projects
            .remove_file(target, &rest[1], account_dir)
            .map(|removed| {
                output.emit(&removed, |value| {
                    format!(
                        "Deleted {} for good.",
                        value["name"].as_str().unwrap_or_default()
                    )
                });
            }),
        "notes add" => {
            let Some(title) = args.option("title").filter(|t| !t.trim().is_empty()) else {
                return output.usage("projects", "notes add takes --title TITLE", USAGE);
            };
            let text = match (args.option("text"), args.option("file")) {
                (Some(text), None) => Ok(text.to_owned()),
                (None, Some(path)) => {
                    std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))
                }
                _ => {
                    return output.usage(
                        "projects",
                        "notes add takes --text TEXT or --file PATH",
                        USAGE,
                    );
                }
            };
            text.and_then(|text| projects.add_note(target, title, &text))
                .map(|added| {
                    output.emit(&added, |value| {
                        format!(
                            "Added the note {}.",
                            value["title"].as_str().unwrap_or_default()
                        )
                    });
                })
        }
        "sources list" => projects.sources(target).map(|listed| {
            output.emit(&listed, sources_text);
        }),
        "sources add" => projects.add_source(target, &rest[1]).map(|added| {
            output.emit(&added, |value| {
                format!(
                    "Attached {} ({}) to {}. Its chats read from it now.",
                    value["name"].as_str().unwrap_or_default(),
                    value["kind"].as_str().unwrap_or_default(),
                    value["project_name"].as_str().unwrap_or(target),
                )
            });
        }),
        "sources remove" => projects.remove_source(target, &rest[1]).map(|removed| {
            output.emit(&removed, |value| {
                format!(
                    "Detached {} from {}. It stays in your Drive.",
                    value["name"].as_str().unwrap_or_default(),
                    value["project_name"].as_str().unwrap_or(target),
                )
            });
        }),
        _ => unreachable!("checked above"),
    };
    match result {
        Ok(()) => 0,
        Err(message) => output.fail("projects", &message),
    }
}

/// `sources PROJECT [list|add LINK|remove SOURCE]` read as `sources
/// list|add|remove PROJECT ...`, the form the help lists.
fn sources_order(positional: &[String]) -> Vec<String> {
    let verbs = ["list", "add", "remove"];
    match positional {
        [first, project, rest @ ..] if first == "sources" && !verbs.contains(&project.as_str()) => {
            let (verb, more) = match rest.split_first() {
                Some((verb, more)) if verbs.contains(&verb.as_str()) => (verb.clone(), more),
                _ => ("list".to_owned(), rest),
            };
            let mut out = vec![first.clone(), verb, project.clone()];
            out.extend(more.iter().cloned());
            out
        }
        other => other.to_vec(),
    }
}

fn sources_text(listed: &Value) -> String {
    let name = listed["project_name"].as_str().unwrap_or_default();
    let sources = listed["data"].as_array().cloned().unwrap_or_default();
    let mut lines = Vec::new();
    if listed["google"] != "connected" {
        lines.push(
            "Google isn't connected, so chats can't read these now. Connect it with: openagents connections connect google"
                .to_owned(),
        );
    }
    if sources.is_empty() {
        lines.push(format!(
            "{name} has no Drive sources. Attach one with: openagents projects sources add {name} LINK"
        ));
    }
    for source in &sources {
        lines.push(format!(
            "  {}  {}  {}  ({})",
            source["name"].as_str().unwrap_or_default(),
            source["kind"].as_str().unwrap_or_default(),
            source["link"].as_str().unwrap_or_default(),
            source["id"].as_str().unwrap_or_default(),
        ));
    }
    lines.join("\n")
}

struct Projects<'a>(&'a Site);

/// The project `target` names (its name, repository, or id), found as
/// `projects show` finds it, for `chat send --project` (#11542).
pub(crate) fn named(site: &Site, target: &str) -> Result<Value, String> {
    Projects(site).find(target)
}

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

    /// The project's chats with their messages: all of them, or the one
    /// `chat` names (#11504).
    fn read(&self, target: &str, chat: Option<&str>) -> Result<Value, String> {
        let shown = self.show(target)?;
        let mut threads = shown["threads"].as_array().cloned().unwrap_or_default();
        if let Some(chat) = chat {
            let lowered = chat.trim().to_lowercase();
            let id = |thread: &Value| thread["id"].as_str().unwrap_or_default().to_owned();
            let exact: Vec<Value> = threads.iter().filter(|t| id(t) == chat).cloned().collect();
            let prefixed: Vec<Value> = threads
                .iter()
                .filter(|t| id(t).starts_with(chat))
                .cloned()
                .collect();
            let titled: Vec<Value> = threads
                .iter()
                .filter(|t| {
                    t["title"]
                        .as_str()
                        .unwrap_or_default()
                        .trim()
                        .to_lowercase()
                        == lowered
                })
                .cloned()
                .collect();
            threads = [exact, prefixed, titled]
                .into_iter()
                .find(|found| found.len() == 1)
                .ok_or_else(|| {
                    format!(
                        "no single chat in {} is named `{chat}`; give its id, a unique id prefix, or its title (openagents projects show {target} lists them)",
                        shown["project"]["name"].as_str().unwrap_or(target)
                    )
                })?;
        }
        let mut read = Vec::with_capacity(threads.len());
        for mut thread in threads {
            let id = thread["id"].as_str().unwrap_or_default().to_owned();
            thread["turns"] = json!(crate::account_threads::messages(self.0, &id)?);
            read.push(thread);
        }
        Ok(json!({"project": shown["project"], "threads": read}))
    }

    /// `POST /v1/projects {name, instructions?, repositories?}`: a new
    /// project, with no repository unless some are named.
    fn create(
        &self,
        name: &str,
        instructions: Option<&str>,
        repositories: &[&str],
    ) -> Result<Value, String> {
        let site = self.0;
        self.send(
            site.http
                .post(site.url("/v1/projects"))
                .json(&create_body(name, instructions, repositories)),
        )
        .map_err(|message| {
            // A website from before projects without a repository.
            if message.contains("A project is one GitHub repository") {
                "openagents.com can't make a project without a repository yet; it will once its next update is out. Until then: openagents projects add owner/name".to_owned()
            } else {
                message
            }
        })
    }

    /// Add, remove, or make primary one of the project's repositories.
    fn change_repository(
        &self,
        target: &str,
        repository: &str,
        change: RepoChange,
    ) -> Result<Value, String> {
        let project = self.find(target)?;
        let project_id = id(&project)?.to_owned();
        let held = repositories(&project);
        let has = held
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(repository))
            .map(|(name, _)| name.clone())
            .filter(|name| repository_name(name));
        let site = self.0;
        let request = match (change, has) {
            (RepoChange::Add { primary }, _) => site
                .http
                .post(site.url(&format!("/v1/projects/{project_id}/repositories")))
                .json(&json!({"repository": repository, "primary": primary})),
            (RepoChange::Remove, Some(held)) => site
                .http
                .delete(site.url(&format!("/v1/projects/{project_id}/repositories/{held}"))),
            (RepoChange::Primary, Some(held)) => site
                .http
                .patch(site.url(&format!("/v1/projects/{project_id}")))
                .json(&json!({"primary_repository": held})),
            (_, None) => {
                return Err(format!(
                    "{} has no repository {repository}. It has {}.",
                    project["name"].as_str().unwrap_or(target),
                    repositories_text(&project)
                ));
            }
        };
        self.send(request).map_err(|message| {
            if message.contains("answered 404") || message.contains("answered 405") {
                "openagents.com can't change a project's repositories yet; it will once its next update is out.".to_owned()
            } else {
                message
            }
        })
    }

    fn add(&self, repository: &str) -> Result<Value, String> {
        if !repository_name(repository) {
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

    /// Move the chat `chat` names into the project `target` names, or out
    /// of any project (`None`), with `PATCH /v1/threads/{id}`.
    fn move_chat(&self, chat: &str, target: Option<&str>) -> Result<Value, String> {
        let project = target.map(|target| self.find(target)).transpose()?;
        let thread = crate::account_threads::find(self.0, chat)?;
        let thread_id = safe(thread["id"].as_str().unwrap_or_default())?.to_owned();
        let site = self.0;
        let moved = self
            .send(
                site.http
                    .patch(site.url(&format!("/v1/threads/{thread_id}")))
                    .json(&json!({"project": project.as_ref().map(id).transpose()?})),
            )
            .map_err(|message| {
                // A website from before #11541 has no move yet.
                if message.contains("answered 405") {
                    "openagents.com can't move chats yet; it will once its next update is out."
                        .to_owned()
                } else {
                    message
                }
            })?;
        Ok(json!({"thread": moved, "project": project}))
    }

    /// Delete `project` (as [`Projects::find`] answered it) for good.
    fn delete(&self, project: Value, detach: bool) -> Result<(Value, Value), String> {
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

impl Projects<'_> {
    fn send(&self, request: reqwest::blocking::RequestBuilder) -> Result<Value, String> {
        Site::answer(request.bearer_auth(self.0.saved.token()).send())
    }

    /// `PATCH /v1/projects/{id}` with `body` ([`edit_body`]).
    fn edit(&self, project: &str, body: &Value) -> Result<Value, String> {
        let site = self.0;
        self.send(
            site.http
                .patch(site.url(&format!("/v1/projects/{project}")))
                .json(body),
        )
    }

    /// Every attachment of the project, and its protection.
    fn attachments(&self, project: &str) -> Result<(Vec<Value>, String), String> {
        let mut data = Vec::new();
        let mut after: Option<String> = None;
        let mut protection = "standard".to_owned();
        loop {
            let path = match &after {
                Some(after) => {
                    format!("/v1/projects/{project}/attachments?limit=200&after={after}")
                }
                None => format!("/v1/projects/{project}/attachments?limit=200"),
            };
            let page = self.get(&path)?;
            if let Some(word) = page["protection"].as_str() {
                word.clone_into(&mut protection);
            }
            data.extend(page["data"].as_array().cloned().unwrap_or_default());
            match page["next"].as_str() {
                Some(next) if !next.is_empty() && after.as_deref() != Some(next) => {
                    after = Some(safe(next)?.to_owned());
                }
                _ => break,
            }
        }
        Ok((data, protection))
    }

    /// The project's files, notes, sources, and only-me files, each with
    /// a `name`; only-me files are named from the vault's index here.
    fn files(&self, target: &str, account_dir: Option<&str>) -> Result<Value, String> {
        let project = self.find(target)?;
        let id = id(&project)?.to_owned();
        let (attachments, protection) = self.attachments(&id)?;
        let mut files: Vec<Value> = Vec::new();
        let mut registered = Vec::new();
        for attachment in &attachments {
            let mut row = attachment.clone();
            let name = match attachment["type"].as_str() {
                Some("file") => attachment["filename"].clone(),
                Some("document") => attachment["title"].clone(),
                Some("source") => attachment["name"].clone(),
                _ => Value::Null,
            };
            if attachment["type"] == "vault" {
                registered.push(attachment["id"].as_str().unwrap_or_default().to_owned());
            }
            row["name"] = name;
            files.push(row);
        }
        // Only-me files: their names live in the vault, opened here.
        let wants_vault = !registered.is_empty() || protection == "only_me";
        let mut locked = false;
        if wants_vault {
            match crate::vault::with_vault(account_dir, |vault| vault.list(Some(&id))) {
                Ok(entries) => {
                    for entry in entries
                        .iter()
                        .filter(|e| matches!(e.kind, oa_vault::Kind::File))
                    {
                        match files.iter_mut().find(|f| f["id"] == entry.object.as_str()) {
                            Some(row) => row["name"] = json!(entry.name),
                            None => files.push(json!({
                                "id": entry.object,
                                "object": "attachment",
                                "type": "vault",
                                "protection": "only_me",
                                "name": entry.name,
                                "size_bytes": entry.size,
                                "created_at": Value::Null,
                            })),
                        }
                    }
                }
                Err(_) if !registered.is_empty() => locked = true,
                Err(_) => {}
            }
        }
        Ok(json!({
            "project": project,
            "protection": protection,
            "files": files,
            "vault_locked": locked,
        }))
    }

    /// Send one file's bytes; the new file.
    fn upload(&self, path: &Path, project: &str) -> Result<Value, String> {
        let size = std::fs::metadata(path)
            .map_err(|error| format!("{}: {error}", path.display()))?
            .len();
        if size > MAX_FILE {
            return self.upload_large(path, project);
        }
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let name = path
            .file_name()
            .map_or_else(|| "File".into(), |name| name.to_string_lossy().into_owned());
        let site = self.0;
        self.send(
            site.http
                .post(site.url(&format!(
                    "/v1/files?purpose=project&filename={}",
                    encode(&name)
                )))
                .timeout(TRANSFER)
                .header("content-type", "application/octet-stream")
                .body(bytes),
        )
    }

    fn attach(&self, project: &str, body: &Value) -> Result<Value, String> {
        let site = self.0;
        self.send(
            site.http
                .post(site.url(&format!("/v1/projects/{project}/attachments")))
                .json(body),
        )
    }

    fn add_files(
        &self,
        target: &str,
        paths: &[PathBuf],
        protection: Option<&str>,
        account_dir: Option<&str>,
    ) -> Result<Value, String> {
        let project = self.find(target)?;
        let id = id(&project)?.to_owned();
        let project_protection = project["protection"].as_str().unwrap_or("standard");
        let only_me = match protection {
            Some("only_me") => true,
            Some(_) if project_protection == "only_me" => {
                return Err(format!(
                    "Only you can open {}'s files, so they can't be added as standard files.",
                    project["name"].as_str().unwrap_or_default()
                ));
            }
            Some(_) => false,
            None => project_protection == "only_me",
        };
        let mut added = Vec::new();
        for path in paths {
            let attachment = if only_me {
                let entry =
                    crate::vault::with_vault(account_dir, |vault| vault.add_file(path, Some(&id)))?;
                let mut attached = self.attach(&id, &json!({"vault": entry.object}))?;
                attached["name"] = json!(entry.name);
                attached
            } else {
                let file = self.upload(path, &id)?;
                let mut attached = if file["project"] == id {
                    file
                } else {
                    self.attach(&id, &json!({"file": file["id"]}))?
                };
                attached["name"] = attached["filename"].clone();
                attached
            };
            added.push(attachment);
        }
        Ok(json!({"project": project, "added": added}))
    }

    /// The one file `what` names: its id, its name, or the start of its id.
    fn pick(
        &self,
        target: &str,
        what: &str,
        account_dir: Option<&str>,
    ) -> Result<(Value, Value), String> {
        let listed = self.files(target, account_dir)?;
        let files = listed["files"].as_array().cloned().unwrap_or_default();
        let by = |test: &dyn Fn(&Value) -> bool| -> Vec<Value> {
            files.iter().filter(|f| test(f)).cloned().collect()
        };
        let mut found = by(&|f| f["id"] == what);
        if found.is_empty() {
            found = by(&|f| f["name"].as_str() == Some(what));
        }
        if found.is_empty() && what.len() >= 4 {
            found = by(&|f| f["id"].as_str().is_some_and(|id| id.starts_with(what)));
        }
        match found.as_slice() {
            [one] => Ok((listed["project"].clone(), one.clone())),
            [] if listed["vault_locked"] == json!(true) => Err(format!(
                "No file named {what} here, and your vault isn't open on this computer. Open it with: openagents vault unlock"
            )),
            [] => Err(format!(
                "No file in this project matches {what}. See them with: openagents projects files list {target}"
            )),
            _ => Err(format!(
                "More than one file matches {what}; give its id (openagents projects files list {target})."
            )),
        }
    }

    fn get_file(
        &self,
        target: &str,
        what: &str,
        output: Option<&str>,
        account_dir: Option<&str>,
    ) -> Result<Value, String> {
        let (project, file) = self.pick(target, what, account_dir)?;
        let project_id = id(&project)?.to_owned();
        let file_id = file["id"].as_str().unwrap_or_default().to_owned();
        let name = file["name"].as_str().unwrap_or("file").to_owned();
        if file["type"] == "file" {
            let path = self.download_large(
                &format!(
                    "/v1/files/{}/content?project={}",
                    safe(&file_id)?,
                    encode(&project_id)
                ),
                output.map(Path::new),
                &name,
                file["size_bytes"].as_u64().ok_or("This file has no size")?,
                file["sha256"].as_str(),
            )?;
            return Ok(
                json!({"id":file_id,"name":name,"path":path.display().to_string(),"size_bytes":file["size_bytes"]}),
            );
        }
        let bytes: zeroize::Zeroizing<Vec<u8>> = match file["type"].as_str() {
            Some("file") => zeroize::Zeroizing::new(
                self.download(&format!("/v1/files/{}/content", safe(&file_id)?))?,
            ),
            Some("document") => {
                let doc = self.get(&format!(
                    "/v1/projects/{project_id}/documents/{}",
                    safe(&file_id)?
                ))?;
                zeroize::Zeroizing::new(
                    doc["text"].as_str().unwrap_or_default().as_bytes().to_vec(),
                )
            }
            Some("vault") => crate::vault::with_vault(account_dir, |vault| vault.get(&file_id))?.1,
            _ => {
                return Err(format!(
                    "{name} is in Google Drive: {}",
                    file["link"].as_str().unwrap_or_default()
                ));
            }
        };
        let default = if file["type"] == "document" && !name.contains('.') {
            format!("{name}.txt")
        } else {
            name.clone()
        };
        let path = write_private(output.map(Path::new), &default, &bytes)?;
        Ok(
            json!({"id": file_id, "name": name, "path": path.display().to_string(), "size_bytes": bytes.len()}),
        )
    }

    fn download(&self, path: &str) -> Result<Vec<u8>, String> {
        let site = self.0;
        let response = site
            .http
            .get(site.url(path))
            .bearer_auth(site.saved.token())
            .timeout(TRANSFER)
            .send()
            .map_err(|_| "The website couldn't be reached.".to_owned())?;
        if !response.status().is_success() {
            return Site::answer(Ok(response)).map(|_| Vec::new());
        }
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|_| "The download stopped before it finished.".to_owned())
    }

    fn remove_file(
        &self,
        target: &str,
        what: &str,
        account_dir: Option<&str>,
    ) -> Result<Value, String> {
        let (project, file) = self.pick(target, what, account_dir)?;
        let project_id = id(&project)?.to_owned();
        let file_id = file["id"].as_str().unwrap_or_default().to_owned();
        if file["type"] == "source" {
            let site = self.0;
            self.send(site.http.delete(site.url(&format!(
                "/v1/projects/{project_id}/sources/{}",
                safe(&file_id)?
            ))))
            .map_err(not_open)?;
            return Ok(json!({"id": file_id, "name": file["name"], "object": "source.deleted"}));
        }
        if file["type"] == "vault" {
            crate::vault::with_vault(account_dir, |vault| vault.remove(&file_id))?;
        }
        let site = self.0;
        let removed = self.send(site.http.delete(site.url(&format!(
            "/v1/projects/{project_id}/attachments/{}",
            safe(&file_id)?
        ))));
        match removed {
            Ok(_) => {}
            // An only-me file added with `vault add --project` has no
            // record on the website to remove.
            Err(_) if file["type"] == "vault" => {}
            Err(message) => return Err(message),
        }
        Ok(json!({"id": file_id, "name": file["name"], "object": "attachment.deleted"}))
    }

    fn add_note(&self, target: &str, title: &str, text: &str) -> Result<Value, String> {
        let project = self.find(target)?;
        let id = id(&project)?.to_owned();
        self.attach(&id, &json!({"document": {"title": title, "text": text}}))
            .map_err(|message| {
                if project["protection"] == "only_me" {
                    format!("Only you can open this project's files, so notes go in as files: save the note to a file and run openagents projects files add {target} FILE")
                } else {
                    message
                }
            })
    }
}

impl Projects<'_> {
    /// The project's Drive sources, with its name.
    fn sources(&self, target: &str) -> Result<Value, String> {
        let project = self.find(target)?;
        let mut listed = self
            .get(&format!("/v1/projects/{}/sources", id(&project)?))
            .map_err(not_open)?;
        listed["project_name"] = project["name"].clone();
        Ok(listed)
    }

    fn add_source(&self, target: &str, link: &str) -> Result<Value, String> {
        let project = self.find(target)?;
        let site = self.0;
        let mut added = self
            .send(
                site.http
                    .post(site.url(&format!("/v1/projects/{}/sources", id(&project)?)))
                    .json(&json!({"link": link.trim()})),
            )
            .map_err(not_open)?;
        added["project_name"] = project["name"].clone();
        Ok(added)
    }

    fn remove_source(&self, target: &str, source: &str) -> Result<Value, String> {
        let listed = self.sources(target)?;
        let sources = listed["data"].as_array().cloned().unwrap_or_default();
        let found = sources
            .iter()
            .find(|s| s["id"] == source || s["link"] == source || s["name"] == source)
            .ok_or_else(|| {
                format!("That isn't one of this project's sources. See them with: openagents projects sources list {target}")
            })?;
        let source_id = safe(found["id"].as_str().unwrap_or_default())
            .map_err(|_| "The website answered with a source id that isn't valid.".to_owned())?
            .to_owned();
        let site = self.0;
        let mut removed = self.send(site.http.delete(site.url(&format!(
            "/v1/projects/{}/sources/{source_id}",
            listed["project"].as_str().unwrap_or_default()
        ))))?;
        removed["name"] = found["name"].clone();
        removed["project_name"] = listed["project_name"].clone();
        Ok(removed)
    }
}

/// Sharing (#11590): who is in a project, invitations, roles, leaving.
impl Projects<'_> {
    /// Every member of the project the caller is in, following the
    /// pages; the first page's `role` and `invitations` are kept, and the
    /// project rides along for the text.
    fn members(&self, target: &str) -> Result<Value, String> {
        let project = self.find(target)?;
        let id = id(&project)?.to_owned();
        let mut data = Vec::new();
        let mut first: Option<Value> = None;
        let mut after: Option<String> = None;
        loop {
            let path = match &after {
                Some(after) => format!("/v1/projects/{id}/members?after={after}"),
                None => format!("/v1/projects/{id}/members"),
            };
            let page = self.get(&path).map_err(not_shared)?;
            data.extend(page["data"].as_array().cloned().unwrap_or_default());
            let next = page["next"].as_str().map(str::to_owned);
            if first.is_none() {
                first = Some(page);
            }
            match next {
                Some(next) if !next.is_empty() && after.as_deref() != Some(next.as_str()) => {
                    after = Some(safe(&next)?.to_owned());
                }
                _ => break,
            }
        }
        let mut listed = first.unwrap_or_else(|| json!({}));
        listed["data"] = json!(data);
        listed["next"] = Value::Null;
        listed["project"] = project;
        Ok(listed)
    }

    /// The one member `who` names: their account id, else their login,
    /// else their name. More than one match is refused with each named.
    fn member(&self, target: &str, who: &str) -> Result<(Value, Value), String> {
        let listed = self.members(target)?;
        let members = listed["data"].as_array().cloned().unwrap_or_default();
        let found = find_member(&members, who)?.clone();
        Ok((found, listed["project"].clone()))
    }

    fn invite(
        &self,
        target: &str,
        github: Option<&str>,
        role: Option<&str>,
        ttl: Option<u64>,
    ) -> Result<Value, String> {
        let project = self.find(target)?;
        let mut body = json!({});
        if let Some(github) = github {
            body["github"] = json!(github);
        }
        if let Some(role) = role {
            body["role"] = json!(role);
        }
        if let Some(seconds) = ttl {
            body["expires_in"] = json!(seconds);
        }
        let site = self.0;
        let mut made = self
            .send(
                site.http
                    .post(site.url(&format!("/v1/projects/{}/invitations", id(&project)?)))
                    .json(&body),
            )
            .map_err(not_shared)?;
        if made["project_name"].is_null() {
            made["project_name"] = project["name"].clone();
        }
        Ok(made)
    }

    /// Preview the invitation, then accept it.
    fn join(&self, link: &str) -> Result<Value, String> {
        let code = json!({"code": link.trim()});
        let site = self.0;
        let preview = self
            .send(
                site.http
                    .post(site.url("/v1/projects/invitations/preview"))
                    .json(&code),
            )
            .map_err(not_shared)?;
        let project = self
            .send(
                site.http
                    .post(site.url("/v1/projects/invitations/accept"))
                    .json(&code),
            )
            .map_err(not_shared)?;
        Ok(json!({"invitation": preview, "project": project}))
    }

    /// `PATCH /v1/projects/{id}/members/{account}`: the Project, and the
    /// member it changed.
    fn set_role(&self, target: &str, who: &str, role: &str) -> Result<(Value, Value), String> {
        let (member, project) = self.member(target, who)?;
        let account = member["account"].as_str().unwrap_or_default();
        let site = self.0;
        let changed = self
            .send(
                site.http
                    .patch(site.url(&format!(
                        "/v1/projects/{}/members/{}",
                        id(&project)?,
                        encode(account)
                    )))
                    .json(&json!({"role": role})),
            )
            .map_err(not_shared)?;
        Ok((changed, member))
    }

    fn remove_member(&self, target: &str, who: &str) -> Result<(Value, Value, Value), String> {
        let (member, project) = self.member(target, who)?;
        let account = member["account"].as_str().unwrap_or_default();
        let site = self.0;
        let removed = self
            .send(site.http.delete(site.url(&format!(
                "/v1/projects/{}/members/{}",
                id(&project)?,
                encode(account)
            ))))
            .map_err(not_shared)?;
        Ok((removed, member, project))
    }

    fn leave(&self, target: &str) -> Result<(Value, Value), String> {
        let project = self.find(target)?;
        let name = project["name"].as_str().unwrap_or(target).to_owned();
        let owner_stays = || {
            format!(
                "You own {name}, so you can't leave it. Give it to another member first with: openagents projects role {name} ACCOUNT owner"
            )
        };
        if project["role"] == "owner" {
            return Err(owner_stays());
        }
        let site = self.0;
        let left = self
            .send(
                site.http
                    .post(site.url(&format!("/v1/projects/{}/leave", id(&project)?))),
            )
            .map_err(|message| {
                if message.contains("answered 409") {
                    owner_stays()
                } else {
                    not_shared(message)
                }
            })?;
        Ok((left, project))
    }
}

/// A website from before shared projects has none of these routes.
fn not_shared(message: String) -> String {
    if message.contains("answered 404") || message.contains("answered 405") {
        "openagents.com can't share projects yet; it will once its next update is out.".to_owned()
    } else {
        message
    }
}

/// `30m`, `12h`, `7d` (or `90s`) as seconds.
fn duration_seconds(text: &str) -> Result<u64, String> {
    let text = text.trim();
    let wrong = || format!("--ttl is a length of time like 30m, 12h, or 7d, not `{text}`");
    let split = text.len().saturating_sub(1);
    let (number, unit) = text.split_at(split);
    let unit = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return Err(wrong()),
    };
    let number: u64 = number.parse().map_err(|_| wrong())?;
    match number.checked_mul(unit) {
        Some(0) | None => Err(wrong()),
        Some(seconds) => Ok(seconds),
    }
}

/// The member `who` names among `members`: their account id, else their
/// login (an `@` in front is fine), else their name, without regard to
/// case. A login or name more than one member has is never guessed.
fn find_member<'a>(members: &'a [Value], who: &str) -> Result<&'a Value, String> {
    let who = who.trim();
    if let Some(member) = members.iter().find(|m| m["account"] == who) {
        return Ok(member);
    }
    let bare = who.trim_start_matches('@');
    let field = |key: &str| -> Vec<&'a Value> {
        members
            .iter()
            .filter(|m| {
                m[key]
                    .as_str()
                    .is_some_and(|value| value.trim().eq_ignore_ascii_case(bare))
            })
            .collect()
    };
    let by_login = field("login");
    let found = if by_login.is_empty() {
        field("name")
    } else {
        by_login
    };
    match found.as_slice() {
        [member] => Ok(member),
        [] => Err(format!(
            "Nobody in this project is `{who}`. See who is with: openagents projects members PROJECT"
        )),
        many => Err(format!(
            "{} members are called {who}: {}. Use the account id.",
            count_word(many.len()),
            many.iter()
                .map(|m| member_choice(m))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// A member's name as people read it: their name, else their login.
fn member_name(member: &Value) -> String {
    member["name"]
        .as_str()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| member["login"].as_str())
        .unwrap_or("the member")
        .to_owned()
}

/// One member among several a login or name matched.
fn member_choice(member: &Value) -> String {
    let login = member["login"]
        .as_str()
        .filter(|login| !login.is_empty())
        .map(|login| format!(" @{login}"))
        .unwrap_or_default();
    format!(
        "{} ({}{login})",
        member["account"].as_str().unwrap_or_default(),
        member_name(member)
    )
}

/// `a`/`an` with the role, `an editor`.
fn role_phrase(role: &str) -> String {
    match role {
        "owner" => "the owner".to_owned(),
        "editor" => "an editor".to_owned(),
        "viewer" => "a viewer".to_owned(),
        other => other.to_owned(),
    }
}

fn members_text(listed: &Value) -> String {
    let name = listed["project"]["name"].as_str().unwrap_or_default();
    let members = listed["data"].as_array().cloned().unwrap_or_default();
    let mut lines = vec![format!(
        "{name}: {} {}. You're {}.",
        members.len(),
        if members.len() == 1 {
            "person"
        } else {
            "people"
        },
        role_phrase(listed["role"].as_str().unwrap_or_default())
    )];
    for member in &members {
        let login = member["login"]
            .as_str()
            .filter(|login| !login.is_empty())
            .map(|login| format!("  @{login}"))
            .unwrap_or_default();
        lines.push(format!(
            "  {}{login}  {}  joined {}  ({}){}",
            member_name(member),
            member["role"].as_str().unwrap_or_default(),
            day(&member["joined_at"]),
            member["account"].as_str().unwrap_or_default(),
            if member["you"] == json!(true) {
                "  you"
            } else {
                ""
            },
        ));
    }
    let invitations = listed["invitations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !invitations.is_empty() {
        lines.push("Links nobody has used yet:".to_owned());
        for invitation in &invitations {
            lines.push(format!(
                "  {}  works until {}  ({})",
                invitation["role"].as_str().unwrap_or_default(),
                day(&invitation["expires_at"]),
                invitation["id"].as_str().unwrap_or_default(),
            ));
        }
    }
    if listed["role"] == "owner" {
        lines.push(format!(
            "Invite someone with: openagents projects invite {name}"
        ));
    }
    lines.join("\n")
}

fn invited_text(made: &Value) -> String {
    let link = made["url"]
        .as_str()
        .or_else(|| made["code"].as_str())
        .unwrap_or_default();
    if let Some(who) = made["invitee"]["github"].as_str() {
        return format!(
            "Invited {who} to {} as {}. It's waiting on their Projects page on openagents.com until {}: they sign in with GitHub and choose Join. This link works too, once, and won't be shown again:\n{link}",
            made["project_name"].as_str().unwrap_or("the project"),
            role_phrase(made["role"].as_str().unwrap_or("editor")),
            day(&made["expires_at"]),
        );
    }
    format!(
        "Send this link to the person you're inviting to {} as {}. It works once, until {}, and won't be shown again:\n{link}\nThey join with it on openagents.com, or with: openagents projects join LINK",
        made["project_name"].as_str().unwrap_or("the project"),
        role_phrase(made["role"].as_str().unwrap_or("editor")),
        day(&made["expires_at"]),
    )
}

fn joined_text(joined: &Value) -> String {
    let invitation = &joined["invitation"];
    let name = joined["project"]["name"]
        .as_str()
        .or_else(|| invitation["project"]["name"].as_str())
        .unwrap_or("the project");
    let role = joined["project"]["role"]
        .as_str()
        .or_else(|| invitation["role"].as_str())
        .unwrap_or_default();
    let inviter = invitation["invited_by"]
        .as_str()
        .or_else(|| invitation["invited_by"]["name"].as_str())
        .filter(|who| !who.trim().is_empty());
    let mut text = if invitation["member"] == json!(true) {
        format!("You were already in {name}, as {}.", role_phrase(role))
    } else {
        format!("You joined {name} as {}.", role_phrase(role))
    };
    if let Some(inviter) = inviter {
        text.push_str(&format!(" {inviter} invited you."));
    }
    text.push_str(&format!(" See it with: openagents projects show {name}"));
    text
}

fn role_text(project: &Value, member: &Value, role: &str) -> String {
    let name = project["name"].as_str().unwrap_or("the project");
    let who = member_name(member);
    if role == "owner" {
        format!("{who} owns {name} now. You're an editor.")
    } else {
        format!("{who} is {} of {name} now.", role_phrase(role))
    }
}

/// The website's `404` for an account Connections aren't open to yet.
fn not_open(message: String) -> String {
    if message == "Not found." || message.contains("answered 404") {
        "Drive sources aren't open to your account yet.".to_owned()
    } else {
        message
    }
}

/// `text` for a query value.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Write `bytes` to `out`, or to `name` in this folder (never over an
/// existing file), readable only by this user.
fn write_private(out: Option<&Path>, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write as _;
    let (path, fresh) = match out {
        Some(path) => (path.to_path_buf(), false),
        None => {
            let name = Path::new(name)
                .file_name()
                .map_or_else(|| "file".into(), |name| name.to_os_string());
            (PathBuf::from(name), true)
        }
    };
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if fresh {
        options.create_new(true);
    } else {
        options.create(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            format!(
                "{} already exists here. Pass --output PATH to save it somewhere else.",
                path.display()
            )
        } else {
            format!("{}: {error}", path.display())
        }
    })?;
    file.write_all(bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

fn size_text(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", bytes.div_ceil(1024))
    } else {
        format!("{bytes} B")
    }
}

fn file_line(file: &Value) -> String {
    let kind = match file["type"].as_str() {
        Some("document") => "note",
        Some("source") => "Drive",
        Some("vault") => "only me",
        _ => "file",
    };
    let name = file["name"].as_str().unwrap_or("(locked)");
    let size = file["size_bytes"]
        .as_u64()
        .map(size_text)
        .unwrap_or_default();
    format!(
        "  {name}  {kind}{}{}  ({})",
        if size.is_empty() {
            String::new()
        } else {
            format!("  {size}")
        },
        match file["created_at"].as_str() {
            Some(_) => format!("  {}", day(&file["created_at"])),
            None => String::new(),
        },
        file["id"].as_str().unwrap_or_default(),
    )
}

fn files_text(listed: &Value) -> String {
    let name = listed["project"]["name"].as_str().unwrap_or_default();
    let files = listed["files"].as_array().cloned().unwrap_or_default();
    let mut lines = vec![if listed["protection"] == "only_me" {
        format!("{name}: only you can open its files")
    } else {
        format!("{name}: files kept by openagents.com for your account")
    }];
    if files.is_empty() {
        lines.push(format!(
            "No files yet. Add one with: openagents projects files add {name} FILE"
        ));
    }
    lines.extend(files.iter().map(file_line));
    if listed["vault_locked"] == json!(true) {
        lines.push("Your vault isn't open on this computer, so only-me files show no names. Open it with: openagents vault unlock".into());
    }
    lines.join("\n")
}

fn added_text(added: &Value) -> String {
    let name = added["project"]["name"].as_str().unwrap_or_default();
    added["added"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|file| {
            let file_name = file["name"].as_str().unwrap_or_default();
            if file["type"] == "vault" {
                format!("Locked {file_name} on this computer and added it to {name}. Only you can open it.")
            } else {
                format!("Added {file_name} to {name}.")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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

/// What `repo add`, `repo remove`, and `repo primary` do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RepoChange {
    Add { primary: bool },
    Remove,
    Primary,
}

/// Whether `value` is a GitHub `owner/name`.
fn repository_name(value: &str) -> bool {
    let mut parts = value.split('/');
    let part = |p: Option<&str>| {
        p.is_some_and(|p| {
            !p.is_empty()
                && p.len() <= 100
                && !p.starts_with('.')
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
    };
    part(parts.next()) && part(parts.next()) && parts.next().is_none()
}

/// `POST /v1/projects`'s body.
fn create_body(name: &str, instructions: Option<&str>, repositories: &[&str]) -> Value {
    let mut body = json!({"name": name.trim()});
    if let Some(instructions) = instructions.filter(|text| !text.trim().is_empty()) {
        body["instructions"] = json!(instructions);
    }
    if !repositories.is_empty() {
        body["repositories"] = json!(repositories);
    }
    body
}

/// `PATCH /v1/projects/{id}`'s body: only what was given; empty
/// instructions clear them.
fn edit_body(name: Option<&str>, instructions: Option<&str>, protection: Option<&str>) -> Value {
    let mut body = json!({});
    if let Some(name) = name {
        body["name"] = json!(name.trim());
    }
    if let Some(instructions) = instructions {
        body["instructions"] = if instructions.trim().is_empty() {
            Value::Null
        } else {
            json!(instructions)
        };
    }
    if let Some(protection) = protection {
        body["protection"] = json!(protection);
    }
    body
}

/// The project's repositories as `(owner/name, primary)`, the primary
/// first: from `repositories`, or an older website's one `repository`.
fn repositories(project: &Value) -> Vec<(String, bool)> {
    let mut held: Vec<(String, bool)> = match project["repositories"].as_array() {
        Some(rows) => rows
            .iter()
            .filter_map(|row| {
                let name = row["repository"].as_str().or_else(|| row.as_str())?;
                Some((name.to_owned(), row["primary"] == json!(true)))
            })
            .collect(),
        None => project["repository"]
            .as_str()
            .filter(|name| !name.is_empty())
            .map(|name| vec![(name.to_owned(), true)])
            .unwrap_or_default(),
    };
    if held.len() == 1 {
        held[0].1 = true;
    }
    held.sort_by_key(|(_, primary)| !primary);
    held
}

/// The project's repositories in a few words: `no repository`, one
/// `owner/name`, or several with the primary marked.
fn repositories_text(project: &Value) -> String {
    let held = repositories(project);
    match held.as_slice() {
        [] => "no repository".to_owned(),
        [(name, _)] => name.clone(),
        many => many
            .iter()
            .map(|(name, primary)| {
                if *primary {
                    format!("{name} (primary)")
                } else {
                    name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// One project among several a name matched: its id and repositories.
fn choice_text(project: &Value) -> String {
    let held = repositories(project);
    let repos = match held.as_slice() {
        [] => "no repository".to_owned(),
        many => many
            .iter()
            .map(|(name, primary)| {
                if *primary {
                    format!("{name}, primary")
                } else {
                    name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join("; "),
    };
    format!(
        "{} {} ({repos})",
        project["id"].as_str().unwrap_or_default(),
        project["name"].as_str().unwrap_or_default()
    )
}

fn count_word(n: usize) -> String {
    match n {
        2 => "Two".into(),
        3 => "Three".into(),
        4 => "Four".into(),
        n => n.to_string(),
    }
}

/// What `remove PROJECT` without a member's account says: it deletes
/// nothing (#11623).
fn remove_hint(project: &str) -> String {
    let project = if project.is_empty() || project.contains(char::is_whitespace) {
        format!("\"{project}\"")
    } else {
        project.to_owned()
    };
    format!(
        "remove takes a member's account and changed nothing: openagents projects remove {project} ACCOUNT takes a member out; openagents projects delete {project} deletes the project."
    )
}

/// Whether `typed` is the project's name, as `delete` asks for it.
fn names_project(project: &Value, typed: &str) -> bool {
    let name = project["name"].as_str().unwrap_or_default();
    !name.is_empty() && typed.trim() == name
}

/// Ask for the project's name before `delete` deletes it. Nothing can be
/// typed when stdin isn't a terminal, so then it takes `--yes`.
fn confirm_delete(project: &Value) -> Result<(), String> {
    use std::io::{BufRead, IsTerminal, Write};
    let name = project["name"].as_str().unwrap_or_default();
    if !std::io::stdin().is_terminal() {
        return Err(format!(
            "Deleting {name} needs its name typed at a terminal, or --yes. Nothing was deleted."
        ));
    }
    eprint!("This deletes the project {name} for good. Type its name to confirm: ");
    let _ = std::io::stderr().flush();
    let mut typed = String::new();
    if std::io::stdin().lock().read_line(&mut typed).is_err() || !names_project(project, &typed) {
        return Err(format!("That isn't {name}. Nothing was deleted."));
    }
    Ok(())
}

/// The project `target` names: its id, its name, or one of its
/// repositories (`owner/name`); names and repositories match without
/// regard to case. Names can repeat and a repository can be in several
/// projects, so a target that matches more than one project is refused
/// with the choices, never guessed.
fn find_project<'a>(projects: &'a [Value], target: &str) -> Result<&'a Value, String> {
    if let Some(project) = projects.iter().find(|p| p["id"] == target) {
        return Ok(project);
    }
    let named: Vec<&Value> = projects
        .iter()
        .filter(|p| {
            p["name"]
                .as_str()
                .is_some_and(|name| name.trim().eq_ignore_ascii_case(target.trim()))
        })
        .collect();
    let (found, how) = if named.is_empty() {
        let held: Vec<&Value> = projects
            .iter()
            .filter(|p| {
                repositories(p)
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case(target))
            })
            .collect();
        (held, "have the repository")
    } else {
        (named, "are called")
    };
    match found.as_slice() {
        [project] => Ok(project),
        [] => Err(format!(
            "You have no project named `{target}`. See yours with: openagents projects list"
        )),
        many => Err(format!(
            "{} projects {how} {target}: {}. Use the id.",
            count_word(many.len()),
            many.iter()
                .map(|p| choice_text(p))
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
        "{}  {}  {} chat{}  last activity {}  ({}){}",
        project["name"].as_str().unwrap_or_default(),
        repositories_text(project),
        chats,
        if chats == 1 { "" } else { "s" },
        day(&project["updated_at"]),
        project["id"].as_str().unwrap_or_default(),
        sharing_text(project),
    )
}

/// `  shared with 3 people, you're an editor` for a shared project (or
/// one you aren't the owner of); nothing for a project only you are in.
fn sharing_text(project: &Value) -> String {
    let role = project["role"].as_str().unwrap_or("owner");
    let shared = project["shared"] == json!(true);
    if !shared && role == "owner" {
        return String::new();
    }
    let people = project["members"].as_u64().unwrap_or(0);
    let with = if people > 1 {
        format!("shared with {people} people")
    } else {
        "shared".to_owned()
    };
    format!("  {with}, you're {}", role_phrase(role))
}

fn list_text(listed: &Value) -> String {
    let projects = listed["data"].as_array().cloned().unwrap_or_default();
    if projects.is_empty() {
        return "You have no projects on openagents.com yet. Make one with: openagents projects create NAME".into();
    }
    projects
        .iter()
        .map(project_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// `projects read`: each chat's title, then its messages.
fn moved_text(moved: &Value) -> String {
    let title = moved["thread"]["title"].as_str().unwrap_or("The chat");
    match moved["project"]["name"].as_str() {
        Some(name) => format!("Moved {title} to {name}."),
        None => format!("Took {title} out of its project."),
    }
}

fn read_text(read: &Value) -> String {
    let threads = read["threads"].as_array().cloned().unwrap_or_default();
    let mut out = format!(
        "{}: {} chat{}",
        read["project"]["name"].as_str().unwrap_or_default(),
        threads.len(),
        if threads.len() == 1 { "" } else { "s" }
    );
    for thread in &threads {
        out.push_str(&format!(
            "\n\n## {} ({})\n{}",
            thread["title"].as_str().unwrap_or("Untitled"),
            thread["id"].as_str().unwrap_or_default(),
            crate::account_threads::turns_text(&thread["turns"]).trim_start_matches('\n')
        ));
    }
    out.trim_end().to_owned()
}

fn show_text(shown: &Value) -> String {
    let threads = shown["threads"].as_array().cloned().unwrap_or_default();
    let mut lines = vec![project_line(&shown["project"])];
    if let Some(role) = shown["project"]["role"].as_str() {
        let mut line = format!("You're {} of this project.", role_phrase(role));
        if shown["project"]["shared"] == json!(true) {
            line.push_str(&format!(
                " See who else is in it with: openagents projects members {}",
                shown["project"]["name"].as_str().unwrap_or_default()
            ));
        }
        lines.push(line);
    }
    let held = repositories(&shown["project"]);
    if held.len() > 1 {
        for (name, primary) in &held {
            lines.push(format!(
                "  repository {name}{}",
                if *primary { "  Primary" } else { "" }
            ));
        }
    }
    if let Some(instructions) = shown["project"]["instructions"]
        .as_str()
        .filter(|text| !text.trim().is_empty())
    {
        lines.push(format!("Instructions: {}", instructions.trim()));
    }
    let sources = shown["project"]["counts"]["sources"].as_u64().unwrap_or(0);
    if sources > 0 {
        lines.push(format!(
            "{sources} Drive source{} (see them with: openagents projects sources list {})",
            if sources == 1 { "" } else { "s" },
            shown["project"]["name"].as_str().unwrap_or_default()
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
    if !threads.is_empty() {
        lines.push(format!(
            "Read their messages with openagents projects read {}",
            shown["project"]["name"].as_str().unwrap_or_default()
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
    fn delete_needs_the_exact_name_and_remove_alone_points_at_it() {
        let project = &projects()[0];
        assert!(names_project(project, "finances\n"));
        assert!(!names_project(project, "Finances"));
        assert!(!names_project(project, "fin"));
        assert!(!names_project(project, ""));
        assert!(!names_project(&json!({"id": "prj_x"}), ""));
        let hint = remove_hint("finances");
        assert!(!hint.contains('\n'), "{hint}");
        assert!(hint.contains("changed nothing"), "{hint}");
        assert!(
            hint.contains("openagents projects delete finances"),
            "{hint}"
        );
        assert!(remove_hint("my notes").contains("delete \"my notes\""));
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
    fn a_project_needs_no_repository_and_reads_either_shape() {
        let bare = json!({"id": "prj_cccccccccccccccc", "name": "taxes", "repositories": [], "repository": null, "counts": {"threads": 0}, "updated_at": "2026-10-13T08:00:00Z"});
        assert!(repositories(&bare).is_empty());
        assert_eq!(
            project_line(&bare),
            "taxes  no repository  0 chats  last activity 2026-10-13  (prj_cccccccccccccccc)"
        );
        // An older website names one `repository`.
        assert_eq!(
            repositories(&projects()[0]),
            [("chris/finances".to_owned(), true)]
        );
        let two = json!({"id": "prj_dddddddddddddddd", "name": "site", "repository": "acme/web", "repositories": [
            {"repository": "acme/api", "primary": false, "default_branch": "main", "private": true},
            {"repository": "acme/web", "primary": true, "default_branch": "main", "private": false},
        ]});
        assert_eq!(
            repositories(&two),
            [
                ("acme/web".to_owned(), true),
                ("acme/api".to_owned(), false)
            ]
        );
        assert_eq!(repositories_text(&two), "acme/web (primary), acme/api");
        let shown = show_text(&json!({"project": two, "threads": []}));
        assert!(shown.contains("  repository acme/web  Primary"), "{shown}");
        assert!(shown.contains("  repository acme/api\n"), "{shown}");
        // A repo-less project is found by name, as chat send --project
        // finds it.
        let all = vec![bare, projects()[0].clone()];
        assert_eq!(
            find_project(&all, "taxes").unwrap()["id"],
            "prj_cccccccccccccccc"
        );
    }

    #[test]
    fn a_name_two_projects_share_is_never_guessed() {
        let all = vec![
            json!({"id": "prj_aaaaaaaaaaaaaaaa", "name": "finances", "repository": "AtlantisPleb/finances"}),
            json!({"id": "prj_eeeeeeeeeeeeeeee", "name": "finances", "repositories": []}),
            json!({"id": "prj_ffffffffffffffff", "name": "books", "repositories": [{"repository": "AtlantisPleb/finances", "primary": true}]}),
        ];
        let error = find_project(&all, "finances").unwrap_err();
        assert_eq!(
            error,
            "Two projects are called finances: prj_aaaaaaaaaaaaaaaa finances (AtlantisPleb/finances, primary), prj_eeeeeeeeeeeeeeee finances (no repository). Use the id."
        );
        // One repository in two projects: the choice, not a guess.
        let error = find_project(&all, "atlantispleb/finances").unwrap_err();
        assert!(
            error.starts_with(
                "Two projects have the repository atlantispleb/finances: prj_aaaaaaaaaaaaaaaa"
            ),
            "{error}"
        );
        assert_eq!(
            find_project(&all, "prj_eeeeeeeeeeeeeeee").unwrap()["repositories"],
            json!([])
        );
        assert_eq!(
            find_project(&all, "books").unwrap()["id"],
            "prj_ffffffffffffffff"
        );
    }

    #[test]
    fn create_and_edit_send_only_what_was_given() {
        assert_eq!(create_body(" taxes ", None, &[]), json!({"name": "taxes"}));
        assert_eq!(
            create_body("site", Some("Be brief."), &["acme/web", "acme/api"]),
            json!({"name": "site", "instructions": "Be brief.", "repositories": ["acme/web", "acme/api"]})
        );
        assert_eq!(
            edit_body(Some("books"), None, None),
            json!({"name": "books"})
        );
        assert_eq!(
            edit_body(None, Some(""), Some("only_me")),
            json!({"instructions": null, "protection": "only_me"})
        );
        assert!(repository_name("AtlantisPleb/finances"));
        assert!(!repository_name("finances"));
        assert!(!repository_name("a/b/c"));
        assert!(!repository_name("../x"));
    }

    #[test]
    fn a_move_says_where_the_chat_went() {
        let into = json!({"thread": {"title": "Taxes"}, "project": projects()[0]});
        assert_eq!(moved_text(&into), "Moved Taxes to finances.");
        let out = json!({"thread": {"title": "Taxes"}, "project": null});
        assert_eq!(moved_text(&out), "Took Taxes out of its project.");
    }

    #[test]
    fn the_listing_reads_as_one_line_a_project() {
        let text = list_text(&json!({"data": projects()}));
        assert_eq!(
            text,
            "finances  chris/finances  2 chats  last activity 2026-10-09  (prj_aaaaaaaaaaaaaaaa)\n\
             openagents  OpenAgentsInc/openagents  1 chat  last activity 2026-10-12  (prj_bbbbbbbbbbbbbbbb)"
        );
        assert!(list_text(&json!({"data": []})).contains("openagents projects create NAME"));
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
        assert!(text.ends_with("Read their messages with openagents projects read finances"));
    }

    #[test]
    fn a_projects_chats_read_message_by_message() {
        let read = json!({
            "project": projects()[0],
            "threads": [{"id": "c2", "title": "Taxes", "turns": [
                {"role": "user", "text": "When are they due?"},
                {"role": "assistant", "text": "April 15."},
            ]}],
        });
        let text = read_text(&read);
        assert_eq!(
            text,
            "finances: 1 chat\n\n## Taxes (c2)\nyou:\nWhen are they due?\n\nopenagents:\nApril 15."
        );
    }

    #[test]
    fn a_projects_files_read_as_one_line_each() {
        let listed = json!({
            "project": projects()[0],
            "protection": "only_me",
            "files": [
                {"id": "file_aaaaaaaaaaaaaaaaaaaaaaaa", "type": "file", "name": "september.csv", "size_bytes": 2048, "created_at": "2026-10-12T21:30:00Z"},
                {"id": "pdoc_bbbbbbbbbbbbbbbbbbbbbbbb", "type": "document", "name": "Budget notes", "size_bytes": 20, "created_at": "2026-10-12T21:31:00Z"},
                {"id": "ab".repeat(32), "type": "vault", "name": null, "size_bytes": 482_176, "created_at": "2026-10-12T21:32:00Z"},
            ],
            "vault_locked": true,
        });
        let text = files_text(&listed);
        assert!(
            text.starts_with("finances: only you can open its files"),
            "{text}"
        );
        assert!(
            text.contains(
                "  september.csv  file  2 KB  2026-10-12  (file_aaaaaaaaaaaaaaaaaaaaaaaa)"
            ),
            "{text}"
        );
        assert!(text.contains("  Budget notes  note  20 B"), "{text}");
        assert!(text.contains("(locked)  only me  471 KB"), "{text}");
        assert!(text.contains("openagents vault unlock"), "{text}");
        let empty =
            files_text(&json!({"project": projects()[1], "protection": "standard", "files": []}));
        assert!(
            empty.contains("openagents projects files add openagents FILE"),
            "{empty}"
        );
    }

    #[test]
    fn a_file_name_is_sent_encoded() {
        assert_eq!(encode("statement 09/2026.pdf"), "statement%2009%2F2026.pdf");
        assert_eq!(encode("plain-name_1.csv"), "plain-name_1.csv");
    }

    #[test]
    fn sources_read_in_either_order() {
        let words = |w: &[&str]| w.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            sources_order(&words(&["sources", "finances", "add", "LINK"])),
            words(&["sources", "add", "finances", "LINK"])
        );
        assert_eq!(
            sources_order(&words(&["sources", "finances"])),
            words(&["sources", "list", "finances"])
        );
        assert_eq!(
            sources_order(&words(&["sources", "remove", "finances", "f1"])),
            words(&["sources", "remove", "finances", "f1"])
        );
        let text = sources_text(
            &json!({"project_name": "finances", "google": "connected", "data": [
                {"id": "f1", "name": "Finances", "kind": "folder", "link": "https://drive.google.com/drive/folders/f1"}
            ]}),
        );
        assert_eq!(
            text,
            "  Finances  folder  https://drive.google.com/drive/folders/f1  (f1)"
        );
    }

    #[test]
    fn a_link_lasts_as_long_as_ttl_says() {
        assert_eq!(duration_seconds("30m"), Ok(1_800));
        assert_eq!(duration_seconds("12h"), Ok(43_200));
        assert_eq!(duration_seconds("7d"), Ok(604_800));
        assert_eq!(duration_seconds("90s"), Ok(90));
        for wrong in ["", "7", "d", "0h", "7w", "-1d", "1.5h"] {
            assert!(
                duration_seconds(wrong)
                    .unwrap_err()
                    .contains("30m, 12h, or 7d"),
                "{wrong}"
            );
        }
    }

    #[test]
    fn a_member_is_found_by_account_login_or_name_and_never_guessed() {
        let members = vec![
            json!({"account": "acct_1", "name": "Chris", "login": "chris", "role": "owner"}),
            json!({"account": "acct_2", "name": "Sam", "login": "sam-a", "role": "editor"}),
            json!({"account": "acct_3", "name": "Sam", "login": "sam-b", "role": "viewer"}),
        ];
        assert_eq!(find_member(&members, "acct_2").unwrap()["login"], "sam-a");
        assert_eq!(
            find_member(&members, "@SAM-B").unwrap()["account"],
            "acct_3"
        );
        assert_eq!(find_member(&members, "chris").unwrap()["account"], "acct_1");
        assert_eq!(
            find_member(&members, "sam").unwrap_err(),
            "Two members are called sam: acct_2 (Sam @sam-a), acct_3 (Sam @sam-b). Use the account id."
        );
        assert!(
            find_member(&members, "pat")
                .unwrap_err()
                .contains("openagents projects members")
        );
    }

    #[test]
    fn a_shared_project_says_so_and_your_role() {
        let mut shared = projects()[0].clone();
        shared["role"] = json!("editor");
        shared["shared"] = json!(true);
        shared["members"] = json!(3);
        assert!(
            project_line(&shared)
                .ends_with("(prj_aaaaaaaaaaaaaaaa)  shared with 3 people, you're an editor"),
            "{}",
            project_line(&shared)
        );
        let mut alone = projects()[0].clone();
        alone["role"] = json!("owner");
        alone["shared"] = json!(false);
        alone["members"] = json!(1);
        assert!(project_line(&alone).ends_with("(prj_aaaaaaaaaaaaaaaa)"));
        let shown = show_text(&json!({"project": shared, "threads": []}));
        assert!(
            shown.contains("You're an editor of this project. See who else is in it with: openagents projects members finances"),
            "{shown}"
        );
    }

    #[test]
    fn a_cursor_with_other_characters_is_refused() {
        assert!(safe("prj_0123").is_ok());
        assert!(safe("x&y=1").is_err());
    }
}
