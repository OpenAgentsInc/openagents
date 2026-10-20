//! Transcript chrome for a chat on a paired computer (#12270).
use std::rc::Rc;

use futures::StreamExt as _;
use gpui::{
    AnyElement, App, ClickEvent, Context, IntoElement, ParentElement, Styled, Task, div,
    prelude::*, px,
};
use openagents_chat_view::markdown::render::{LinkOutcome, LinkUi};
use openagents_chat_view::todo_panel;
use openagents_proto::{Chat, ContextUsage, TodoStatus};
use openagents_web_chat_view::labels::{self as transcript, button};
use openagents_web_chat_view::theme;
use openagents_web_client::computer;
use serde_json::json;

use super::ChatApp;

#[derive(Default)]
pub(super) struct Extras {
    chat: Option<Chat>,
    pub progress_second: i64,
    pub context: Option<ContextUsage>,
    pub connected: bool,
    todo_closed: Option<bool>,
    todo_all: bool,
    _rpc: Option<Task<()>>,
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

    /// The registry's row, or the page's host hint while it arrives.
    pub(super) fn chat_metadata(&self, cx: &App) -> Option<Chat> {
        self.selected_chat().or_else(|| {
            let host = self.compose.read(cx).host()?;
            let config = host
                .harness
                .as_ref()
                .and_then(|h| serde_json::from_value::<openagents_proto::HarnessId>(json!(h)).ok())
                .map(|harness| {
                    json!({"harness": harness, "model": null, "reasoning": null,
                    "modelOptions": {}, "sandbox": "danger-full-access"})
                });
            serde_json::from_value(json!({"id": host.chat, "deviceId": host.device,
                "archived": false, "createdAt": js_sys::Date::new_0().to_iso_string().as_string(), "config": config}))
            .ok()
        })
    }

