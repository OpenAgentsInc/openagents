//! Claude and Data controllers over the common page host. Each is linked
//! into its own WASM module; credentials never enter browser storage.
use forms::cloud::{Action, State};
use gpui::{Context, Task};
use openagents_chat_view::settings_forms::{self as forms, Remote};
use openagents_web_client::Refusal;
use openagents_web_page_host::{Host, Intent, Page, Paints};
use serde_json::{Value, json};
use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::JsFuture;

pub struct AccountPage<const CLAUDE: bool> {
    remote: Remote,
    credential: String,
    task: Option<Task<()>>,
}

impl<const CLAUDE: bool> Page for AccountPage<CLAUDE> {
    const NAME: &'static str = if CLAUDE { "claude" } else { "data" };
    fn new() -> Self {
        Self {
            remote: Remote::default(),
            credential: String::new(),
            task: None,
        }
    }
    fn title(&self) -> String {
        if CLAUDE { "Claude" } else { "Data" }.into()
    }
    fn view(&self, paints: &Paints) -> rust_native::Node<Intent> {
        forms::cloud::view(
            CLAUDE,
            &self.remote.cloud,
            self.remote.account.signed_in,
            paints,
        )
    }
    fn open(host: &mut Host<Self>, _: &str, cx: &mut Context<Host<Self>>) {
        host.page.credential.clear();
        host.page.remote.account.signed_in = false;
        Self::load_cloud(host, cx);
    }
    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Context<Host<Self>>) {
        match intent {
            Intent::Cloud(action) => Self::cloud_action(host, action, cx),
            Intent::Account(forms::account::Action::SignIn) => host.go("/login"),
            _ => {}
        }
        host.changed(cx);
    }
}

impl<const CLAUDE: bool> AccountPage<CLAUDE> {
    fn load_cloud(host: &mut Host<Self>, cx: &mut Context<Host<Self>>) {
        host.page.remote.cloud = State::default();
        let client = host.client.clone();
        host.page.task = Some(cx.spawn(async move |this, cx| {
            let answer: anyhow::Result<Value> = client
                .get(if CLAUDE {
                    "/v1/settings/claude"
                } else {
                    "/v1/account"
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match answer {
                    Ok(value) => {
                        this.page.remote.account.signed_in = true;
                        if CLAUDE {
                            this.page.take_cloud(value);
                        } else {
                            this.page.remote.cloud.loaded = true;
                        }
                    }
                    Err(_) => {
                        this.page.remote.cloud.loaded = true;
                        this.page.remote.cloud.notice =
                            Some("Sign in to manage your account settings.".into());
                    }
                }
                this.changed(cx);
            });
        }));
    }

    fn take_cloud(&mut self, value: Value) {
        let state = &mut self.remote.cloud;
        state.loaded = true;
        state.busy = false;
        state.available = value["available"].as_bool().unwrap_or(false);
        state.subscription_allowed = value["subscription_allowed"].as_bool().unwrap_or(false);
        state.saved = value["saved"]["label"].as_str().map(|label| {
            format!(
                "{} · {}",
                label,
                value["saved"]["fingerprint"].as_str().unwrap_or("")
            )
        });
    }

