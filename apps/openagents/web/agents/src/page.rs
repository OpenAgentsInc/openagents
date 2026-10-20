use std::time::Duration;

use gpui::{Context, Task};
use openagents_chat_view::settings_forms::agents::{Action, State, computers, landings, view};
use openagents_web_page_host::{Host, Intent, Load, Page, Paints, Prompt, plain};
use rust_native::Node;
use serde_json::{Value, json};

/// How often the page reads again while something runs.
const EVERY: Duration = Duration::from_secs(5);

pub struct Agents {
    state: State,
    _poll: Option<Task<()>>,
}

type Cx<'a> = Context<'a, Host<Agents>>;

impl Page for Agents {
    const NAME: &'static str = "agents";

    fn new() -> Self {
        Self {
            state: State::default(),
            _poll: None,
        }
    }

    fn title(&self) -> String {
        "Agents".into()
    }

    fn view(&self, paints: &Paints) -> Node<Intent> {
        view(&self.state, paints)
    }

    fn open(host: &mut Host<Self>, _path: &str, cx: &mut Cx) {
        read(host, cx);
    }

    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx) {
        let Intent::Agents(action) = intent else {
            return;
        };
        match action {
            Action::Refresh => read(host, cx),
            Action::Stop(computer, item) => act(host, computer, item, "stop", None, cx),
            Action::Message(computer, item) => host.ask(
                Prompt {
                    purpose: format!("{computer}\n{item}"),
                    label: "Message this agent. It reads it at its next step.".into(),
                    placeholder: "Write a message".into(),
                    value: String::new(),
                },
                cx,
            ),
        }
    }

    fn submit(host: &mut Host<Self>, prompt: &Prompt, text: String, cx: &mut Cx) {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return host.say(Some("Write a message first.".into()), cx);
        }
        if let Some((computer, item)) = prompt.purpose.split_once('\n') {
            act(
                host,
                computer.into(),
                item.into(),
                "message",
                Some(text),
                cx,
            );
        }
    }
}

/// Read the agents and the landing queue, and again every [`EVERY`] while
/// something runs.
fn read(host: &mut Host<Agents>, cx: &mut Cx) {
    if !matches!(host.page.state.computers, Load::Ready(_)) {
        host.page.state.computers = Load::Loading;
    }
    let client = host.client.clone();
    host.page._poll =
        Some(cx.spawn(async move |this, cx| {
            loop {
                let agents: Result<Value, _> = client.get("/v1/agents").await;
                let land: Result<Value, _> = client.get("/v1/land").await;
                let live = this
                    .update(cx, |host, cx| {
                        let state = &mut host.page.state;
                        state.computers = match agents {
                            Ok(answer) => Load::Ready(computers(&answer)),
                            Err(error) => Load::Failed(plain(&error)),
                        };
                        // Only the people who land changes see the queue.
                        state.landing = match land {
                            Ok(answer) => Load::Ready(landings(&answer)),
                            Err(error) => Load::Failed(plain(&error)),
                        };
                        host.changed(cx);
                        host.page.state.computers.ready().is_some_and(|all| {
                            all.iter().any(|c| c.agents.iter().any(|a| a.live()))
                        })
                    })
                    .unwrap_or(false);
                if !live {
                    break;
                }
                cx.background_executor().timer(EVERY).await;
            }
        }));
}

fn act(
    host: &mut Host<Agents>,
    computer: String,
    item: String,
    action: &'static str,
    text: Option<String>,
    cx: &mut Cx,
) {
    host.page.state.busy = Some(item.clone());
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let body = json!({
            "request_id": uuid::Uuid::new_v4().to_string(),
            "computer": computer,
            "item": item,
            "action": action,
            "text": text,
        });
        let answer: Result<Value, _> = client.send("POST", "/v1/agents/actions", Some(&body)).await;
        this.update(cx, |host, cx| {
            host.page.state.busy = None;
            let line = match answer {
                Ok(answer) => answer["message"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        if action == "stop" {
                            "Asked it to stop. It stops at its next step.".into()
                        } else {
                            "Sent. It reads your message at its next step.".into()
                        }
                    }),
                Err(error) => plain(&error),
            };
            host.say(Some(line), cx);
            read(host, cx);
        })
        .ok();
    })
    .detach();
}
