use std::time::Duration;

use base64::Engine as _;
use futures::StreamExt as _;
use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, Render, SharedString, Styled, Task, Window,
    div, prelude::*, px,
};
use openagents_chat_view::{account_usage, context_usage};
use openagents_proto::{AgentAccountsSnapshot, Chat, Connectivity, ContextUsage, HarnessId};
use openagents_web_chat_view::{
    labels::{button, footer_text},
    theme,
};
use openagents_web_client::{computer, store, web::Client};
use serde::Deserialize;
use serde_json::json;

use crate::model::{self, Accounts};

#[derive(Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
struct Target {
    device: String,
    chat: String,
    harness: Option<HarnessId>,
    generation: u64,
}

pub struct Extras {
    target: Target,
    accounts: Accounts,
    appearance_key: Option<(Option<String>, gpui::WindowAppearance)>,
    context: Option<ContextUsage>,
    context_open: bool,
    open: bool,
    connected: bool,
    loading: bool,
    pending: bool,
    forking: bool,
    _accounts: Option<Task<()>>,
    _connectivity: Option<Task<()>>,
}

fn waiting(target: &Target) {
    store::signal("oa-extras-state", &json!({
        "chat": target.chat, "generation": target.generation,
        "notice": "Allow this browser on the computer: in OpenAgents, open Settings, then Devices.",
    }).to_string());
}

