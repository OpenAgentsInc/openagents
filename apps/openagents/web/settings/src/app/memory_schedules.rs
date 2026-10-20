//! Memory and schedules over the account API, loaded with Settings rather than chat.
use super::*;
use forms::{memory, schedules};
use openagents_web_chat_view::composer::{Edited, TextInput};
use std::collections::HashMap;

pub(super) type Fields = HashMap<String, gpui::Entity<TextInput>>;

impl SettingsApp {
    pub(super) fn load_memory_schedules(&mut self, page: Page, cx: &mut Context<Self>) {
        if !self.context.signed_in {
            let message = "Sign in to change your memory and schedules.".to_owned();
            if page == Page::Memory {
                self.remote.memory.notes = Load::Failed(message);
            } else {
                self.remote.schedules.listed = Load::Failed(message);
            }
            return;
        }
        let client = self.client.clone();
        if page == Page::Memory {
            self.remote.memory.notes = Load::Loading;
        } else {
            self.remote.schedules.listed = Load::Loading;
        }
        let task = cx.spawn(async move |this, cx| {
            let result: Result<Value, _> = client
                .get(if page == Page::Memory {
                    "/v1/memory"
                } else {
                    "/v1/schedules"
                })
                .await;
            this.update(cx, |this, cx| {
                match page {
                    Page::Memory => {
                        this.remote.memory.notes = match result {
                            Ok(v) => match serde_json::from_value::<Vec<memory::Note>>(
                                v["notes"].clone(),
                            ) {
                                Ok(notes) => Load::Ready(notes),
                                Err(_) => {
                                    Load::Failed("We couldn't read your memory. Try again.".into())
                                }
                            },
                            Err(e) => Load::Failed(plain(&e)),
                        }
                    }
                    _ => {
                        this.remote.schedules.listed = match result {
                            Ok(v) => match serde_json::from_value(v) {
                                Ok(listed) => Load::Ready(listed),
                                Err(_) => Load::Failed(
                                    "We couldn't read your schedules. Try again.".into(),
                                ),
                            },
                            Err(e) => Load::Failed(plain(&e)),
                        }
                    }
                }
                this.revision += 1;
                cx.notify();
            })
            .ok();
        });
        if page == Page::Memory {
            self.memory_read = Some(task);
        } else {
            self.schedules_read = Some(task);
        }
    }

    pub(super) fn memory_action(&mut self, action: memory::Action, cx: &mut Context<Self>) {
        use memory::Action::*;
        if self.remote.memory.busy {
            return;
        }
        match action {
            Reload => self.load_memory_schedules(forms::Page::Memory, cx),
            Add => self.remote.memory.edit(None),
            Edit(id) => self.remote.memory.edit(Some(&id)),
            Cancel => self.remote.memory.draft = None,
            Kind(kind) => {
                if let Some(d) = &mut self.remote.memory.draft {
                    d.kind = kind;
                }
            }
            Page(page) => self.remote.memory.page = page,
            Delete(id) => self.write_memory_schedule(
                forms::Page::Memory,
                "DELETE",
                format!("/v1/memory/{}", encode(&id)),
                None,
                cx,
            ),
            Save => {
                if let Some(draft) = &self.remote.memory.draft {
                    self.write_memory_schedule(
                        forms::Page::Memory,
                        "PUT",
                        format!(
                            "/v1/memory/{}",
                            encode(draft.id.as_deref().unwrap_or("new"))
                        ),
                        Some(serde_json::to_value(draft).unwrap()),
                        cx,
                    );
                }
            }
            Field(_) => {}
        }
    }

