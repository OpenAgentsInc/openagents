//! Transcript chrome for a chat on a paired computer (#12270).
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use futures::StreamExt as _;
use gpui::{
    AnyElement, ClickEvent, Context, Image, ImageFormat, IntoElement, ListOffset, ParentElement,
    SharedString, Styled, Task, div, prelude::*, px,
};
use openagents_chat_view::markdown::render::{LinkOutcome, LinkUi};
use openagents_chat_view::{
    account_usage, context_usage, rail, todo_panel, tool_images, tool_progress,
};
use openagents_doc::MessagePart;
use openagents_proto::{
    AgentAccountsSnapshot, Chat, Connectivity, ContextUsage, TodoStatus, WorkspaceImageChunk,
};
use openagents_web_chat_view::transcript::{self, button, footer_text};
use openagents_web_chat_view::{markdown, theme};
use openagents_web_client::computer;
use serde_json::json;

use super::ChatApp;

#[derive(Default)]
pub(super) struct Extras {
    chat: Option<Chat>,
    pub progress_second: i64,
    pub context: Option<ContextUsage>,
    pub connected: bool,
    connectivity: Option<Connectivity>,
    accounts: AgentAccountsSnapshot,
    account_error: Option<String>,
    accounts_open: bool,
    context_open: bool,
    todo_closed: Option<bool>,
    todo_all: bool,
    busy: bool,
    _accounts: Option<Task<()>>,
    _connectivity: Option<Task<()>>,
    /// Expanded groups only. Dropped on collapse or chat change.
    outputs: HashMap<String, String>,
    images: HashMap<String, Arc<Image>>,
    loading: HashSet<String>,
    errors: HashMap<String, String>,
}

impl ChatApp {
    pub(super) fn selected_chat(&self) -> Option<Chat> {
        let id = self.current.as_deref()?;
        self.registry
            .as_ref()
            .and_then(|r| r.read(|r| r.chat(id)))
            .or_else(|| self.pending_fork.clone().filter(|c| c.id == id))
            .or_else(|| self.extras.chat.clone().filter(|c| c.id == id))
    }

