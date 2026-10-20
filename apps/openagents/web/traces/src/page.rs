use gpui::Context;
use openagents_chat_view::settings_forms::traces::{
    Action, AgentRow, Open, State, Summary, steps_of, view,
};
use openagents_web_page_host::{Host, Intent, Load, Page, Paints, plain};
use rust_native::Node;
use serde_json::{Value, json};

pub struct Traces {
    state: State,
}

type Cx<'a> = Context<'a, Host<Traces>>;

fn encode(id: &str) -> String {
    String::from(js_sys::encode_uri_component(id))
}

impl Page for Traces {
    const NAME: &'static str = "traces";

    fn new() -> Self {
        Self {
            state: State::default(),
        }
    }

    fn title(&self) -> String {
        "Traces".into()
    }

    fn view(&self, paints: &Paints) -> Node<Intent> {
        view(&self.state, paints)
    }

    fn open(host: &mut Host<Self>, path: &str, cx: &mut Cx) {
        let mut parts = path.split('/').filter(|p| !p.is_empty());
        let id = parts.next().map(str::to_owned);
        let agent = parts.next().map(str::to_owned);
        load_list(host, cx);
        match id {
            Some(id) => open_trace(host, id, agent, cx),
            None => host.page.state.open = None,
        }
    }

    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx) {
        let Intent::Traces(action) = intent else {
            return;
        };
        match action {
            Action::Open(id, agent) => open_trace(host, id, agent, cx),
            Action::List => {
                host.page.state.open = None;
                host.page.state.confirm_delete = None;
                host.moved("");
            }
            Action::Copy(text) => host.copy(text, cx),
            Action::AskDelete(id) => host.page.state.confirm_delete = id,
            Action::Share(id, shared) => share(host, id, shared, cx),
            Action::Delete(id) => delete(host, id, cx),
        }
    }
}

fn load_list(host: &mut Host<Traces>, cx: &mut Cx) {
    if !matches!(host.page.state.list, Load::Ready(_)) {
        host.page.state.list = Load::Loading;
    }
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let answer: Result<Value, _> = client.get("/v1/traces").await;
        this.update(cx, |host, cx| {
            host.page.state.list = match answer {
                Ok(answer) => Load::Ready(Summary::list(&answer)),
                Err(error) => Load::Failed(plain(&error)),
            };
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}

fn open_trace(host: &mut Host<Traces>, id: String, agent: Option<String>, cx: &mut Cx) {
    host.page.state.confirm_delete = None;
    host.page.state.open = Some(Open {
        id: id.clone(),
        agent: agent.clone(),
        steps: Load::Loading,
        agents: Load::Loading,
    });
    host.moved(&match &agent {
        Some(agent) => format!("{id}/{agent}"),
        None => id.clone(),
    });
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let base = format!("/v1/traces/{}", encode(&id));
        let document_path = match &agent {
            Some(agent) => format!("{base}/agents/{}", encode(agent)),
            None => base.clone(),
        };
        let document: Result<Value, _> = client.get(&document_path).await;
        let agents: Result<Value, _> = client.get(&format!("{base}/agents")).await;
        this.update(cx, |host, cx| {
            let Some(open) = host
                .page
                .state
                .open
                .as_mut()
                .filter(|o| o.id == id && o.agent == agent)
            else {
                return;
            };
            open.steps = match document {
                Ok(document) => Load::Ready(steps_of(&document)),
                Err(error) => Load::Failed(plain(&error)),
            };
            open.agents = match agents {
                Ok(answer) => Load::Ready(AgentRow::list(&answer)),
                Err(_) => Load::Ready(Vec::new()),
            };
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}

fn share(host: &mut Host<Traces>, id: String, shared: bool, cx: &mut Cx) {
    host.page.state.busy = true;
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let path = format!("/v1/traces/{}/share", encode(&id));
        let answer: Result<Value, _> = client
            .send("POST", &path, Some(&json!({ "shared": shared })))
            .await;
        this.update(cx, |host, cx| {
            host.page.state.busy = false;
            match answer {
                Ok(answer) => {
                    if let (Some(trace), Load::Ready(list)) = (
                        Summary::from_value(&answer["trace"]),
                        &mut host.page.state.list,
                    ) && let Some(row) = list.iter_mut().find(|t| t.id == trace.id)
                    {
                        *row = trace;
                    }
                }
                Err(error) => host.notice = Some(plain(&error)),
            }
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}

fn delete(host: &mut Host<Traces>, id: String, cx: &mut Cx) {
    host.page.state.busy = true;
    host.page.state.confirm_delete = None;
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let path = format!("/v1/traces/{}", encode(&id));
        let answer: Result<Value, _> = client.send("DELETE", &path, None::<&()>).await;
        this.update(cx, |host, cx| {
            host.page.state.busy = false;
            match answer {
                Ok(_) => {
                    if let Load::Ready(list) = &mut host.page.state.list {
                        list.retain(|t| t.id != id);
                    }
                    host.page.state.open = None;
                    host.moved("");
                    host.notice = Some("Deleted.".into());
                }
                Err(error) => host.notice = Some(plain(&error)),
            }
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}
