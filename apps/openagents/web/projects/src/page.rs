use gpui::Context;
use openagents_chat_view::settings_forms::projects::{
    Action, Members, Open, Project, State, sources, view,
};
use openagents_web_page_host::{Host, Intent, Load, Page, Paints, Prompt, plain};
use rust_native::Node;
use serde_json::{Value, json};

pub struct Projects {
    state: State,
}

type Cx<'a> = Context<'a, Host<Projects>>;

fn encode(text: &str) -> String {
    String::from(js_sys::encode_uri_component(text))
}

fn base(id: &str) -> String {
    format!("/v1/projects/{}", encode(id))
}

impl Page for Projects {
    const NAME: &'static str = "projects";

    fn new() -> Self {
        Self {
            state: State::default(),
        }
    }

    fn title(&self) -> String {
        "Projects".into()
    }

    fn view(&self, paints: &Paints) -> Node<Intent> {
        view(&self.state, paints)
    }

    fn open(host: &mut Host<Self>, path: &str, cx: &mut Cx) {
        load_list(host, cx);
        match path.split('/').find(|p| !p.is_empty()) {
            Some(id) => open_project(host, id.to_owned(), cx),
            None => host.page.state.open = None,
        }
    }

    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx) {
        let Intent::Projects(action) = intent else {
            return;
        };
        let ask = |host: &mut Host<Self>,
                   purpose: String,
                   label: &str,
                   placeholder: &str,
                   value: String,
                   cx: &mut Cx| {
            host.ask(
                Prompt {
                    purpose,
                    label: label.into(),
                    placeholder: placeholder.into(),
                    value,
                },
                cx,
            )
        };
        match action {
            Action::Open(id) => open_project(host, id, cx),
            Action::List => {
                host.page.state.open = None;
                host.moved("");
            }
            Action::Copy(text) => host.copy(text, cx),
            Action::Create => ask(
                host,
                "create".into(),
                "Name the new project.",
                "Project name",
                String::new(),
                cx,
            ),
            Action::Rename(id) => {
                let name = host
                    .page
                    .state
                    .project(&id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                ask(
                    host,
                    format!("rename\n{id}"),
                    "Rename the project.",
                    "Project name",
                    name,
                    cx,
                )
            }
            Action::InvitePerson(id) => ask(
                host,
                format!("invite\n{id}"),
                "Invite a GitHub name or a Nostr key (npub1…). It waits on their Projects page.",
                "GitHub name or npub1…",
                String::new(),
                cx,
            ),
            Action::AddRepository(id) => ask(
                host,
                format!("repository\n{id}"),
                "Add a GitHub repository, such as owner/name.",
                "owner/name",
                String::new(),
                cx,
            ),
            Action::AddSource(id) => ask(
                host,
                format!("source\n{id}"),
                "Paste a link to a Google Drive folder or file.",
                "https://drive.google.com/…",
                String::new(),
                cx,
            ),
            Action::InviteLink(id, role) => send(
                host,
                "POST",
                format!("{}/invitations", base(&id)),
                Some(json!({"role": role})),
                Then::Link,
                cx,
            ),
            Action::SetRole(id, account, role) => send(
                host,
                "PATCH",
                format!("{}/members/{}", base(&id), encode(&account)),
                Some(json!({"role": role})),
                Then::Members,
                cx,
            ),
            Action::AskRemove(account) => {
                if let Some(open) = host.page.state.open.as_mut() {
                    open.confirm_remove = account;
                }
            }
            Action::RemoveMember(id, account) => send(
                host,
                "DELETE",
                format!("{}/members/{}", base(&id), encode(&account)),
                None,
                Then::Members,
                cx,
            ),
            Action::MakePrimary(id, repository) => send(
                host,
                "PATCH",
                base(&id),
                Some(json!({"primary_repository": repository})),
                Then::Project,
                cx,
            ),
            Action::RemoveRepository(id, repository) => {
                let (owner, name) = repository.split_once('/').unwrap_or((&repository, ""));
                send(
                    host,
                    "DELETE",
                    format!(
                        "{}/repositories/{}/{}",
                        base(&id),
                        encode(owner),
                        encode(name)
                    ),
                    None,
                    Then::Project,
                    cx,
                )
            }
            Action::RemoveSource(id, source) => send(
                host,
                "DELETE",
                format!("{}/sources/{}", base(&id), encode(&source)),
                None,
                Then::Sources,
                cx,
            ),
            Action::AskLeave(ask) => {
                if let Some(open) = host.page.state.open.as_mut() {
                    open.confirm_leave = ask;
                }
            }
            Action::Leave(id) => send(
                host,
                "POST",
                format!("{}/leave", base(&id)),
                Some(json!({})),
                Then::Left,
                cx,
            ),
        }
    }

    fn submit(host: &mut Host<Self>, prompt: &Prompt, text: String, cx: &mut Cx) {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let (what, id) = prompt
            .purpose
            .split_once('\n')
            .unwrap_or((prompt.purpose.as_str(), ""));
        match what {
            "create" => send(
                host,
                "POST",
                "/v1/projects".into(),
                Some(json!({"name": text})),
                Then::Created,
                cx,
            ),
            "rename" => send(
                host,
                "PATCH",
                base(id),
                Some(json!({"name": text})),
                Then::Project,
                cx,
            ),
            "repository" => send(
                host,
                "POST",
                format!("{}/repositories", base(id)),
                Some(json!({"repository": text})),
                Then::Project,
                cx,
            ),
            "source" => send(
                host,
                "POST",
                format!("{}/sources", base(id)),
                Some(json!({"link": text})),
                Then::Sources,
                cx,
            ),
            "invite" => {
                let body = if text.starts_with("npub1") {
                    json!({"npub": text})
                } else {
                    json!({"github": text.trim_start_matches('@')})
                };
                send(
                    host,
                    "POST",
                    format!("{}/invitations", base(id)),
                    Some(body),
                    Then::Invited,
                    cx,
                )
            }
            _ => {}
        }
    }
}

