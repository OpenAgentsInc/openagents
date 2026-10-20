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
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[derive(Deserialize)]
struct Listed {
    #[serde(default)]
    keys: Vec<keys::Entry>,
    #[serde(default)]
    agents: Vec<keys::Entry>,
}

#[derive(Deserialize)]
struct Own {
    providers: Option<Vec<keys::Provider>>,
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
            paste_available: clipboard().is_some(),
            billing_enabled: openagents_web_client::features::on("billing"),
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
            Action::Settings => host.go("/settings?ui=gpui"),
            Action::SignIn => host.go(&format!(
                "/login?next=%2Fsettings%2F{}%3Fui%3Dgpui",
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
                if host.page.page == SettingsPage::ApiKeys && host.page.state.actions.is_empty() {
                    host.notice = Some("Choose at least one permission for the key.".into());
                    return;
                }
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
            Action::Scope(scope) if !host.page.state.busy => {
                if keys::SCOPES.iter().any(|(s, _)| *s == scope)
                    && (scope != "billing" || host.page.state.billing_enabled)
                {
                    if host.page.state.actions.contains(&scope) {
                        host.page.state.actions.retain(|s| s != &scope);
                    } else {
                        host.page.state.actions.push(scope);
                    }
                }
            }
            Action::Limit if !host.page.state.busy => host.ask(
                Prompt {
                    purpose: "limit".into(),
                    label: "Monthly limit in dollars (empty means no limit)".into(),
                    placeholder: "20 or 7.50".into(),
                    value: host.page.state.monthly_limit.clone(),
                },
                cx,
            ),
            Action::ProviderPaste(id) if !host.page.state.busy => {
                let read = clipboard()
                    .and_then(|(clipboard, read)| read.call0(&clipboard).ok())
                    .map(|v| js_sys::Promise::resolve(&v));
                if let Some(read) = read {
                    provider(host, id, Some(read), cx);
                } else {
                    host.notice = Some(
                        "Allow clipboard access in your browser, then choose Paste key again."
                            .into(),
                    );
                }
            }
            Action::ProviderRemove(id)
                if !host.page.state.busy
                    && host.page.state.confirm.as_ref() == Some(&format!("provider:{id}")) =>
            {
                provider(host, id, None, cx)
            }
            _ => {}
        }
    }

    fn submit(host: &mut Host<Self>, prompt: &Prompt, text: String, cx: &mut Cx) {
        if host.page.state.busy {
            return;
        }
        if prompt.purpose == "limit" {
            let text = text.trim().trim_start_matches('$').trim();
            if !text.is_empty()
                && (text.len() > 32
                    || !text.chars().all(|c| c.is_ascii_digit() || c == '.')
                    || text.split_once('.').is_some_and(|(_, f)| f.len() > 2)
                    || !text
                        .parse::<f64>()
                        .is_ok_and(|n| n > 0. && n.trunc() <= 100_000.))
            {
                host.notice = Some(
                    "Enter dollars, such as 20 or 7.50, or leave it empty for no limit.".into(),
                );
                return;
            }
            host.page.state.monthly_limit = text.into();
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
        let providers = if page == SettingsPage::ApiKeys && result.is_ok() {
            client
                .get::<Own>("/v1/keys/own")
                .await
                .ok()
                .and_then(|v| v.providers)
        } else {
            None
        };
        let _ = this.update(cx, |host, cx| {
            host.page.state.busy = false;
            host.page.state.providers = providers;
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
    let actions = host.page.state.actions.clone();
    let monthly_limit = host.page.state.monthly_limit.clone();
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
            else { body["actions"] = json!(actions); body["monthly_limit"] = json!(monthly_limit); }
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

/// The clipboard is read only in the Paste key activation, while that browser
/// gesture still grants access. Its contents never enter a prompt or view.
fn clipboard() -> Option<(JsValue, js_sys::Function)> {
    let navigator = web_sys::window()?.navigator();
    let clipboard =
        js_sys::Reflect::get(navigator.as_ref(), &JsValue::from_str("clipboard")).ok()?;
    let read = js_sys::Reflect::get(&clipboard, &JsValue::from_str("readText"))
        .ok()?
        .dyn_into()
        .ok()?;
    Some((clipboard, read))
}

fn provider(host: &mut Host<Keys>, id: String, read: Option<js_sys::Promise>, cx: &mut Cx) {
    if !host
        .page
        .state
        .providers
        .as_ref()
        .is_some_and(|v| v.iter().any(|p| p.id == id))
    {
        return;
    }
    host.page.secret = None;
    host.page.state.copy_available = false;
    host.page.state.busy = true;
    let client = host.client.clone();
    let saving = read.is_some();
    host.page.task = Some(cx.spawn(async move |this, cx| {
        let path = format!("/v1/keys/own/{}", String::from(js_sys::encode_uri_component(&id)));
        let result: anyhow::Result<Value> = if let Some(read) = read {
            match JsFuture::from(read).await.ok().and_then(|v| v.as_string()) {
                Some(key) if !key.trim().is_empty() && key.len() <= 16_384 => client.send("PUT", &path, Some(&json!({"key":key}))).await,
                _ => Err(anyhow::anyhow!("clipboard unavailable")),
            }
        } else { client.send("DELETE", &path, Some(&json!({}))).await };
        let _ = this.update(cx, |host, cx| {
            host.page.state.busy = false;
            host.page.state.confirm = None;
            match result {
                Ok(_) => { host.notice = Some(if saving { "Provider key saved." } else { "Provider key removed." }.into()); load(host, cx); }
                Err(_) => host.notice = Some(if saving {
                    "We couldn't save the provider key. Allow clipboard access and check the key, then try again."
                } else { "We couldn't remove the provider key. Refresh the list and try again." }.into()),
            }
            host.changed(cx);
        });
    }));
    host.changed(cx);
}