impl Extras {
    pub fn new(context: &str, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            target: Target::default(),
            accounts: Accounts::default(),
            appearance_key: None,
            context: None,
            context_open: false,
            open: false,
            connected: false,
            loading: false,
            pending: false,
            forking: false,
            _accounts: None,
            _connectivity: None,
        };
        this.open(context, cx);
        this
    }

    pub fn open(&mut self, context: &str, cx: &mut Context<Self>) {
        let stored = store::get(openagents_chat_view::settings_forms::Prefs::STORAGE_KEY);
        let appearance = cx.window_appearance();
        if self.appearance_key.as_ref() != Some(&(stored.clone(), appearance)) {
            let look = openagents_chat_view::settings_forms::Prefs::from_json(stored.as_deref())
                .appearance;
            let families =
                store::get(openagents_chat_view::settings_forms::Appearance::COMPILED_KEY)
                    .and_then(|text| base64::engine::general_purpose::STANDARD.decode(text).ok())
                    .and_then(|bytes| {
                        postcard::from_bytes::<Vec<openagents_theme::ThemeFamily>>(&bytes).ok()
                    })
                    .unwrap_or_default();
            theme::set(
                look.theme_with(
                    families,
                    matches!(
                        cx.window_appearance(),
                        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
                    ),
                ),
                cx,
            );
            self.appearance_key = Some((stored, appearance));
        }
        let value: serde_json::Value = serde_json::from_str(context).unwrap_or_default();
        self.context = serde_json::from_value(value["context_usage"].clone()).ok();
        self.connected = value["connected"].as_bool().unwrap_or(false);
        self.loading = value["loading"].as_bool().unwrap_or(false);
        cx.notify();
        let target = serde_json::from_value::<Target>(value).unwrap_or_default();
        if target == self.target {
            return;
        }
        self.target = target;
        self.open = false;
        self.restart(cx);
    }

    fn publish(&self, fork: Option<Chat>, notice: Option<String>) {
        store::signal(
            "oa-extras-state",
            &json!({
                "chat": self.target.chat, "generation": self.target.generation,
                "fork": fork, "notice": notice, "open": self.open,
            })
            .to_string(),
        );
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        self._accounts = None;
        self._connectivity = None;
        self.accounts = Accounts::default();
        self.pending = false;
        self.forking = false;
        if self.target.device.is_empty() || self.target.chat.is_empty() {
            return;
        }
        let target = self.target.clone();
        self._accounts = Some(cx.spawn(async move |this, cx| {
            loop {
                let Ok(revision) = this.read_with(cx, |this, _| this.accounts.revision) else {
                    break;
                };
                let result = async {
                    let asked = target.clone();
                    let host = computer::connect(&Client::from_page(), &target.device, move || {
                        waiting(&asked)
                    })
                    .await?;
                    serde_json::from_value::<AgentAccountsSnapshot>(
                        host.call("ListAgentAccounts", json!({})).await?,
                    )
                    .map_err(anyhow::Error::from)
                }
                .await
                .map_err(|e| e.to_string());
                let failed = result.is_err();
                let stop = this
                    .update(cx, |this, cx| {
                        if this.target != target {
                            return true;
                        }
                        let accepted = this.accounts.read(revision, result);
                        if accepted {
                            this.publish(None, None);
                            cx.notify();
                        }
                        accepted && failed
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
            }
        }));
        let target = self.target.clone();
        self._connectivity = Some(cx.spawn(async move |this, cx| {
            let asked = target.clone();
            let Ok(host) = computer::connect(&Client::from_page(), &target.device, move || {
                waiting(&asked)
            })
            .await
            else {
                return;
            };
            let Ok(mut watch) = host.watch("WatchConnectivity", json!({})) else {
                return;
            };
            while let Some(Ok(value)) = watch.next().await {
                let Ok(state) = serde_json::from_value::<Connectivity>(value) else {
                    continue;
                };
                if this
                    .update(cx, |this, cx| {
                        if this.target != target {
                            return;
                        }
                        this.pending = model::pending(&state, &target.chat);
                        this.publish(None, None);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn switch(&mut self, harness: HarnessId, account: String, cx: &mut Context<Self>) {
        if self.accounts.busy {
            return;
        }
        let revision = self.accounts.begin();
        let target = self.target.clone();
        cx.spawn(async move |this, cx| {
            let result = async {
                let host = computer::connect(&Client::from_page(), &target.device, || {}).await?;
                serde_json::from_value::<AgentAccountsSnapshot>(
                    host.call(
                        "ActivateAgentAccount",
                        json!({"harness": harness, "accountId": account}),
                    )
                    .await?,
                )
                .map_err(anyhow::Error::from)
            }
            .await
            .map_err(|e| e.to_string());
            this.update(cx, |this, cx| {
                if this.target != target {
                    return;
                }
                this.accounts.switched(revision, result);
                this.publish(None, None);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn fork(&mut self, cx: &mut Context<Self>) {
        if self.accounts.busy || self.target.chat.is_empty() {
            self.publish(None, None);
            return;
        }
        self.accounts.begin();
        self.forking = true;
        self.publish(None, None);
        let target = self.target.clone();
        cx.spawn(async move |this, cx| {
            let result =
                async {
                    let host =
                        computer::connect(&Client::from_page(), &target.device, || {}).await?;
                    serde_json::from_value::<Chat>(host.call("ForkChat", json!({
                    "chatId": uuid::Uuid::new_v4().to_string(), "sourceChatId": target.chat,
                })).await?).map_err(anyhow::Error::from)
                }
                .await;
            this.update(cx, |this, cx| {
                if this.target != target {
                    return;
                }
                this.accounts.busy = false;
                this.forking = false;
                match result {
                    Ok(chat) => this.publish(Some(chat), None),
                    Err(error) => this.publish(None, Some(error.to_string())),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Extras {
    fn card(&mut self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let mut card = div()
            .id("account-popover")
            .w(px(320.))
            .h(px(360.))
            .flex_none()
            .p(px(10.))
            .rounded(px(10.))
            .bg(theme::surface_card())
            .border_1()
            .border_color(theme::border())
            .font_family(theme::SANS)
            .text_color(theme::text())
            .flex()
            .flex_col()
            .gap(px(6.))
            .overflow_y_scroll()
            .child(
                button("accounts-close", "Close", false).on_click(cx.listener(|this, _, _, cx| {
                    this.open = false;
                    this.publish(None, None);
                    cx.notify();
                    store::signal("oa-extras-close", "");
                })),
            );
        if self.context_open {
            return card.child(context_usage::card(self.context, &theme::desktop()));
        }
        let accounts: Vec<_> = self
            .accounts
            .snapshot
            .accounts
            .iter()
            .filter(|a| Some(a.harness) == self.target.harness)
            .collect();
        if accounts.is_empty() && self.accounts.error.is_none() {
            card = card.child(footer_text(if self.accounts.loaded {
                "No account usage available"
            } else {
                "Loading accounts…"
            }));
        }
        for account in accounts {
            let id = account.id.clone();
            let harness = account.harness;
            let enabled = !account.active && account.switchable && !self.accounts.busy;
            let name = account
                .email
                .as_deref()
                .or(account.display_name.as_deref())
                .unwrap_or("Account");
            card = card.child(
                button(
                    SharedString::from(format!("account-{id}")),
                    format!("{}{}", if account.active { "✓ " } else { "" }, name),
                    account.active,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if enabled {
                        this.switch(harness, id.clone(), cx);
                    }
                })),
            );
            for window in &account.usage_windows {
                card = card.child(footer_text(format!(
                    "{} · {:.0}%{}",
                    window.label,
                    window.used_fraction * 100.,
                    window
                        .resets_at
                        .map(|at| format!(" · resets {}", at.format("%H:%M UTC")))
                        .unwrap_or_default()
                )));
            }
            if let Some(error) = &account.usage_error {
                card = card.child(footer_text(error.clone()));
            }
        }
        for warning in &self.accounts.snapshot.warnings {
            if Some(warning.harness) == self.target.harness {
                card = card.child(footer_text(warning.message.clone()));
            }
        }
        if let Some(error) = &self.accounts.error {
            card = card.child(footer_text(error.clone())).child(
                button("accounts-retry", "Try again", false).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        if !this.accounts.busy {
                            this.restart(cx);
                        }
                    },
                )),
            );
        }
        card
    }
}

impl Extras {
    fn toggle(&mut self, context: bool, cx: &mut Context<Self>) {
        self.open = !self.open || self.context_open != context;
        self.context_open = context;
        self.publish(None, None);
        cx.notify();
    }

    fn retry(&mut self, cx: &mut Context<Self>) {
        let target = self.target.clone();
        cx.spawn(async move |this, cx| {
            let result = async {
                let host = computer::connect(&Client::from_page(), &target.device, || {}).await?;
                host.call("RetryDelivery", json!({"chatId": target.chat}))
                    .await
            }
            .await;
            this.update(cx, |this, cx| {
                if this.target == target {
                    this.publish(None, result.err().map(|e| e.to_string()));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let fraction = self
            .accounts
            .snapshot
            .accounts
            .iter()
            .find(|a| a.active && Some(a.harness) == self.target.harness)
            .and_then(account_usage::used_fraction);
        let color = account_usage::usage_color(
            account_usage::usage_level(fraction.unwrap_or(0.)),
            &theme::desktop(),
        );
        let mut row = div().flex().items_center().gap(px(5.)).child(
            context_usage::ring_chip(
                "account-usage",
                fraction.unwrap_or(0.),
                color,
                theme::text_muted(),
                fraction
                    .map(|f| format!("{:.0}%", f * 100.))
                    .unwrap_or_else(|| "Accounts".into()),
                self.open && !self.context_open,
                &theme::desktop(),
            )
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle(false, cx))),
        );
        if context_usage::has_window(self.context) {
            row = row.child(
                context_usage::chip(
                    self.context,
                    self.open && self.context_open,
                    &theme::desktop(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle(true, cx))),
            );
        }
        let connected = self.connected;
        row = row.child(button("connection-pill", if self.loading { "Connecting…" } else if connected { "Connected" } else { "Reconnect" }, false)
            .text_color(if connected { theme::text_faint() } else { theme::danger() })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, _| {
                if !connected {
                    store::signal("oa-extras-state", &json!({"chat":this.target.chat, "generation":this.target.generation, "action":"reconnect"}).to_string());
                }
            })));
        row = row.child(
            button(
                "fork-chat",
                if self.forking {
                    "Forking…"
                } else {
                    "Fork chat"
                },
                false,
            )
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.fork(cx))),
        );
        let mut footer = div()
            .w_full()
            .h(px(76.))
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .justify_end()
            .gap(px(6.))
            .child(row);
        if self.pending {
            footer = footer.child(
                button("retry-delivery", "Not delivered — click to retry", false)
                    .text_color(theme::danger())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.retry(cx))),
            );
        }
        footer
    }
}

impl Render for Extras {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let card = self.open.then(|| self.card(cx));
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_end()
            .gap(px(6.))
            .font_family(theme::SANS)
            .text_color(theme::text())
            .children(card)
            .child(self.footer(cx))
    }
}
