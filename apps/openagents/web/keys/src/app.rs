//! Browser transport and input for the shared credential forms.

use gpui::{Context, Entity, IntoElement, Render, Styled, Task, Window, div, prelude::*, px};
use openagents_chat_view::{
    rust_native_gpui,
    settings_forms::{
        Intent, Load, Page, Paints,
        keys::{self, Action},
    },
    theme::Theme,
};
use openagents_web_chat_view::{
    composer::{Submitted, TextInput},
    theme,
};
use openagents_web_client::{store, web::Client};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Listed {
    #[serde(default)]
    keys: Vec<keys::Entry>,
    #[serde(default)]
    agents: Vec<keys::Entry>,
}

// A creation response is never serialized, logged, or passed to the shared view.
#[derive(Deserialize)]
struct Created {
    token: String,
}

pub struct KeysApp {
    client: Client,
    page: Page,
    state: keys::State,
    secret: Option<String>,
    name: Entity<TextInput>,
    task: Option<Task<()>>,
    copy_task: Option<Task<()>>,
    revision: u64,
}

impl KeysApp {
    pub fn new(page: Page, signed_in: bool, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| TextInput::new("Key name or claim code", cx));
        cx.subscribe(&name, |this, _, _: &Submitted, cx| {
            this.action(Action::Create, cx)
        })
        .detach();
        let mut this = Self {
            client: Client::from_page(),
            page,
            state: keys::State {
                signed_in,
                ..Default::default()
            },
            secret: None,
            name,
            task: None,
            copy_task: None,
            revision: 0,
        };
        this.refresh(cx);
        this
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.task = None;
        self.copy_task = None;
        self.secret = None;
        self.state.copy_available = false;
        self.state.busy = false;
        self.name.update(cx, |field, cx| {
            field.take(cx);
        });
        cx.notify();
    }

    pub fn open(&mut self, page: Page, signed_in: bool, cx: &mut Context<Self>) {
        self.clear(cx);
        self.page = page;
        self.state = keys::State {
            signed_in,
            ..Default::default()
        };
        self.refresh(cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.state.signed_in {
            return;
        }
        self.state.entries = Load::Loading;
        self.state.busy = true;
        let client = self.client.clone();
        let page = self.page;
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = client.get::<Listed>(keys::list_path(page)).await;
            let _ = this.update(cx, |this, cx| {
                this.state.busy = false;
                this.state.entries = match result {
                    Ok(list) => Load::Ready(if page == Page::AgentAccounts {
                        list.agents
                    } else {
                        list.keys
                    }),
                    Err(_) => Load::Failed("We couldn't load this list. Try again.".into()),
                };
                this.revision += 1;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn action(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Dismiss => {
                self.secret = None;
                self.state.copy_available = false;
            }
            Action::Confirm(id) => self.state.confirm = id,
            Action::Expiry(days) => self.state.expires_in_days = days,
            Action::Refresh if !self.state.busy => self.refresh(cx),
            Action::Copy => {
                let Some(secret) = self.secret.clone() else {
                    return;
                };
                self.copy_task = Some(cx.spawn(async move |this, cx| {
                    let result = if let Some(window) = web_sys::window() {
                        wasm_bindgen_futures::JsFuture::from(
                            window.navigator().clipboard().write_text(&secret),
                        )
                        .await
                        .is_ok()
                    } else {
                        false
                    };
                    let _ = this.update(cx, |this, cx| {
                        this.state.notice = Some(
                            if result {
                                "Key copied."
                            } else {
                                "We couldn't copy the key. Try Copy key again."
                            }
                            .into(),
                        );
                        this.revision += 1;
                        cx.notify();
                    });
                }));
            }
            Action::Create if !self.state.busy => {
                let name = self.name.read(cx).text().trim().to_owned();
                if name.is_empty() {
                    self.state.notice = Some(
                        if self.page == Page::AgentAccounts {
                            "Enter the claim code."
                        } else {
                            "Name the key."
                        }
                        .into(),
                    );
                } else if name.len() > 256 {
                    self.state.notice = Some("Use a shorter name or code.".into());
                } else {
                    self.change(None, name, cx);
                }
            }
            Action::Revoke(id) if !self.state.busy && self.state.confirm.as_ref() == Some(&id) => {
                self.change(Some(id), String::new(), cx)
            }
            Action::SignIn => {
                if let Some(w) = web_sys::window() {
                    let _ = w.location().set_href(&format!(
                        "/login?next=%2Fgpui%3Fsettings%3D{}",
                        self.page.slug()
                    ));
                }
            }
            Action::ProviderKeys => {
                if let Some(w) = web_sys::window() {
                    let _ = w
                        .location()
                        .set_href("/settings/api-keys?ui=html#api-keys-own");
                }
            }
            _ => {}
        }
        self.revision += 1;
        cx.notify();
    }

    fn change(&mut self, revoke: Option<String>, name: String, cx: &mut Context<Self>) {
        self.secret = None;
        self.state.copy_available = false;
        self.state.notice = None;
        self.state.busy = true;
        let client = self.client.clone();
        let page = self.page;
        let expiry = self.state.expires_in_days;
        self.task = Some(cx.spawn(async move |this, cx| {
            let result: anyhow::Result<Option<String>> = if let Some(id) = &revoke {
                let id = String::from(js_sys::encode_uri_component(id));
                let (method, path) = if page == Page::AgentAccounts { ("POST", format!("/v1/agents/{id}/revoke")) }
                    else { ("DELETE", format!("/v1/keys/{id}{}", if page == Page::ApiKeys { "?kind=api" } else { "" })) };
                client.send::<Value>(method, &path, Some(&json!({}))).await.map(|_| None)
            } else if page == Page::AgentAccounts {
                client.send::<Value>("POST", "/v1/agents/claim", Some(&json!({"claim": name}))).await.map(|_| None)
            } else {
                let mut body = json!({"name": name, "kind": if page == Page::ApiKeys { "api" } else { "signin" }});
                if page == Page::SignInKeys { body["expires_in_days"] = json!(expiry); }
                client.send::<Created>("POST", "/v1/keys", Some(&body)).await.map(|made| Some(made.token))
            };
            let _ = this.update(cx, |this, cx| {
                this.state.busy = false;
                this.state.confirm = None;
                match result {
                    Ok(secret) => {
                        this.secret = secret;
                        this.state.copy_available = this.secret.is_some();
                        this.name.update(cx, |field, cx| { field.take(cx); });
                        this.state.notice = Some(if revoke.is_some() { "Revoked." } else if page == Page::AgentAccounts { "Agent claimed." } else { "Key created." }.into());
                        this.refresh(cx);
                    }
                    Err(_) => this.state.notice = Some("We couldn't save this change. Refresh the list before trying again.".into()),
                }
                this.revision += 1;
                cx.notify();
            });
        }));
    }
}

impl Render for KeysApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let paints = paints(&theme::desktop());
        let root = keys::view(self.page, &self.state, &paints);
        let rust_native::Element::Stack { children, .. } = root.element else {
            unreachable!()
        };
        let body = children
            .into_iter()
            .map(|node| {
                if node.key == "keys-input" {
                    return div()
                        .border_1()
                        .border_color(theme::border())
                        .rounded(px(8.))
                        .p(px(12.))
                        .bg(theme::input_bg())
                        .child(self.name.clone())
                        .into_any_element();
                }
                rust_native_gpui::mount_root(
                    &node,
                    "keys",
                    self.revision,
                    |this: &mut Self, activation, cx| {
                        let root = keys::view(this.page, &this.state, &paints);
                        if let Some(Intent::Keys(action)) = find_intent(&root, &activation.node) {
                            this.action(action, cx);
                        }
                    },
                    cx,
                )
            })
            .collect::<Vec<_>>();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .text_color(theme::text())
            .font_family(theme::SANS)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(12.))
                    .p(px(16.))
                    .child(
                        div()
                            .id("keys-back")
                            .cursor_pointer()
                            .on_click(|_, _, _| store::signal("oa-settings-close", ""))
                            .child("← Back to chat"),
                    )
                    .children(
                        Page::ALL
                            .into_iter()
                            .filter(|page| {
                                *page != Page::Earn
                                    || openagents_web_client::features::on(
                                        openagents_web_client::features::EARN,
                                    )
                            })
                            .map(|page| {
                                div()
                                    .id(gpui::SharedString::from(format!(
                                        "keys-nav-{}",
                                        page.slug()
                                    )))
                                    .cursor_pointer()
                                    .on_click(move |_, _, _| {
                                        store::signal("oa-settings-page", page.slug())
                                    })
                                    .child(page.label())
                            }),
                    ),
            )
            .child(
                div()
                    .id("keys-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .max_w(px(760.))
                            .w_full()
                            .mx_auto()
                            .p(px(24.))
                            .flex()
                            .flex_col()
                            .gap(px(20.))
                            .children(body),
                    ),
            )
    }
}

fn find_intent(root: &rust_native::Node<Intent>, key: &str) -> Option<Intent> {
    use rust_native::Element;
    if root.key == key {
        return match &root.element {
            Element::Button {
                intent,
                enabled: true,
                ..
            }
            | Element::Choice {
                intent,
                enabled: true,
                ..
            } => Some(intent.clone()),
            _ => None,
        };
    }
    match &root.element {
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Choice { children, .. }
        | Element::Dialog { children, .. } => children.iter().find_map(|c| find_intent(c, key)),
        _ => None,
    }
}

/// The forms' colors from the theme the page draws with.
fn paints(theme: &Theme) -> Paints {
    let color = |c: gpui::Hsla| {
        let rgba = c.to_rgb();
        rust_native::style::Color {
            red: (rgba.r * 255.).round() as u8,
            green: (rgba.g * 255.).round() as u8,
            blue: (rgba.b * 255.).round() as u8,
            alpha: (rgba.a * 255.).round() as u8,
        }
    };
    Paints {
        text: color(theme.text),
        muted: color(theme.text_muted),
        faint: color(theme.text_faint),
        card: color(theme.surface_card),
        border: color(theme.border),
        danger: color(theme.danger),
    }
}
