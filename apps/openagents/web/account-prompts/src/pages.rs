use gpui::{Context, Task};
use openagents_chat_view::settings_forms::{memory, schedules};
use openagents_web_page_host::{Host, Intent, Load, Page, Paints, plain};
use rust_native::Node;
use serde_json::{Value, json};

pub struct Memory {
    state: memory::State,
    read: Option<Task<()>>,
    write: Option<Task<()>>,
}
pub struct Schedules {
    state: schedules::State,
    read: Option<Task<()>>,
    write: Option<Task<()>>,
}

type Cx<'a, P> = Context<'a, Host<P>>;
fn encode(id: &str) -> String {
    String::from(js_sys::encode_uri_component(id))
}

impl Page for Memory {
    const NAME: &'static str = "memory";
    fn new() -> Self {
        Self {
            state: memory::State::default(),
            read: None,
            write: None,
        }
    }
    fn title(&self) -> String {
        "Settings · Memory".into()
    }
    fn view(&self, paints: &Paints) -> Node<Intent> {
        memory::view(&self.state, paints)
    }
    fn open(host: &mut Host<Self>, _: &str, cx: &mut Cx<Self>) {
        load_memory(host, cx);
    }
    fn field(host: &mut Host<Self>, key: &str, value: String, _: &mut Cx<Self>) {
        if !host.page.state.busy {
            host.page.state.field(key, value);
        }
    }
    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx<Self>) {
        let Intent::Memory(action) = intent else {
            return;
        };
        if host.page.state.busy {
            return;
        }
        use memory::Action::*;
        match action {
            Reload => load_memory(host, cx),
            Add => host.page.state.edit(None),
            Edit(id) => host.page.state.edit(Some(&id)),
            Cancel => host.page.state.draft = None,
            Kind(kind) => {
                if let Some(d) = &mut host.page.state.draft {
                    d.kind = kind;
                }
            }
            Page(page) => host.page.state.page = page,
            Delete(id) => write_memory(host, "DELETE", id, None, cx),
            Save => {
                if let Some(draft) = &host.page.state.draft {
                    let id = draft.id.clone().unwrap_or_else(|| "new".into());
                    let body = serde_json::to_value(draft).unwrap();
                    write_memory(host, "PUT", id, Some(body), cx);
                }
            }
            Field(_) => {}
        }
    }
}

fn load_memory(host: &mut Host<Memory>, cx: &mut Cx<Memory>) {
    host.page.state.notes = Load::Loading;
    let client = host.client.clone();
    host.page.read = Some(cx.spawn(async move |this, cx| {
        let result: Result<Value, _> = client.get("/v1/memory").await;
        this.update(cx, |host, cx| {
            host.page.state.notes = match result {
                Ok(v) => match serde_json::from_value(v["notes"].clone()) {
                    Ok(notes) => Load::Ready(notes),
                    Err(_) => Load::Failed("We couldn't read your memory. Try again.".into()),
                },
                Err(e) => Load::Failed(plain(&e)),
            };
            host.changed(cx);
        })
        .ok();
    }));
}

fn write_memory(
    host: &mut Host<Memory>,
    method: &'static str,
    id: String,
    body: Option<Value>,
    cx: &mut Cx<Memory>,
) {
    host.page.state.busy = true;
    let client = host.client.clone();
    host.page.write = Some(cx.spawn(async move |this, cx| {
        let result = client
            .send::<Value>(
                method,
                &format!("/v1/memory/{}", encode(&id)),
                body.as_ref(),
            )
            .await;
        this.update(cx, |host, cx| {
            host.page.state.busy = false;
            match result {
                Ok(_) => {
                    if method == "PUT"
                        || host
                            .page
                            .state
                            .draft
                            .as_ref()
                            .is_some_and(|d| d.id.as_deref() == Some(&id))
                    {
                        host.page.state.draft = None;
                    }
                    load_memory(host, cx);
                }
                Err(e) => host.notice = Some(plain(&e)),
            }
            host.changed(cx);
        })
        .ok();
    }));
}

