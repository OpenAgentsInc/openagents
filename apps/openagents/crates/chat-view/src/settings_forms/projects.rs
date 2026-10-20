//! Projects (#12331): the account's projects and one project's name,
//! members and roles, invitations, repositories, and Drive sources, as the
//! site's `/projects` pages have them. A host reads `/v1/projects`,
//! `/v1/projects/{id}/members`, and `/v1/projects/{id}/sources`, fills
//! [`State`], and sends each [`Action`] to the same API.

use rust_native::Node;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    Intent, Load, Paints, button, card, column, danger_button, header, key, row, section,
    segmented, text, wide_row,
};

/// A repository connected to a project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Repository {
    pub name: String,
    pub primary: bool,
}

/// One project, as `/v1/projects` lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub repositories: Vec<Repository>,
    pub threads: u64,
}

impl Project {
    pub fn from_value(value: &Value) -> Option<Self> {
        Some(Self {
            id: value["id"].as_str()?.to_owned(),
            name: value["name"]
                .as_str()
                .unwrap_or("Untitled project")
                .to_owned(),
            repositories: value["repositories"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| {
                    Some(Repository {
                        name: r["repository"].as_str()?.to_owned(),
                        primary: r["primary"].as_bool().unwrap_or(false),
                    })
                })
                .collect(),
            threads: value["counts"]["threads"].as_u64().unwrap_or(0),
        })
    }

    /// `GET /v1/projects`'s answer.
    pub fn list(answer: &Value) -> Vec<Self> {
        answer["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Self::from_value)
            .collect()
    }

    fn detail(&self) -> String {
        let chats = match self.threads {
            1 => "1 chat".to_string(),
            n => format!("{n} chats"),
        };
        match self.repositories.iter().find(|r| r.primary) {
            Some(primary) => format!("{} · {chats}", primary.name),
            None => chats,
        }
    }
}

/// A member of a project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Member {
    pub account: String,
    pub name: String,
    /// `owner`, `editor`, or `viewer`.
    pub role: String,
    pub you: bool,
}

/// `GET /v1/projects/{id}/members`: the members, your role, and the open
/// invitations the owner sees.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Members {
    pub members: Vec<Member>,
    pub role: String,
    pub invitations: Vec<String>,
}

impl Members {
    pub fn from_value(answer: &Value) -> Self {
        Self {
            members: answer["data"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| {
                    Some(Member {
                        account: m["account"].as_str()?.to_owned(),
                        name: m["name"]
                            .as_str()
                            .filter(|n| !n.is_empty())
                            .or(m["login"].as_str())
                            .unwrap_or("Member")
                            .to_owned(),
                        role: m["role"].as_str().unwrap_or("viewer").to_owned(),
                        you: m["you"].as_bool().unwrap_or(false),
                    })
                })
                .collect(),
            role: answer["role"].as_str().unwrap_or("viewer").to_owned(),
            invitations: answer["invitations"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|i| {
                    let role = role_words(i["role"].as_str().unwrap_or("editor"));
                    match i["expires_at"].as_str() {
                        Some(at) => format!("{role} link, until {}", at.get(..10).unwrap_or(at)),
                        None => format!("{role} link"),
                    }
                })
                .collect(),
        }
    }

    fn owner(&self) -> bool {
        self.role == "owner"
    }
}

/// A Drive folder or file the project's chats read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub kind: String,
}

/// `GET /v1/projects/{id}/sources`'s sources.
pub fn sources(answer: &Value) -> Vec<Source> {
    answer["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some(Source {
                id: s["id"].as_str()?.to_owned(),
                name: s["name"].as_str().unwrap_or("Drive item").to_owned(),
                kind: s["kind"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// The open project.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Open {
    pub id: String,
    pub members: Load<Members>,
    pub sources: Load<Vec<Source>>,
    /// The invitation link just made, shown once.
    pub link: Option<String>,
    pub confirm_leave: bool,
    pub confirm_remove: Option<String>,
}

/// What the page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub list: Load<Vec<Project>>,
    pub open: Option<Open>,
    pub busy: bool,
}

impl State {
    pub fn project(&self, id: &str) -> Option<&Project> {
        self.list.ready()?.iter().find(|p| p.id == id)
    }
}

/// What a control on the page asks for. The ones that need words
/// (`Create`, `Rename`, `InvitePerson`, `AddRepository`, `AddSource`) ask
/// the host for a text first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Open(String),
    List,
    Create,
    Rename(String),
    /// A single-use link, for an editor or a viewer.
    InviteLink(String, String),
    /// By GitHub name or Nostr key.
    InvitePerson(String),
    Copy(String),
    SetRole(String, String, String),
    AskRemove(Option<String>),
    RemoveMember(String, String),
    AddRepository(String),
    MakePrimary(String, String),
    RemoveRepository(String, String),
    AddSource(String),
    RemoveSource(String, String),
    AskLeave(bool),
    Leave(String),
}

