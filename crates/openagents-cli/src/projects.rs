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
//! A project also keeps private files and text documents (`files`,
//! `notes`). In a project whose protection is `only_me`, `files add` locks
//! each file on this computer with the person's vault (`openagents vault
//! add --project`) before anything is sent; the website records only that
//! the locked file belongs to the project.
//!
//! GitHub Project boards are `openagents board`, a different thing.

use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(test)]
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
  read PROJECT [CHAT]
              Every chat in the project, message by message, or only CHAT
              (its id, a unique id prefix, or its title). One chat also
              reads with openagents chat read --thread ID.
  add REPOSITORY [--protection standard|only_me]
              Add a project from a connected GitHub repository (owner/name).
  edit PROJECT --protection standard|only_me
              Who can open the project's new files. standard: openagents.com
              keeps them for your account. only_me: each file is locked on
              your computer before it's sent, and only your devices can open
              it (set up once with openagents vault setup).
  move CHAT PROJECT
              Move a chat into a project. CHAT is its id, a unique id
              prefix, or its title; PROJECT its name, repository, or id.
  move CHAT --none
              Take a chat out of its project.
  remove PROJECT [--detach-threads]
              Remove a project. With chats in it, --detach-threads keeps them,
              outside any project.
  files list PROJECT
              The project's files, notes, and Drive sources.
  files add PROJECT PATH... [--protection standard|only_me]
              Add files, 10 MB each at most. They follow the project's
              protection unless you pass --protection only_me.
  files get PROJECT FILE [--output PATH]
              Save one of the project's files or notes here.
  files remove PROJECT FILE
              Delete a file or note from the project for good.
  notes add PROJECT --title TITLE (--text TEXT | --file PATH)
              Add a text note to the project.
Start a new chat inside a project with openagents chat send MESSAGE
--project PROJECT; OpenAgents answers it there and it shows in the project.
These are the projects in the left panel of openagents.com, signed in as
openagents login or coder login saved them, or OPENAGENTS_TOKEN. Ask about
only-me files with openagents vault ask FILE --project ID. GitHub Project
boards are openagents board.";

/// What each command does, for the chat router's command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("threads", Effect::ReadOnly),
    Declared::computer("read", Effect::ReadOnly),
    Declared::computer("add", Effect::Publishes),
    Declared::computer("edit", Effect::Publishes),
    Declared::computer("move", Effect::Publishes),
    Declared::computer("remove", Effect::Publishes),
    Declared::computer("files list", Effect::ReadOnly),
    Declared::computer("files add", Effect::Publishes),
    Declared::computer("files get", Effect::LocalWrite),
    Declared::computer("files remove", Effect::Publishes),
    Declared::computer("notes add", Effect::Publishes),
];

/// The largest file `files add` sends.
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
    let args = match Args::parse(words, &["detach-threads", "none"]) {
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
    // `files add` and `notes add` are two words; their arguments follow.
    let (command, rest): (String, &[String]) = match command {
        "files" | "notes" => match positional.get(1) {
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
        "add" => (&["account-dir", "protection"], 1, 1),
        "edit" => (&["account-dir", "protection"], 1, 1),
        "move" if args.switch("none") => (&["account-dir", "none"], 1, 1),
        "move" => (&["account-dir"], 2, 2),
        "remove" => (&["account-dir", "detach-threads"], 1, 1),
        "files list" => (&["account-dir"], 1, 1),
        "files add" => (&["account-dir", "protection"], 2, usize::MAX),
        "files get" => (&["account-dir", "output", "out"], 2, 2),
        "files remove" => (&["account-dir"], 2, 2),
        "notes add" => (&["account-dir", "title", "text", "file"], 1, 1),
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
    if command == "edit" && protection.is_none() {
        return output.usage(
            "projects",
            "edit takes --protection standard|only_me",
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
        "add" => projects
            .add(target)
            .and_then(|added| match protection {
                Some("only_me") => projects.edit(id(&added)?, "only_me"),
                _ => Ok(added),
            })
            .map(|added| {
                output.emit(&added, |value| {
                    format!(
                        "Added {} ({}).",
                        value["name"].as_str().unwrap_or_default(),
                        value["repository"].as_str().unwrap_or_default()
                    )
                });
            }),
        "edit" => projects
            .find(target)
            .and_then(|project| projects.edit(id(&project)?, protection.unwrap_or_default()))
            .map(|edited| {
                output.emit(&edited, |value| {
                    let name = value["name"].as_str().unwrap_or_default();
                    if value["protection"] == "only_me" {
                        format!("Only you can open {name}'s new files now. They're locked on your computer before they're sent.")
                    } else {
                        format!("openagents.com keeps {name}'s new files for your account now.")
                    }
                });
            }),
        "move" => projects
            .move_chat(target, rest.get(1).map(String::as_str))
            .map(|moved| {
                output.emit(&moved, moved_text);
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
                    format!("Deleted {} for good.", value["name"].as_str().unwrap_or_default())
                });
            }),
        "notes add" => {
            let Some(title) = args.option("title").filter(|t| !t.trim().is_empty()) else {
                return output.usage("projects", "notes add takes --title TITLE", USAGE);
            };
            let text = match (args.option("text"), args.option("file")) {
                (Some(text), None) => Ok(text.to_owned()),
                (None, Some(path)) => std::fs::read_to_string(path)
                    .map_err(|error| format!("{path}: {error}")),
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
                        format!("Added the note {}.", value["title"].as_str().unwrap_or_default())
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

impl Projects<'_> {
    fn send(&self, request: reqwest::blocking::RequestBuilder) -> Result<Value, String> {
        Site::answer(request.bearer_auth(self.0.saved.token()).send())
    }

    /// `PATCH /v1/projects/{id}` with the protection.
    fn edit(&self, project: &str, protection: &str) -> Result<Value, String> {
        let site = self.0;
        self.send(
            site.http
                .patch(site.url(&format!("/v1/projects/{project}")))
                .json(&json!({"protection": protection})),
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
    fn upload(&self, path: &Path) -> Result<Value, String> {
        let size = std::fs::metadata(path)
            .map_err(|error| format!("{}: {error}", path.display()))?
            .len();
        if size > MAX_FILE {
            return Err(format!("{} is bigger than 10 MB.", path.display()));
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
                let file = self.upload(path)?;
                let mut attached = self.attach(&id, &json!({"file": file["id"]}))?;
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
            return Err(format!(
                "{} is a Drive source; remove it on the project's Sources page: {}/projects/{project_id}/sources",
                file["name"].as_str().unwrap_or_default(),
                self.0.saved.origin
            ));
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
    fn a_cursor_with_other_characters_is_refused() {
        assert!(safe("prj_0123").is_ok());
        assert!(safe("x&y=1").is_err());
    }
}