    pub(super) fn start_extras(&mut self, chat: &Chat, cx: &mut Context<Self>) {
        self.extras.chat = Some(chat.clone());
        let generation = self.generation;
        let client = self.client.clone();
        let device = chat.device_id.clone();
        self.extras._accounts = Some(cx.spawn(async move |this, cx| {
            loop {
                let result = async {
                    let host = computer::connect(&client, &device, || {}).await?;
                    serde_json::from_value::<AgentAccountsSnapshot>(
                        host.call("ListAgentAccounts", json!({})).await?,
                    )
                    .map_err(anyhow::Error::from)
                }
                .await;
                if this
                    .update(cx, |this, cx| {
                        if this.generation != generation {
                            return;
                        }
                        match result {
                            Ok(accounts) => {
                                this.extras.accounts = accounts;
                                this.extras.account_error = None;
                            }
                            Err(error) => this.extras.account_error = Some(error.to_string()),
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
            }
        }));
        let client = self.client.clone();
        let device = chat.device_id.clone();
        self.extras._connectivity = Some(cx.spawn(async move |this, cx| {
            let result = async {
                let host = computer::connect(&client, &device, || {}).await?;
                host.watch("WatchConnectivity", json!({}))
            }
            .await;
            if let Ok(mut watch) = result {
                while let Some(Ok(value)) = watch.next().await {
                    if let Ok(connectivity) = serde_json::from_value::<Connectivity>(value) {
                        if this
                            .update(cx, |this, cx| {
                                if this.generation == generation {
                                    this.extras.connectivity = Some(connectivity);
                                    cx.notify();
                                }
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }));
        if let Some(name) =
            openagents_proto::saved_agent::agent_name(chat.config.as_ref(), chat.title.as_deref())
        {
            self.composer.update(cx, |input, _| {
                input.set_placeholder(format!("Message {name}"))
            });
        }
    }

    pub(super) fn render_identity(&self) -> Option<AnyElement> {
        let chat = self.selected_chat()?;
        let config = chat.config.as_ref()?;
        let name = openagents_proto::saved_agent::agent_name(Some(config), chat.title.as_deref())?;
        Some(
            transcript::chip(
                "icons/openagents-logo.svg",
                openagents_proto::saved_agent::identity_line(&name, config.harness),
                theme::text_muted(),
            )
            .into_any_element(),
        )
    }

    pub(super) fn render_todos(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let items = todo_panel::latest_todo(&self.entries)?;
        let summary = todo_panel::TodoSummary::of(&items);
        let range = if self.extras.todo_all {
            0..items.len()
        } else {
            todo_panel::focus_window(&items)
        };
        let finished = summary.finished();
        let mut panel = div()
            .w_full()
            .max_w(px(transcript::CONTENT_WIDTH))
            .px(px(16.))
            .py(px(8.))
            .rounded(px(12.))
            .bg(theme::surface_raised())
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(
                button(
                    "todo-toggle",
                    format!("Plan · {}/{}", summary.done, summary.total),
                    false,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    let finished = todo_panel::latest_todo(&this.entries)
                        .is_some_and(|items| todo_panel::TodoSummary::of(&items).finished());
                    this.extras.todo_closed = Some(!this.extras.todo_closed.unwrap_or(finished));
                    cx.notify();
                })),
            );
        if !self.extras.todo_closed.unwrap_or(finished) {
            panel = panel.children(items[range].iter().map(|item| {
                div()
                    .text_size(px(12.5))
                    .text_color(if item.status() == TodoStatus::Completed {
                        theme::text_faint()
                    } else {
                        theme::text_muted()
                    })
                    .child(format!(
                        "{}  {}",
                        match item.status() {
                            TodoStatus::Completed => "✓",
                            TodoStatus::InProgress => "●",
                            TodoStatus::Pending => "○",
                        },
                        item.text
                    ))
            }));
            if items.len() > todo_panel::FOLD_ABOVE {
                panel = panel.child(
                    button(
                        "todo-all",
                        if self.extras.todo_all {
                            "Show less"
                        } else {
                            "Show all"
                        },
                        false,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.extras.todo_all = !this.extras.todo_all;
                        cx.notify();
                    })),
                );
            }
        }
        Some(panel.into_any_element())
    }

    pub(super) fn render_extras(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.compose.read(cx).host()?;
        let chat = self.selected_chat();
        let harness = chat
            .as_ref()
            .and_then(|c| c.config.as_ref())
            .map(|c| c.harness);
        let accounts: Vec<_> = self
            .extras
            .accounts
            .accounts
            .iter()
            .filter(|a| Some(a.harness) == harness)
            .collect();
        let fraction = accounts
            .iter()
            .find(|a| a.active)
            .and_then(|a| account_usage::used_fraction(a));
        let usage_color = account_usage::usage_color(
            account_usage::usage_level(fraction.unwrap_or(0.)),
            theme::desktop(),
        );
        let mut row = div().flex().items_center().gap(px(5.));
        if !accounts.is_empty() || self.extras.account_error.is_some() {
            row = row.child(
                context_usage::ring_chip(
                    "account-usage",
                    fraction.unwrap_or(0.),
                    usage_color,
                    theme::text_muted(),
                    fraction
                        .map(|f| format!("{:.0}%", f * 100.))
                        .unwrap_or_else(|| "Accounts".into()),
                    self.extras.accounts_open,
                    theme::desktop(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.extras.accounts_open = !this.extras.accounts_open;
                    this.extras.context_open = false;
                    cx.notify();
                })),
            );
        }
        if context_usage::has_window(self.extras.context) {
            row = row.child(
                context_usage::chip(
                    self.extras.context,
                    self.extras.context_open,
                    theme::desktop(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.extras.context_open = !this.extras.context_open;
                    this.extras.accounts_open = false;
                    cx.notify();
                })),
            );
        }
        let connected = self.extras.connected;
        let label = if self.loading_chat {
            "Connecting…"
        } else if connected {
            "Connected"
        } else {
            "Reconnect"
        };
        row = row.child(
            button("connection-pill", label, false)
                .text_color(if connected {
                    theme::text_faint()
                } else {
                    theme::danger()
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if !connected {
                        this.load(this.current.clone(), cx);
                    }
                })),
        );
        row = row.child(
            button(
                "fork-chat",
                if self.extras.busy {
                    "Forking…"
                } else {
                    "Fork chat"
                },
                false,
            )
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.fork_chat(cx))),
        );
        let mut footer = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .child(row);
        if self.extras.context_open {
            footer = footer.child(context_usage::card(self.extras.context, theme::desktop()));
        }
        if self.extras.accounts_open {
            let mut card = div()
                .w(px(320.))
                .p(px(10.))
                .rounded(px(10.))
                .bg(theme::surface_card())
                .border_1()
                .border_color(theme::border())
                .flex()
                .flex_col()
                .gap(px(6.));
            for account in accounts {
                let id = account.id.clone();
                let harness = account.harness;
                let enabled = !account.active && account.switchable && !self.extras.busy;
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
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            if enabled {
                                this.switch_account(harness, id.clone(), cx);
                            }
                        },
                    )),
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
            for warning in &self.extras.accounts.warnings {
                if Some(warning.harness) == harness {
                    card = card.child(footer_text(warning.message.clone()));
                }
            }
            if let Some(error) = &self.extras.account_error {
                card = card.child(footer_text(error.clone()));
            }
            footer = footer.child(card);
        }
        let pending = self
            .extras
            .connectivity
            .as_ref()
            .and_then(|c| {
                c.chats
                    .iter()
                    .find(|c| Some(&c.chat_id) == self.current.as_ref())
            })
            .is_some_and(|c| c.pending_pushes > 0 && !c.delivery_live);
        if pending {
            footer = footer.child(
                button("retry-delivery", "Not delivered — click to retry", false)
                    .text_color(theme::danger())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(host) = this.compose.read(cx).host().cloned() {
                            this.computer_call(
                                host.device,
                                "RetryDelivery",
                                json!({"chatId": host.chat}),
                                None,
                                cx,
                            );
                        }
                    })),
            );
        }
        Some(footer.into_any_element())
    }

    fn switch_account(
        &mut self,
        harness: openagents_proto::HarnessId,
        account: String,
        cx: &mut Context<Self>,
    ) {
        let Some(host) = self.compose.read(cx).host().cloned() else {
            return;
        };
        self.extras.busy = true;
        let client = self.client.clone();
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let result = async {
                let computer = computer::connect(&client, &host.device, || {}).await?;
                Ok::<_, anyhow::Error>(serde_json::from_value::<AgentAccountsSnapshot>(
                    computer
                        .call(
                            "ActivateAgentAccount",
                            json!({"harness": harness, "accountId": account}),
                        )
                        .await?,
                )?)
            }
            .await;
            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.extras.busy = false;
                match result {
                    Ok(accounts) => {
                        this.extras.accounts = accounts;
                        this.extras.account_error = None;
                    }
                    Err(error) => this.extras.account_error = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn fork_chat(&mut self, cx: &mut Context<Self>) {
        if self.extras.busy {
            return;
        }
        let Some(host) = self.compose.read(cx).host().cloned() else {
            return;
        };
        self.extras.busy = true;
        let client = self.client.clone();
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let result = async {
                let computer = computer::connect(&client, &host.device, || {}).await?;
                Ok::<_, anyhow::Error>(serde_json::from_value::<Chat>(computer.call("ForkChat", json!({"chatId": uuid::Uuid::new_v4().to_string(), "sourceChatId": host.chat})).await?)?)
            }.await;
            this.update(cx, |this, cx| {
                if this.generation != generation { return; }
                this.extras.busy = false;
                match result {
                    Ok(chat) => {
                        let id = chat.id.clone();
                        // The RPC can answer before the registry frame arrives.
                        this.meta.insert(id.clone(), crate::model::Meta { computer: (chat.device_id.clone(), this.meta.get(&host.chat).map(|m| m.computer.1.clone()).unwrap_or_default()), ..Default::default() });
                        this.pending_fork = Some(chat);
                        super::route::push(Some(&id));
                        this.load(Some(id), cx);
                    }
                    Err(error) => this.notice = Some(error.to_string()),
                }
                cx.notify();
            }).ok();
        }).detach();
        cx.notify();
    }

    pub(super) fn render_progress(&self) -> Option<AnyElement> {
        let chats = self
            .registry
            .as_ref()
            .map(|r| r.read(|r| r.chats()))
            .unwrap_or_default();
        let progress = tool_progress::running(&self.entries, &chats, chrono::Utc::now())?;
        Some(
            footer_text(format!("{}… {}s", progress.label, progress.elapsed_secs))
                .into_any_element(),
        )
    }

    pub(super) fn render_rail(&self, width: f32, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !rail::rail_visible(width) {
            return None;
        }
        let ticks = rail::rail_ticks(&self.entries, &[]);
        let rows: Vec<_> = ticks
            .iter()
            .filter_map(|t| self.entries.iter().position(|e| e.id == t.message_id))
            .collect();
        let active = rail::active_tick(&rows, self.list_state.logical_scroll_top().item_ix);
        let buckets = rail::tick_buckets(ticks.len(), rail::MAX_RAIL_TICKS);
        Some(
            div()
                .absolute()
                .left(px(8.))
                .top(px(60.))
                .flex()
                .flex_col()
                .gap(px(3.))
                .children(buckets.iter().enumerate().map(|(ix, &(start, end))| {
                    let at = active.filter(|&a| a >= start && a < end).unwrap_or(start);
                    let row = rows[at];
                    let prompt = ticks[at].prompt.clone();
                    let reply = ticks[at].reply.clone().unwrap_or_default();
                    div()
                        .id(SharedString::from(format!("rail-{ix}")))
                        .group("rail-tick")
                        .relative()
                        .w(px(20.))
                        .h(px(10.))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.list_state.scroll_to(ListOffset {
                                item_ix: row,
                                offset_in_item: px(0.),
                            });
                            cx.notify();
                        }))
                        .child(
                            div()
                                .w(px(14.))
                                .h(px(2.))
                                .mt(px(4.))
                                .bg(if active == Some(at) {
                                    theme::text_muted()
                                } else {
                                    theme::text_faint().opacity(0.4)
                                }),
                        )
                        .child(
                            div()
                                .absolute()
                                .left(px(22.))
                                .top(px(-8.))
                                .w(px(280.))
                                .p(px(10.))
                                .rounded(px(8.))
                                .bg(theme::surface_card())
                                .border_1()
                                .border_color(theme::border())
                                .invisible()
                                .group_hover("rail-tick", |s| s.visible())
                                .text_size(px(12.))
                                .child(prompt)
                                .child(div().text_color(theme::text_faint()).child(reply)),
                        )
                }))
                .into_any_element(),
        )
    }

    /// Workspace links use the same parser as the desktop; the host file API
    /// decides whether the resolved file is readable.
    pub(super) fn install_links(&self, cx: &mut Context<Self>) {
        let Some(chat) = self.selected_chat() else {
            markdown::set_links(None);
            return;
        };
        let root = chat.cwd.unwrap_or_else(|| "/".into());
        let entity = cx.entity().downgrade();
        markdown::set_links(Some(LinkUi {
            source_session: Some(chat.id),
            source_local: false,
            file_roots: None,
            handler: Rc::new(move |activation, _, cx| {
                if let Some(id) = activation.target.original.strip_prefix("openagents-chat:") {
                    let id = id.to_owned();
                    let _ = entity.update(cx, |this, cx| {
                        if this.meta.contains_key(&id) {
                            super::route::push(Some(&id));
                            this.load(Some(id), cx);
                        } else {
                            this.notice = Some("The original chat was deleted".into());
                            cx.notify();
                        }
                    });
                    return LinkOutcome::Internal;
                }
                use openagents_chat_view::workspace_links::{
                    FileLinkResolution, first_root_owning,
                };
                if let Some(resolved) =
                    first_root_owning(&activation.target.original, [root.as_str()])
                {
                    let link = match resolved {
                        FileLinkResolution::Owned { link, .. }
                        | FileLinkResolution::Outside(link) => link,
                    };
                    crate::compose::page_event("oa-file-open", &link.path);
                    LinkOutcome::Internal
                } else {
                    activation.web_outcome(false)
                }
            }),
        }));
    }
}