    fn cloud_action(host: &mut Host<Self>, action: Action, cx: &mut Context<Host<Self>>) {
        if host.page.remote.cloud.busy {
            return;
        }
        match action {
            Action::Provider(provider) => {
                host.page.credential.clear();
                host.page.remote.cloud.provider = provider;
                host.page.remote.cloud.pasted = false;
                host.page.remote.cloud.consent = false;
            }
            Action::Consent(on) => host.page.remote.cloud.consent = on,
            Action::AskDelete(on) => host.page.remote.cloud.confirm_delete = on,
            Action::Open(path) => host.go(&path),
            Action::Paste => {
                // Start the browser permission request in the click handler.
                let Some(window) = web_sys::window() else {
                    return;
                };
                let pasted = window.navigator().clipboard().read_text();
                host.page.task = Some(cx.spawn(async move |this, cx| {
                    let value = JsFuture::from(pasted).await.ok().and_then(|v| v.as_string());
                    let _ = this.update(cx, |this, cx| {
                        if !CLAUDE { return; }
                        match value.filter(|v| !v.trim().is_empty() && v.len() <= 65_536) {
                            Some(value) => {
                                this.page.credential = value;
                                this.page.remote.cloud.pasted = true;
                                this.page.remote.cloud.notice = None;
                            }
                            None => this.page.remote.cloud.notice = Some("Allow clipboard access, copy your credential, and try Paste credential again.".into()),
                        }
                        this.changed(cx);
                    });
                }));
            }
            Action::Save | Action::Remove | Action::Delete | Action::Export => {
                if action == Action::Save
                    && (!host.page.remote.cloud.consent || host.page.credential.is_empty())
                {
                    return;
                }
                if action == Action::Delete && !host.page.remote.cloud.confirm_delete {
                    return;
                }
                let provider = if host.page.remote.cloud.provider.is_empty() {
                    "anthropic_api_key"
                } else {
                    &host.page.remote.cloud.provider
                };
                let body = if action == Action::Save {
                    json!({"material":provider,"value":std::mem::take(&mut host.page.credential),"consent":true})
                } else {
                    json!({"confirm":"delete all chats"})
                };
                host.page.remote.cloud.pasted = false;
                host.page.remote.cloud.busy = true;
                host.page.remote.cloud.notice = None;
                let client = host.client.clone();
                host.page.task = Some(cx.spawn(async move |this, cx| {
                    let answer: anyhow::Result<Value> = match action {
                        Action::Export => client.get("/v1/account/export").await,
                        Action::Delete => client.send("DELETE", "/v1/settings/data/chats", Some(&body)).await,
                        Action::Remove => client.send("DELETE", "/v1/settings/claude", None::<&()>).await,
                        _ => client.send("PUT", "/v1/settings/claude", Some(&body)).await,
                    };
                    let _ = this.update(cx, |this, cx| {
                        this.page.remote.cloud.busy = false;
                        match answer {
                            Ok(value) if action == Action::Export => {
                                this.page.remote.cloud.notice = Some(if download(&value).is_ok() { "Your export is ready to download." } else { "Your export couldn't be downloaded. Try again." }.into());
                            }
                            Ok(value) if action == Action::Delete => {
                                let left = value["remaining"].as_u64().unwrap_or(0);
                                this.page.remote.cloud.confirm_delete = false;
                                this.page.remote.cloud.notice = Some(if left == 0 { "Your chats were deleted.".into() } else { format!("{left} chats are still being answered. Delete them when the answers finish.") });
                            }
                            Ok(value) => {
                                this.page.take_cloud(value);
                                this.page.remote.cloud.consent = false;
                                this.page.remote.cloud.notice = Some(if action == Action::Remove { "Credential removed." } else { "Credential saved." }.into());
                            }
                            Err(error) => {
                                let refusal = error.downcast_ref::<Refusal>();
                                this.page.remote.cloud.notice = Some(refusal.map(|r| r.message.clone()).filter(|m| !m.is_empty()).unwrap_or_else(|| "We couldn't finish that. Try again.".into()));
                            }
                        }
                        this.changed(cx);
                    });
                }));
            }
        }
    }
}

fn download(value: &Value) -> Result<(), wasm_bindgen::JsValue> {
    let parts = js_sys::Array::new();
    parts.push(&wasm_bindgen::JsValue::from_str(
        &serde_json::to_string_pretty(value).unwrap_or_default(),
    ));
    let blob = web_sys::Blob::new_with_str_sequence(&parts)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let document = web_sys::window().unwrap().document().unwrap();
    let link = document
        .create_element("a")?
        .dyn_into::<web_sys::HtmlAnchorElement>()?;
    link.set_href(&url);
    link.set_download("openagents-export.json");
    if let Some(body) = document.body() {
        body.append_child(&link)?;
    }
    link.click();
    link.remove();
    // Let the browser start the download before releasing its URL.
    let revoke = Closure::once(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    web_sys::window()
        .unwrap()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            revoke.as_ref().unchecked_ref(),
            1_000,
        )?;
    revoke.forget();
    Ok(())
}