impl Page for Schedules {
    const NAME: &'static str = "schedules";
    fn new() -> Self {
        Self {
            state: schedules::State::default(),
            read: None,
            write: None,
        }
    }
    fn title(&self) -> String {
        "Settings · Schedules".into()
    }
    fn view(&self, paints: &Paints) -> Node<Intent> {
        schedules::view(&self.state, paints)
    }
    fn open(host: &mut Host<Self>, _: &str, cx: &mut Cx<Self>) {
        load_schedules(host, cx);
    }
    fn field(host: &mut Host<Self>, key: &str, value: String, _: &mut Cx<Self>) {
        if !host.page.state.busy {
            host.page.state.field(key, value);
        }
    }
    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx<Self>) {
        let Intent::Schedules(action) = intent else {
            return;
        };
        if host.page.state.busy {
            return;
        }
        use schedules::Action::*;
        match action {
            Reload => load_schedules(host, cx),
            Add => host.page.state.edit(None),
            Edit(id) => host.page.state.edit(Some(&id)),
            Cancel => host.page.state.draft = None,
            Page(page) => host.page.state.page = page,
            Delete(id) => write_schedule(host, "DELETE", id, None, false, cx),
            Pause(id, paused) => {
                write_schedule(host, "POST", id, Some(json!({"paused":paused})), true, cx)
            }
            Save => {
                if let Some(draft) = &host.page.state.draft {
                    match draft.body() {
                        Ok(body) => {
                            let id = if draft.schedule.id.is_empty() {
                                "new".into()
                            } else {
                                draft.schedule.id.clone()
                            };
                            write_schedule(host, "PUT", id, Some(body), false, cx);
                        }
                        Err(message) => host.notice = Some(message.into()),
                    }
                }
            }
            change => {
                if let Some(draft) = &mut host.page.state.draft {
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
}

fn load_schedules(host: &mut Host<Schedules>, cx: &mut Cx<Schedules>) {
    host.page.state.listed = Load::Loading;
    let client = host.client.clone();
    host.page.read = Some(cx.spawn(async move |this, cx| {
        let result: Result<Value, _> = client.get("/v1/schedules").await;
        this.update(cx, |host, cx| {
            host.page.state.listed = match result {
                Ok(v) => match serde_json::from_value(v) {
                    Ok(listed) => Load::Ready(listed),
                    Err(_) => Load::Failed("We couldn't read your schedules. Try again.".into()),
                },
                Err(e) => Load::Failed(plain(&e)),
            };
            host.changed(cx);
        })
        .ok();
    }));
}

fn write_schedule(
    host: &mut Host<Schedules>,
    method: &'static str,
    id: String,
    body: Option<Value>,
    pause: bool,
    cx: &mut Cx<Schedules>,
) {
    host.page.state.busy = true;
    let client = host.client.clone();
    host.page.write = Some(cx.spawn(async move |this, cx| {
        let path = format!(
            "/v1/schedules/{}{}",
            encode(&id),
            if pause { "/pause" } else { "" }
        );
        let result = client.send::<Value>(method, &path, body.as_ref()).await;
        this.update(cx, |host, cx| {
            host.page.state.busy = false;
            match result {
                Ok(_) => {
                    if pause
                        && let Some(draft) = &mut host.page.state.draft
                        && draft.schedule.id == id
                        && let Some(paused) = body.as_ref().and_then(|v| v["paused"].as_bool())
                    {
                        draft.schedule.paused = paused;
                    }
                    if method == "PUT"
                        || (method == "DELETE"
                            && host
                                .page
                                .state
                                .draft
                                .as_ref()
                                .is_some_and(|d| d.schedule.id == id))
                    {
                        host.page.state.draft = None;
                    }
                    load_schedules(host, cx);
                }
                Err(e) => host.notice = Some(plain(&e)),
            }
            host.changed(cx);
        })
        .ok();
    }));
}