    pub(super) fn start_extras(&mut self, chat: &Chat, cx: &mut Context<Self>) {
        self.extras.chat = Some(chat.clone());
        self.start_transcript_rpc(chat, cx);
        let generation = self.generation;
        if self.extra_events.is_none() {
            let mut states = openagents_web_client::store::listen("oa-extras-state");
            self.extra_events = Some(cx.spawn(async move |this, cx| {
                while let Some(detail) = states.next().await {
                    let Ok(value) = serde_json::from_str::<serde_json::Value>(&detail) else {
                        continue;
                    };
                    if this
                        .update(cx, |this, cx| {
                            if value["generation"].as_u64() != Some(this.generation)
                                || value["chat"].as_str() != this.current.as_deref()
                            {
                                return;
                            }
                            if value["action"] == "reconnect" {
                                this.load(this.current.clone(), cx);
                                return;
                            }
                            if let Some(notice) = value["notice"].as_str() {
                                this.notice = Some(notice.to_owned());
                            } else if this
                                .notice
                                .as_deref()
                                .is_some_and(|n| n.starts_with("Allow this browser"))
                            {
                                this.notice = None;
                            }
                            if let Ok(chat) = serde_json::from_value::<Chat>(value["fork"].clone())
                            {
                                let id = chat.id.clone();
                                let computer = this
                                    .current
                                    .as_ref()
                                    .and_then(|id| this.meta.get(id))
                                    .map(|m| m.computer.1.clone())
                                    .unwrap_or_default();
                                this.meta.insert(
                                    id.clone(),
                                    crate::model::Meta {
                                        computer: (chat.device_id.clone(), computer),
                                        ..Default::default()
                                    },
                                );
                                this.pending_fork = Some(chat);
                                super::route::push(Some(&id));
                                this.load(Some(id), cx);
                            }
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        crate::compose::page_event(
            "oa-extras-context",
            &json!({
                "device": chat.device_id, "chat": chat.id, "generation": generation,
                "harness": chat.config.as_ref().map(|c| c.harness), "loading": self.loading_chat,
                "context_usage": self.extras.context, "connected": self.extras.connected,
            })
            .to_string(),
        );
        if let Some(name) =
            openagents_proto::saved_agent::agent_name(chat.config.as_ref(), chat.title.as_deref())
        {
            self.composer.update(cx, |input, cx| {
                input.set_placeholder(format!("Message {name}"), cx)
            });
        }
    }

    pub(super) fn sync_extras(&self) {
        crate::compose::page_event(
            "oa-extras-usage",
            &json!({
                "chat": self.current, "generation": self.generation,
                "context_usage": self.extras.context, "connected": self.extras.connected,
                "loading": self.loading_chat,
            })
            .to_string(),
        );
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
            .max_w(px(transcript::content_width()))
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
        // The separately mounted footer keeps this space in the composer layout.
        Some(
            gpui::canvas(
                |bounds, _, _| {
                    crate::compose::page_event(
                        "oa-extras-bounds",
                        &json!({
                            "x": f32::from(bounds.origin.x), "bottom": f32::from(bounds.bottom()),
                            "width": f32::from(bounds.size.width),
                        })
                        .to_string(),
                    );
                },
                |_, _, _, _| {},
            )
            .w_full()
            .h(px(76.))
            .into_any_element(),
        )
    }

    /// Workspace links use the same parser as the desktop; the host file API
    /// decides whether the resolved file is readable.
    pub(super) fn install_links(&self, cx: &mut Context<Self>) {
        let Some(chat) = self.selected_chat() else {
            self.transcript
                .update(cx, |view, _| view.workspace_link = None);
            return;
        };
        // An unknown folder still accepts relative links; absolute links
        // keep their full host path until the registry supplies the folder.
        let root = chat.cwd.unwrap_or_else(|| ".".into());
        let device = chat.device_id;
        let source_chat = chat.id.clone();
        let entity = cx.entity().downgrade();
        let links = LinkUi {
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
                    crate::compose::page_event("oa-files-open", &json!({
                        "device": device, "chat": source_chat, "path": link.path, "line": link.line,
                    }).to_string());
                    LinkOutcome::Internal
                } else {
                    activation.web_outcome(false)
                }
            }),
        };
        self.transcript
            .update(cx, |view, _| view.set_workspace_link_handler(links));
    }
}

struct TranscriptCall(futures::channel::mpsc::UnboundedSender<TranscriptRequest>);
struct TranscriptRequest {
    method: String,
    params: serde_json::Value,
    reply: futures::channel::oneshot::Sender<Result<serde_json::Value, String>>,
}
impl openagents_chat_view::rpc::Call for TranscriptCall {
    fn call(&self, method: &str, params: serde_json::Value) -> openagents_chat_view::rpc::Reply {
        let (reply, receiver) = futures::channel::oneshot::channel();
        let sent = self.0.unbounded_send(TranscriptRequest {
            method: method.into(),
            params,
            reply,
        });
        Box::pin(async move {
            sent.map_err(|_| "The chat was closed".to_owned())?;
            receiver
                .await
                .map_err(|_| "The chat was closed".to_owned())?
        })
    }
}

impl ChatApp {
    fn start_transcript_rpc(&mut self, chat: &Chat, cx: &mut Context<Self>) {
        let (sender, mut requests) = futures::channel::mpsc::unbounded::<TranscriptRequest>();
        let rpc = openagents_chat_view::rpc::Rpc::new(TranscriptCall(sender));
        let place = openagents_chat_view::transcript::host::ChatPlace {
            device_id: chat.device_id.clone(),
            cwd: chat.cwd.clone(),
        };
        self.shared.update(cx, |state, cx| {
            state.set_engine(Some(rpc), Some(place));
            cx.notify();
        });
        let client = self.client.clone();
        let device = chat.device_id.clone();
        self.extras._rpc = Some(cx.spawn(async move |_, _| {
            while let Some(request) = requests.next().await {
                if request.reply.is_canceled() {
                    continue;
                }
                let result = async {
                    if request.method == "ReadTranscriptImage" {
                        return read_transcript_image(&device, &request.params).await;
                    }
                    let host = computer::connect(&client, &device, || {}).await?;
                    host.call(&request.method, request.params).await
                }
                .await
                .map_err(|error| error.to_string());
                let _ = request.reply.send(result);
            }
        }));
    }
}

async fn read_transcript_image(
    device: &str,
    params: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    unsafe extern "C" {
        #[wasm_bindgen(catch, js_namespace = window, js_name = __oaReadTranscriptImage)]
        fn read_image(device: &str, chat: &str, path: &str) -> Result<js_sys::Promise, JsValue>;
    }
    let read = read_image(
        device,
        params["chatId"].as_str().unwrap_or_default(),
        params["path"].as_str().unwrap_or_default(),
    )
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let value = wasm_bindgen_futures::JsFuture::from(read)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(serde_json::from_str(
        &value.as_string().unwrap_or_default(),
    )?)
}