fn act(action: Action) -> Intent {
    Intent::Projects(action)
}

fn role_words(role: &str) -> &'static str {
    match role {
        "owner" => "Owner",
        "editor" => "Editor",
        _ => "Viewer",
    }
}

/// The view.
pub fn view(state: &State, paints: &Paints) -> Node<Intent> {
    let body = match &state.open {
        Some(open) => project(state, open, paints),
        None => list(state, paints),
    };
    column("projects", body)
}

fn list(state: &State, paints: &Paints) -> Vec<Node<Intent>> {
    let mut out = vec![header(
        "projects-title",
        "Projects",
        Some(
            "A project keeps chats, instructions, files, and repositories together, and you can share it.",
        ),
        paints,
    )];
    out.push(button("projects-new", "New project", act(Action::Create)));
    match state.list.ready() {
        Some(projects) if !projects.is_empty() => out.push(card(
            "projects-list",
            projects
                .iter()
                .map(|p| {
                    row(
                        &key("project", &p.id),
                        &p.name,
                        Some(&p.detail()),
                        vec![button(
                            &key("project-open", &p.id),
                            "Open",
                            act(Action::Open(p.id.clone())),
                        )],
                        paints,
                    )
                })
                .collect(),
            paints,
        )),
        _ => out.push(card(
            "projects-none",
            vec![text(
                "projects-none-line",
                match &state.list {
                    Load::Failed(error) => error.as_str(),
                    Load::Ready(_) => "No projects yet.",
                    _ => "Loading…",
                },
                paints.muted,
            )],
            paints,
        )),
    }
    out
}

