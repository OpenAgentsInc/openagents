//! Browser plumbing shared by the three independently loaded admin modules.
use gpui::Context;
use openagents_chat_view::settings_forms::admin::{self, Action, Report, State};
use openagents_web_page_host::{Host, Intent, Load, Page, Paints, Prompt, plain};
use rust_native::Node;
use serde_json::{Value, json};

pub struct Admin {
    state: State,
    generation: u64,
}

type Cx<'a> = Context<'a, Host<Admin>>;

impl Page for Admin {
    const NAME: &'static str = super::NAME;
    fn new() -> Self {
        Self {
            state: State::default(),
            generation: 0,
        }
    }
    fn title(&self) -> String {
        super::TITLE.into()
    }
    fn view(&self, paints: &Paints) -> Node<Intent> {
        admin::view(super::TITLE, super::DRIVE, &self.state, paints)
    }
    fn open(host: &mut Host<Self>, _: &str, cx: &mut Cx) {
        read(host, cx);
    }
    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx) {
        let Intent::Admin(action) = intent else {
            return;
        };
        match action {
            Action::Refresh => read(host, cx),
            Action::Open(path) => {
                let url = match path.as_str() {
                    "admin-analytics" => "/admin/analytics?ui=gpui",
                    "admin-attribution" => "/admin/attribution?ui=gpui",
                    "admin-drive-access" => "/admin/access/drive-connections?ui=gpui",
                    other => other,
                };
                host.go(url);
            }
            Action::Add if super::DRIVE && !host.page.state.busy => host.ask(
                Prompt {
                    purpose: "add".into(),
                    label: "Allow Google Drive access".into(),
                    placeholder: "Email or account id".into(),
                    value: String::new(),
                },
                cx,
            ),
            Action::ConfirmRemove(who) if super::DRIVE => {
                host.page.state.confirm_remove = who;
                host.changed(cx);
            }
            Action::Remove(who)
                if super::DRIVE
                    && !host.page.state.busy
                    && host.page.state.confirm_remove.as_deref() == Some(&who) =>
            {
                let encoded: String = who
                    .bytes()
                    .map(|b| {
                        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                            (b as char).to_string()
                        } else {
                            format!("%{b:02X}")
                        }
                    })
                    .collect();
                change(
                    host,
                    "DELETE",
                    format!("{}/{encoded}", super::API),
                    None,
                    cx,
                );
            }
            _ => {}
        }
    }
    fn submit(host: &mut Host<Self>, prompt: &Prompt, text: String, cx: &mut Cx) {
        let who = text.trim();
        if !super::DRIVE || prompt.purpose != "add" || host.page.state.busy {
            return;
        }
        if who.is_empty() {
            return host.say(Some("Give an email address or an account id.".into()), cx);
        }
        let body = if who.contains('@') {
            json!({"email":who})
        } else {
            json!({"account":who})
        };
        change(host, "POST", super::API.into(), Some(body), cx);
    }
}

fn report(answer: Value) -> Result<Report, String> {
    if super::DRIVE {
        Ok(admin::drive_report(&answer))
    } else {
        serde_json::from_value(answer)
            .map_err(|_| "The numbers couldn't be read. Try again in a minute.".into())
    }
}

fn read(host: &mut Host<Admin>, cx: &mut Cx) {
    // Clear old privileged data before rechecking the session. A late read
    // from an earlier opening must not overwrite a newer response.
    host.page.generation += 1;
    let generation = host.page.generation;
    host.page.state.report = Load::Loading;
    host.page.state.confirm_remove = None;
    host.changed(cx);
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let answer: Result<Value, _> = client.get(super::API).await;
        let answer = answer.map_err(|e| plain(&e)).and_then(report);
        this.update(cx, |host, cx| {
            if host.page.generation != generation {
                return;
            }
            host.page.state.report = match answer {
                Ok(report) => Load::Ready(report),
                Err(error) => Load::Failed(error),
            };
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}

fn change(
    host: &mut Host<Admin>,
    method: &'static str,
    path: String,
    body: Option<Value>,
    cx: &mut Cx,
) {
    host.page.generation += 1;
    host.page.state.busy = true;
    host.changed(cx);
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let answer: Result<Value, _> = client.send(method, &path, body.as_ref()).await;
        this.update(cx, |host, cx| {
            host.page.state.busy = false;
            let notice = match answer {
                Ok(_) if method == "POST" => "Added.".into(),
                Ok(_) => "Removed.".into(),
                Err(error) => plain(&error),
            };
            host.say(Some(notice), cx);
            read(host, cx);
        })
        .ok();
    })
    .detach();
}
