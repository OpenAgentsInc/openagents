//! The account's Claude key and data; values never go to browser storage.
use super::*;
use forms::cloud::{Action, State};

impl SettingsApp {
    pub(super) fn load_cloud(&mut self, cx: &mut Context<Self>) {
        self.remote.cloud = State::default();
        let client = self.client.clone();
        self.cloud_task = Some(cx.spawn(async move |this, cx| {
            let answer: anyhow::Result<Value> = client.get("/v1/settings/claude").await;
            let _ = this.update(cx, |this, cx| {
                match answer {
                    Ok(value) => this.take_cloud(value),
                    Err(_) => {
                        this.remote.cloud.loaded = true;
                        this.remote.cloud.notice =
                            Some("We couldn't read your Claude settings. Try again.".into());
                    }
                }
                this.revision += 1;
                cx.notify();
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

    pub(super) fn cloud_action(&mut self, action: Action, cx: &mut Context<Self>) {
        if self.remote.cloud.busy {
            return;
        }
        match action {
            Action::Provider(provider) => {
                self.credential.clear();
                self.remote.cloud.provider = provider;
                self.remote.cloud.pasted = false;
                self.remote.cloud.consent = false;
            }
            Action::Consent(on) => self.remote.cloud.consent = on,
            Action::AskDelete(on) => self.remote.cloud.confirm_delete = on,
            Action::Open(path) => go(&path),
            Action::Paste => {
                // Start the browser permission request in the click handler.
                let Some(window) = web_sys::window() else {
                    return;
                };
                let pasted = window.navigator().clipboard().read_text();
                self.cloud_task = Some(cx.spawn(async move |this, cx| {
                    let value = JsFuture::from(pasted).await.ok().and_then(|v| v.as_string());
                    let _ = this.update(cx, |this, cx| {
                        if this.page != Page::Claude { return; }
                        match value.filter(|v| !v.trim().is_empty() && v.len() <= 65_536) {
                            Some(value) => {
                                this.credential = value;
                                this.remote.cloud.pasted = true;
                                this.remote.cloud.notice = None;
                            }
                            None => this.remote.cloud.notice = Some("Allow clipboard access, copy your credential, and try Paste credential again.".into()),
                        }
                        this.revision += 1;
                        cx.notify();
                    });
                }));
            }
            Action::Save | Action::Remove | Action::Delete | Action::Export => {
                if action == Action::Save
                    && (!self.remote.cloud.consent || self.credential.is_empty())
                {
                    return;
                }
                if action == Action::Delete && !self.remote.cloud.confirm_delete {
                    return;
                }
                let provider = if self.remote.cloud.provider.is_empty() {
                    "anthropic_api_key"
                } else {
                    &self.remote.cloud.provider
                };
                let body = if action == Action::Save {
                    json!({"material":provider,"value":std::mem::take(&mut self.credential),"consent":true})
                } else {
                    json!({"confirm":"delete all chats"})
                };
                self.remote.cloud.pasted = false;
                self.remote.cloud.busy = true;
                self.remote.cloud.notice = None;
                let client = self.client.clone();
                self.cloud_task = Some(cx.spawn(async move |this, cx| {
                    let answer: anyhow::Result<Value> = match action {
                        Action::Export => client.get("/v1/account/export").await,
                        Action::Delete => client.send("DELETE", "/v1/settings/data/chats", Some(&body)).await,
                        Action::Remove => client.send("DELETE", "/v1/settings/claude", None::<&()>).await,
                        _ => client.send("PUT", "/v1/settings/claude", Some(&body)).await,
                    };
                    let _ = this.update(cx, |this, cx| {
                        this.remote.cloud.busy = false;
                        match answer {
                            Ok(value) if action == Action::Export => {
                                this.remote.cloud.notice = Some(if download(&value).is_ok() { "Your export is ready to download." } else { "Your export couldn't be downloaded. Try again." }.into());
                            }
                            Ok(value) if action == Action::Delete => {
                                let left = value["remaining"].as_u64().unwrap_or(0);
                                this.remote.cloud.confirm_delete = false;
                                this.remote.cloud.notice = Some(if left == 0 { "Your chats were deleted.".into() } else { format!("{left} chats are still being answered. Delete them when the answers finish.") });
                                store::signal("oa-settings-data-deleted", "");
                            }
                            Ok(value) => {
                                this.take_cloud(value);
                                this.remote.cloud.consent = false;
                                this.remote.cloud.notice = Some(if action == Action::Remove { "Credential removed." } else { "Credential saved." }.into());
                            }
                            Err(error) => {
                                let refusal = error.downcast_ref::<Refusal>();
                                this.remote.cloud.notice = Some(refusal.map(|r| r.message.clone()).filter(|m| !m.is_empty()).unwrap_or_else(|| "We couldn't finish that. Try again.".into()));
                            }
                        }
                        this.revision += 1;
                        cx.notify();
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