impl ChatApp {
    pub(super) fn toggle_tool_group(&mut self, key: String, cx: &mut Context<Self>) {
        if self.expanded.remove(&key) {
            // Release decoded images on collapse, like the desktop's tool tray.
            self.extras.images.clear();
            self.extras.outputs.clear();
            self.extras.errors.remove(&key);
        } else {
            self.expanded.insert(key.clone());
            self.load_tool_group(key, cx);
        }
        self.list_state.remeasure();
        cx.notify();
    }

    fn load_tool_group(&mut self, key: String, cx: &mut Context<Self>) {
        let Some(host) = self.compose.read(cx).host().cloned() else {
            return;
        };
        let Some((entry_key, index)) = key.rsplit_once('/') else {
            return;
        };
        let Some(entry_id) = entry_key.strip_prefix(&format!("{}/", host.chat)) else {
            return;
        };
        let Some(entry) = self.entries.iter().find(|e| e.id == entry_id) else {
            return;
        };
        let Ok(index) = index.parse::<usize>() else {
            return;
        };
        let parts: Vec<_> = entry
            .parts
            .iter()
            .skip(index)
            .take_while(|p| matches!(p, MessagePart::Tool { .. } | MessagePart::Reasoning { .. }))
            .cloned()
            .collect();
        if !self.extras.loading.insert(key.clone()) {
            return;
        }
        let cwd = self.selected_chat().and_then(|c| c.cwd);
        let client = self.client.clone();
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let result = async {
                let computer = computer::connect(&client, &host.device, || {}).await?;
                let mut outputs = HashMap::new();
                let mut paths = Vec::new();
                for part in &parts {
                    if let MessagePart::Tool {
                        call,
                        output,
                        output_ref,
                        ..
                    } = part
                    {
                        let output = if let Some(reference) = output_ref {
                            let value = computer
                                .call("FetchToolBlob", json!({"blobRef": reference}))
                                .await?;
                            let text = value["text"].as_str().unwrap_or_default();
                            // The sidecar can be large; keep only bounded display text.
                            let text: String = text.chars().take(64 * 1024).collect();
                            outputs.insert(reference.clone(), text.clone());
                            Some(text)
                        } else {
                            output.clone()
                        };
                        for path in tool_images::image_paths(call, output.as_deref()).iter() {
                            let path = tool_images::resolve_path(path, cwd.as_deref());
                            if paths.len() < 16 && !paths.contains(&path) {
                                paths.push(path);
                            }
                        }
                    }
                }
                let mut images = HashMap::new();
                if !paths.is_empty() {
                    let page = computer
                        .call(
                            "ListWorkspaceDirectory",
                            json!({"chatId": host.chat, "directory": "", "includeIgnored": false}),
                        )
                        .await?;
                    let checkout = page["checkoutId"].as_str().ok_or_else(|| {
                        anyhow::anyhow!("The chat's folder changed. Open the tool again.")
                    })?;
                    let mut budget = 16 * 1024 * 1024;
                    for path in paths {
                        if let Ok(image) =
                            read_image(&computer, &host.chat, &path, checkout, &mut budget).await
                        {
                            images.insert(path, image);
                        }
                    }
                }
                Ok::<_, anyhow::Error>((outputs, images))
            }
            .await;
            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.extras.loading.remove(&key);
                if !this.expanded.contains(&key) {
                    return;
                }
                match result {
                    Ok((outputs, images)) => {
                        // One viewport's previews, bounded even across many groups.
                        if this.extras.images.len() + images.len() > 16 {
                            this.extras.images.clear();
                        }
                        if this.extras.outputs.len() + outputs.len() > 32 {
                            this.extras.outputs.clear();
                        }
                        this.extras.outputs.extend(outputs);
                        this.extras.images.extend(images);
                        this.extras.errors.remove(&key);
                    }
                    Err(error) => {
                        this.extras.errors.insert(key, error.to_string());
                    }
                }
                this.list_state.remeasure();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_tool_part(&self, part: &MessagePart) -> AnyElement {
        let mut hydrated = part.clone();
        if let MessagePart::Tool {
            output,
            output_ref: Some(reference),
            ..
        } = &mut hydrated
        {
            if let Some(text) = self.extras.outputs.get(reference) {
                *output = Some(text.clone());
            }
        }
        let mut row = div()
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(transcript::tool_row(&hydrated));
        if let MessagePart::Tool {
            call,
            output,
            output_ref,
            ..
        } = &hydrated
        {
            let cwd = self.selected_chat().and_then(|c| c.cwd);
            for path in tool_images::image_paths(call, output.as_deref()).iter() {
                let resolved = tool_images::resolve_path(path, cwd.as_deref());
                if let Some(image) = self.extras.images.get(&resolved) {
                    row = row.child(
                        openagents_gpui_web::img(image.clone())
                            .max_w(px(560.))
                            .max_h(px(220.))
                            .rounded(px(8.)),
                    );
                }
            }
            if output_ref.is_some() {
                if let Some(output) = output {
                    row = row.child(
                        div()
                            .id(SharedString::from(format!("tool-output-{}", part.id())))
                            .max_h(px(240.))
                            .overflow_y_scroll()
                            .text_size(px(12.))
                            .font_family(theme::MONO)
                            .text_color(theme::text_faint())
                            .child(output.clone()),
                    );
                }
            }
        }
        row.into_any_element()
    }

    pub(super) fn render_tool_errors(&self) -> AnyElement {
        div()
            .children(
                self.extras.errors.values().map(|line| {
                    footer_text(format!("{line} Close and reopen the tool to try again."))
                }),
            )
            .into_any_element()
    }
}

async fn read_image(
    computer: &computer::Computer,
    chat: &str,
    path: &str,
    checkout: &str,
    budget: &mut usize,
) -> anyhow::Result<Arc<Image>> {
    let mut bytes = Vec::new();
    let mut offset = 0;
    let mut hash = None::<String>;
    let mut mime;
    loop {
        let chunk: WorkspaceImageChunk = serde_json::from_value(computer.call("ReadWorkspaceImage", json!({"chatId": chat, "path": path, "expectedCheckoutId": checkout, "offset": offset, "expectedContentHash": hash})).await?)?;
        anyhow::ensure!(
            chunk.size <= 8 * 1024 * 1024 && chunk.size <= *budget,
            "Image is too large"
        );
        anyhow::ensure!(
            hash.as_ref().is_none_or(|h| h == &chunk.content_hash),
            "Image changed while loading"
        );
        let data = base64::engine::general_purpose::STANDARD.decode(&chunk.data)?;
        anyhow::ensure!(bytes.len() + data.len() <= chunk.size, "Image size changed");
        bytes.extend(data);
        hash.get_or_insert(chunk.content_hash);
        mime = chunk.mime_type;
        if chunk.done {
            break;
        }
        anyhow::ensure!(chunk.next_offset > offset, "Image read stopped");
        offset = chunk.next_offset;
    }
    *budget = budget.saturating_sub(bytes.len());
    let format = match mime.as_str() {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        "image/bmp" => ImageFormat::Bmp,
        "image/svg+xml" => ImageFormat::Svg,
        _ => anyhow::bail!("Image format isn't supported"),
    };
    Ok(Arc::new(Image::from_bytes(format, bytes)))
}