    pub(super) fn schedules_action(&mut self, action: schedules::Action, cx: &mut Context<Self>) {
        use schedules::Action::*;
        if self.remote.schedules.busy {
            return;
        }
        match action {
            Reload => self.load_memory_schedules(forms::Page::Schedules, cx),
            Add => self.remote.schedules.edit(None),
            Edit(id) => self.remote.schedules.edit(Some(&id)),
            Cancel => self.remote.schedules.draft = None,
            Page(page) => self.remote.schedules.page = page,
            Delete(id) => self.write_memory_schedule(
                forms::Page::Schedules,
                "DELETE",
                format!("/v1/schedules/{}", encode(&id)),
                None,
                cx,
            ),
            Pause(id, paused) => self.write_memory_schedule(
                forms::Page::Schedules,
                "POST",
                format!("/v1/schedules/{}/pause", encode(&id)),
                Some(json!({"paused": paused})),
                cx,
            ),
            Save => {
                if let Some(draft) = &self.remote.schedules.draft {
                    match draft.body() {
                        Ok(body) => self.write_memory_schedule(
                            forms::Page::Schedules,
                            "PUT",
                            format!(
                                "/v1/schedules/{}",
                                encode(if draft.schedule.id.is_empty() {
                                    "new"
                                } else {
                                    &draft.schedule.id
                                })
                            ),
                            Some(body),
                            cx,
                        ),
                        Err(message) => self.notice = Some(message.into()),
                    }
                }
            }
            change => {
                if let Some(draft) = &mut self.remote.schedules.draft {
                    match change {
                        Computer(computer) => {
                            draft.schedule.computer = computer;
                            draft.schedule.chat = None;
                        }
                        Chat(chat) => draft.schedule.chat = chat,
                        Interval(interval) => draft.interval = interval,
                        Day(day) if day <= 6 => {
                            if draft.schedule.days.contains(&day) {
                                draft.schedule.days.retain(|d| *d != day);
                            } else {
                                draft.schedule.days.push(day);
                                draft.schedule.days.sort();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn write_memory_schedule(
        &mut self,
        page: Page,
        method: &'static str,
        path: String,
        body: Option<Value>,
        cx: &mut Context<Self>,
    ) {
        if !self.context.signed_in {
            return;
        }
        let client = self.client.clone();
        if page == Page::Memory {
            self.remote.memory.busy = true;
        } else {
            self.remote.schedules.busy = true;
        }
        self.notice = None;
        let task = cx.spawn(async move |this, cx| {
            let result = client.send::<Value>(method, &path, body.as_ref()).await;
            this.update(cx, |this, cx| {
                if page == Page::Memory {
                    this.remote.memory.busy = false;
                } else {
                    this.remote.schedules.busy = false;
                }
                match result {
                    Ok(_) => {
                        let id = path.rsplit('/').next().unwrap_or_default();
                        if page == Page::Schedules
                            && method == "POST"
                            && let Some(draft) = &mut this.remote.schedules.draft
                            && path == format!("/v1/schedules/{}/pause", encode(&draft.schedule.id))
                            && let Some(paused) = body.as_ref().and_then(|v| v["paused"].as_bool())
                        {
                            draft.schedule.paused = paused;
                        }
                        // Keep edits when pausing or deleting another record.
                        if method == "PUT" {
                            if page == Page::Memory {
                                this.remote.memory.draft = None;
                            } else {
                                this.remote.schedules.draft = None;
                            }
                        }
                        if method == "DELETE" {
                            if page == Page::Memory
                                && this.remote.memory.draft.as_ref().is_some_and(|d| {
                                    encode(d.id.as_deref().unwrap_or_default()) == id
                                })
                            {
                                this.remote.memory.draft = None;
                            } else if page == Page::Schedules
                                && this
                                    .remote
                                    .schedules
                                    .draft
                                    .as_ref()
                                    .is_some_and(|d| encode(&d.schedule.id) == id)
                            {
                                this.remote.schedules.draft = None;
                            }
                        }
                        this.load_memory_schedules(page, cx);
                    }
                    Err(e) => this.notice = Some(plain(&e)),
                }
                this.revision += 1;
                cx.notify();
            })
            .ok();
        });
        if page == Page::Memory {
            self.memory_write = Some(task);
        } else {
            self.schedules_write = Some(task);
        }
    }

    /// Keep field entities across renders so selection, focus, and IME survive.
    /// Field labels, limits, values, and layout all come from the shared form.
    pub(super) fn prepare_account_fields(
        &mut self,
        root: &rust_native::Node<Intent>,
        cx: &mut Context<Self>,
    ) {
        use rust_native::Element;
        match &root.element {
            Element::Field {
                value,
                placeholder,
                multiline,
                max_bytes,
                enabled,
                ..
            } if *enabled => {
                let key = root.key.clone();
                if !self.account_fields.contains_key(&key) {
                    let input = cx.new(|cx| {
                        if *multiline {
                            {
                                let mut field = TextInput::multiline(placeholder.clone(), cx);
                                field.set_send_with_modifier(true);
                                field
                            }
                        } else {
                            TextInput::new(placeholder.clone(), cx)
                        }
                    });
                    cx.subscribe(
                        &input,
                        |_,
                         input,
                         submitted: &openagents_web_chat_view::composer::Submitted,
                         cx| {
                            input.update(cx, |input, cx| input.set_text(submitted.0.clone(), cx));
                        },
                    )
                    .detach();
                    let field_key = key.clone();
                    let limit = *max_bytes;
                    cx.subscribe(&input, move |this, input, _: &Edited, cx| {
                        let text = input.read(cx).text();
                        if text.len() > limit {
                            let mut end = limit;
                            while !text.is_char_boundary(end) {
                                end -= 1;
                            }
                            input
                                .update(cx, |input, cx| input.set_text(text[..end].to_owned(), cx));
                        }
                        let text = input.read(cx).text();
                        if field_key.starts_with("memory-") {
                            this.remote.memory.field(&field_key, text);
                        } else {
                            this.remote.schedules.field(&field_key, text);
                        }
                        this.revision += 1;
                        cx.notify();
                    })
                    .detach();
                    self.account_fields.insert(key.clone(), input);
                }
                if let Some(input) = self.account_fields.get(&key) {
                    if input.read(cx).text() != *value {
                        input.update(cx, |input, cx| input.set_text(value.clone(), cx));
                    }
                }
            }
            Element::Stack { children, .. }
            | Element::List { children, .. }
            | Element::Choice { children, .. }
            | Element::Dialog { children, .. } => {
                for child in children {
                    self.prepare_account_fields(child, cx);
                }
            }
            _ => {}
        }
    }
}

fn encode(value: &str) -> String {
    js_sys::encode_uri_component(value)
        .as_string()
        .unwrap_or_default()
}