fn load_list(host: &mut Host<Projects>, cx: &mut Cx) {
    if !matches!(host.page.state.list, Load::Ready(_)) {
        host.page.state.list = Load::Loading;
    }
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let answer: Result<Value, _> = client.get("/v1/projects?limit=100").await;
        this.update(cx, |host, cx| {
            host.page.state.list = match answer {
                Ok(answer) => Load::Ready(Project::list(&answer)),
                Err(error) => Load::Failed(plain(&error)),
            };
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}

fn open_project(host: &mut Host<Projects>, id: String, cx: &mut Cx) {
    host.page.state.open = Some(Open {
        id: id.clone(),
        members: Load::Loading,
        sources: Load::Loading,
        ..Open::default()
    });
    host.moved(&id);
    read_members(host, id.clone(), cx);
    read_sources(host, id, cx);
}

fn read_members(host: &mut Host<Projects>, id: String, cx: &mut Cx) {
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let answer: Result<Value, _> = client.get(&format!("{}/members", base(&id))).await;
        this.update(cx, |host, cx| {
            if let Some(open) = host.page.state.open.as_mut().filter(|o| o.id == id) {
                open.members = match answer {
                    Ok(answer) => Load::Ready(Members::from_value(&answer)),
                    Err(error) => Load::Failed(plain(&error)),
                };
            }
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}

fn read_sources(host: &mut Host<Projects>, id: String, cx: &mut Cx) {
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let answer: Result<Value, _> = client.get(&format!("{}/sources", base(&id))).await;
        this.update(cx, |host, cx| {
            if let Some(open) = host.page.state.open.as_mut().filter(|o| o.id == id) {
                open.sources = match answer {
                    Ok(answer) => Load::Ready(sources(&answer)),
                    Err(error) => Load::Failed(plain(&error)),
                };
            }
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}

/// What a change's answer updates.
#[derive(Clone, Copy)]
enum Then {
    /// The answer is the project.
    Project,
    /// A new project: list it and open it.
    Created,
    Members,
    Sources,
    /// An invitation link, shown once.
    Link,
    Invited,
    Left,
}

fn send(
    host: &mut Host<Projects>,
    method: &'static str,
    path: String,
    body: Option<Value>,
    then: Then,
    cx: &mut Cx,
) {
    host.page.state.busy = true;
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let answer: Result<Value, _> = client.send(method, &path, body.as_ref()).await;
        this.update(cx, |host, cx| {
            host.page.state.busy = false;
            let answer = match answer {
                Ok(answer) => answer,
                Err(error) => return host.say(Some(plain(&error)), cx),
            };
            let open = host.page.state.open.as_ref().map(|o| o.id.clone());
            match then {
                Then::Project | Then::Created => {
                    if let Some(project) = Project::from_value(&answer) {
                        let id = project.id.clone();
                        if let Load::Ready(list) = &mut host.page.state.list {
                            match list.iter_mut().find(|p| p.id == id) {
                                Some(row) => *row = project,
                                None => list.push(project),
                            }
                        }
                        if matches!(then, Then::Created) {
                            open_project(host, id, cx);
                        }
                    }
                }
                Then::Members => {
                    if let Some(id) = open {
                        if let Some(open) = host.page.state.open.as_mut() {
                            open.confirm_remove = None;
                        }
                        read_members(host, id, cx);
                    }
                }
                Then::Sources => {
                    if let Some(id) = open {
                        read_sources(host, id, cx);
                    }
                }
                Then::Link => {
                    if let Some(open) = host.page.state.open.as_mut() {
                        open.link = answer["url"].as_str().map(str::to_owned);
                    }
                    if let Some(id) = open {
                        read_members(host, id, cx);
                    }
                }
                Then::Invited => {
                    host.notice = Some("Invited. It waits on their Projects page.".into());
                }
                Then::Left => {
                    if let (Some(id), Load::Ready(list)) = (open, &mut host.page.state.list) {
                        list.retain(|p| p.id != id);
                    }
                    host.page.state.open = None;
                    host.moved("");
                    host.notice = Some("You left the project.".into());
                }
            }
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}