fn project(state: &State, open: &Open, paints: &Paints) -> Vec<Node<Intent>> {
    let found = state.project(&open.id);
    let name = found.map_or("Project", |p| p.name.as_str());
    let members = open.members.ready();
    let owner = members.is_some_and(Members::owner);
    let editor = members.is_some_and(|m| m.owner() || m.role == "editor");
    let id = open.id.clone();
    let mut out = vec![header(
        "project-title",
        name,
        members.map(|m| role_words(&m.role)),
        paints,
    )];
    let mut top = vec![button("project-back", "All projects", act(Action::List))];
    if editor {
        top.push(button(
            "project-rename",
            "Rename",
            act(Action::Rename(id.clone())),
        ));
    }
    out.push(card(
        "project-top",
        vec![row(
            "project-top-row",
            name,
            found.map(|p| p.detail()).as_deref(),
            top,
            paints,
        )],
        paints,
    ));

    // Members and invitations.
    let mut rows = Vec::new();
    match &open.members {
        Load::Ready(members) => {
            for m in &members.members {
                let mut controls = Vec::new();
                if owner && !m.you && m.role != "owner" {
                    let roles = [
                        (
                            "Editor",
                            act(Action::SetRole(
                                id.clone(),
                                m.account.clone(),
                                "editor".into(),
                            )),
                        ),
                        (
                            "Viewer",
                            act(Action::SetRole(
                                id.clone(),
                                m.account.clone(),
                                "viewer".into(),
                            )),
                        ),
                    ];
                    controls.extend(segmented(
                        &key("member-role", &m.account),
                        &[
                            (roles[0].0, roles[0].1.clone()),
                            (roles[1].0, roles[1].1.clone()),
                        ],
                        usize::from(m.role == "viewer"),
                    ));
                    controls.push(
                        if open.confirm_remove.as_deref() == Some(m.account.as_str()) {
                            danger_button(
                                &key("member-remove-confirm", &m.account),
                                "Remove for good",
                                act(Action::RemoveMember(id.clone(), m.account.clone())),
                                paints,
                            )
                        } else {
                            danger_button(
                                &key("member-remove", &m.account),
                                "Remove",
                                act(Action::AskRemove(Some(m.account.clone()))),
                                paints,
                            )
                        },
                    );
                }
                let who = if m.you {
                    format!("{} (you)", m.name)
                } else {
                    m.name.clone()
                };
                rows.push(row(
                    &key("member", &m.account),
                    &who,
                    Some(role_words(&m.role)),
                    controls,
                    paints,
                ));
            }
            for (ix, line) in members.invitations.iter().enumerate() {
                rows.push(row(
                    &format!("invitation-{ix}"),
                    "Open invitation",
                    Some(line),
                    Vec::new(),
                    paints,
                ));
            }
        }
        Load::Failed(error) => rows.push(text("members-state", error, paints.muted)),
        _ => rows.push(text("members-state", "Loading…", paints.muted)),
    }
    if owner {
        rows.push(row(
            "invite",
            "Invite someone",
            Some("Send a link that works once, or invite a GitHub name or Nostr key."),
            vec![
                button(
                    "invite-editor",
                    "Editor link",
                    act(Action::InviteLink(id.clone(), "editor".into())),
                ),
                button(
                    "invite-viewer",
                    "Viewer link",
                    act(Action::InviteLink(id.clone(), "viewer".into())),
                ),
                button(
                    "invite-person",
                    "Invite by name",
                    act(Action::InvitePerson(id.clone())),
                ),
            ],
            paints,
        ));
    }
    if let Some(link) = &open.link {
        rows.push(wide_row(
            "invite-link",
            "Invitation link",
            Some(link),
            vec![button(
                "invite-link-copy",
                "Copy",
                act(Action::Copy(link.clone())),
            )],
            paints,
        ));
    }
    out.push(section("project-members", "Members", rows, paints));

    // Repositories.
    let mut rows = Vec::new();
    for r in found.map(|p| p.repositories.as_slice()).unwrap_or_default() {
        let mut controls = Vec::new();
        if editor {
            if !r.primary {
                controls.push(button(
                    &key("repo-primary", &r.name),
                    "Make primary",
                    act(Action::MakePrimary(id.clone(), r.name.clone())),
                ));
            }
            controls.push(danger_button(
                &key("repo-remove", &r.name),
                "Remove",
                act(Action::RemoveRepository(id.clone(), r.name.clone())),
                paints,
            ));
        }
        rows.push(row(
            &key("repo", &r.name),
            &r.name,
            r.primary.then_some("Primary"),
            controls,
            paints,
        ));
    }
    if rows.is_empty() {
        rows.push(text("repos-none", "No repositories yet.", paints.muted));
    }
    if editor {
        rows.push(row(
            "repo-add",
            "Add a repository",
            Some("A GitHub repository the OpenAgents app can reach, such as owner/name."),
            vec![button(
                "repo-add-button",
                "Add",
                act(Action::AddRepository(id.clone())),
            )],
            paints,
        ));
    }
    out.push(section("project-repos", "Repositories", rows, paints));

    // Drive sources.
    let mut rows = Vec::new();
    match &open.sources {
        Load::Ready(sources) => {
            for s in sources {
                rows.push(row(
                    &key("source", &s.id),
                    &s.name,
                    (!s.kind.is_empty()).then_some(s.kind.as_str()),
                    if editor {
                        vec![danger_button(
                            &key("source-remove", &s.id),
                            "Remove",
                            act(Action::RemoveSource(id.clone(), s.id.clone())),
                            paints,
                        )]
                    } else {
                        Vec::new()
                    },
                    paints,
                ));
            }
            if sources.is_empty() {
                rows.push(text(
                    "sources-none",
                    "No Drive folders or files yet.",
                    paints.muted,
                ));
            }
            if editor {
                rows.push(row(
                    "source-add",
                    "Add from Google Drive",
                    Some("Paste a link to a Drive folder or file."),
                    vec![button(
                        "source-add-button",
                        "Add",
                        act(Action::AddSource(id.clone())),
                    )],
                    paints,
                ));
            }
            out.push(section("project-sources", "Sources", rows, paints));
        }
        // Without a Google connection the sources stay on the site's page.
        Load::Failed(_) => {}
        _ => {
            rows.push(text("sources-state", "Loading…", paints.muted));
            out.push(section("project-sources", "Sources", rows, paints));
        }
    }

    if members.is_some() && !owner {
        out.push(card(
            "project-leave",
            vec![row(
                "project-leave-row",
                "Leave this project",
                Some("You lose access to its chats and files."),
                vec![if open.confirm_leave {
                    danger_button(
                        "project-leave-confirm",
                        "Leave for good",
                        act(Action::Leave(id.clone())),
                        paints,
                    )
                } else {
                    danger_button(
                        "project-leave-button",
                        "Leave",
                        act(Action::AskLeave(true)),
                        paints,
                    )
                }],
                paints,
            )],
            paints,
        ));
    }
    out
}
