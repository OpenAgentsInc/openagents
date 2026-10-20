//! Transport and actions for the shared credential forms. Secrets stay private.

use gpui::{Context, Task};
use openagents_chat_view::settings_forms::{
    Page as SettingsPage,
    keys::{self, Action},
};
use openagents_web_client::Refusal;
use openagents_web_page_host::{Host, Intent, Load, Page, Paints, Prompt};
use rust_native::Node;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Listed {
    #[serde(default)]
    keys: Vec<keys::Entry>,
    #[serde(default)]
    agents: Vec<keys::Entry>,
}

// This response is never serialized, logged, or passed to the shared view.
#[derive(Deserialize)]
struct Created {
    token: String,
}

pub struct Keys {
    page: SettingsPage,
    state: keys::State,
    secret: Option<String>,
    task: Option<Task<()>>,
}

type Cx<'a> = Context<'a, Host<Keys>>;

impl Page for Keys {
    const NAME: &'static str = "keys";

    fn new() -> Self {
        Self {
            page: SettingsPage::ApiKeys,
            state: keys::State::default(),
            secret: None,
            task: None,
        }
    }

    fn title(&self) -> String {
        format!("Settings · {}", self.page.label())
    }

    fn view(&self, paints: &Paints) -> Node<Intent> {
        keys::view(self.page, &self.state, paints)
    }

    fn open(host: &mut Host<Self>, path: &str, cx: &mut Cx) {
        host.page.task = None;
        host.page.secret = None;
        host.page.state = keys::State {
            signed_in: true,
            ..Default::default()
        };
        host.notice = None;
        host.unask(cx);
        if path == "__closed" {
            return;
        }
        host.page.page = SettingsPage::from_slug(path)
            .filter(|page| keys::is_page(*page))
            .unwrap_or(SettingsPage::ApiKeys);
        load(host, cx);
    }

    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx) {
        let Intent::Keys(action) = intent else { return };
        match action {
            Action::Open(page) => {
                host.moved(page.slug());
                Self::open(host, page.slug(), cx);
            }
            Action::Settings => host.go("/gpui?ui=gpui&settings=general"),
            Action::SignIn => host.go(&format!(
                "/login?next=%2Fgpui%3Fpage%3Dkeys%2F{}",
                host.page.page.slug()
            )),
            Action::Dismiss => {
                host.page.secret = None;
                host.page.state.copy_available = false;
            }
            Action::Copy => {
                if let Some(secret) = host.page.secret.clone() {
                    host.copy(secret, cx);
                }
            }
            Action::Confirm(id) => host.page.state.confirm = id,
            Action::Expiry(days) => host.page.state.expires_in_days = days,
            Action::Refresh if !host.page.state.busy => load(host, cx),
            Action::Create if !host.page.state.busy => {
                let agents = host.page.page == SettingsPage::AgentAccounts;
                host.ask(
                    Prompt {
                        purpose: "create".into(),
                        label: if agents { "Claim code" } else { "Key name" }.into(),
                        placeholder: if agents { "clm_…" } else { "Key name" }.into(),
                        value: String::new(),
                    },
                    cx,
                );
            }
            Action::Revoke(id)
                if !host.page.state.busy && host.page.state.confirm.as_ref() == Some(&id) =>
            {
                change(host, Some(id), String::new(), cx)
            }
            Action::ProviderKeys => host.go("/settings/api-keys?ui=html#api-keys-own"),
            _ => {}
        }
    }

    fn submit(host: &mut Host<Self>, _: &Prompt, text: String, cx: &mut Cx) {
        if host.page.state.busy {
            return;
        }
        let name = text.trim().to_owned();
        if name.is_empty() || name.len() > 256 {
            host.notice = Some("Enter a name or claim code, up to 256 characters.".into());
            return;
        }
        change(host, None, name, cx);
    }
}

fn load(host: &mut Host<Keys>, cx: &mut Cx) {
    host.page.state.entries = Load::Loading;
    host.page.state.busy = true;
    let client = host.client.clone();
    let page = host.page.page;
    host.page.task = Some(cx.spawn(async move |this, cx| {
        let result = client.get::<Listed>(keys::list_path(page)).await;
        let _ = this.update(cx, |host, cx| {
            host.page.state.busy = false;
            host.page.state.entries = match result {
                Ok(list) => Load::Ready(if page == SettingsPage::AgentAccounts {
                    list.agents
                } else {
                    list.keys
                }),
                Err(error) => {
                    if error
                        .downcast_ref::<Refusal>()
                        .is_some_and(|r| r.code == "signed_out")
                    {
                        host.page.state.signed_in = false;
                    }
                    Load::Failed("We couldn't load this list. Try again.".into())
                }
            };
            host.changed(cx);
        });
    }));
    host.changed(cx);
}

fn change(host: &mut Host<Keys>, revoke: Option<String>, name: String, cx: &mut Cx) {
    host.page.secret = None;
    host.page.state.copy_available = false;
    host.page.state.busy = true;
    host.notice = None;
    let client = host.client.clone();
    let page = host.page.page;
    let expiry = host.page.state.expires_in_days;
    host.page.task = Some(cx.spawn(async move |this, cx| {
        let result: anyhow::Result<Option<String>> = if let Some(id) = &revoke {
            let id = String::from(js_sys::encode_uri_component(id));
            let (method, path) = if page == SettingsPage::AgentAccounts { ("POST", format!("/v1/agents/{id}/revoke")) }
                else { ("DELETE", format!("/v1/keys/{id}{}", if page == SettingsPage::ApiKeys { "?kind=api" } else { "" })) };
            client.send::<Value>(method, &path, Some(&json!({}))).await.map(|_| None)
        } else if page == SettingsPage::AgentAccounts {
            client.send::<Value>("POST", "/v1/agents/claim", Some(&json!({"claim": name}))).await.map(|_| None)
        } else {
            let mut body = json!({"name": name, "kind": if page == SettingsPage::ApiKeys { "api" } else { "signin" }});
            if page == SettingsPage::SignInKeys { body["expires_in_days"] = json!(expiry); }
            client.send::<Created>("POST", "/v1/keys", Some(&body)).await.map(|made| Some(made.token))
        };
        let _ = this.update(cx, |host, cx| {
            host.page.state.busy = false;
            host.page.state.confirm = None;
            match result {
                Ok(secret) => {
                    host.page.secret = secret;
                    host.page.state.copy_available = host.page.secret.is_some();
                    host.notice = Some(if revoke.is_some() { "Revoked." } else if page == SettingsPage::AgentAccounts { "Agent claimed." } else { "Key created." }.into());
                    load(host, cx);
                }
                Err(_) => host.notice = Some("We couldn't save this change. Refresh the list before trying again.".into()),
            }
            host.changed(cx);
        });
    }));
    host.changed(cx);
}
