//! The work panel: three tabs (Work, Environments, Mac jobs), each a list
//! that refreshes while it shows, and a run or job opened from it that
//! follows itself live: a work run and a Mac job by long poll on their
//! `after` cursor, an environment and its Claude Code run every two seconds.

use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, Context, Entity, FontWeight, IntoElement, ParentElement, Render,
    SharedString, Styled, Task, Window, div, prelude::*, px,
};
use openagents_web_chat_view::composer::{Submitted, TextInput};
use openagents_web_chat_view::labels::{button, footer_text};
use openagents_web_chat_view::theme;
use openagents_web_client::web::Client;
use serde_json::{Value, json};

use crate::model::{
    self, Environment, EnvironmentState, Item, MacJob, RunState, WorkRun, state_words,
};

const REFRESH: Duration = Duration::from_secs(5);
const FOLLOW: Duration = Duration::from_secs(2);

/// Tells the page something (`oa-work-close`, `oa-work-failed`).
pub fn signal(name: &str) {
    if let Some(window) = web_sys::window()
        && let Ok(event) = web_sys::Event::new(name)
    {
        let _ = window.dispatch_event(&event);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Work,
    Environments,
    Mac,
}

enum Open {
    Nothing,
    Work(WorkRun),
    Environment(String, Option<EnvironmentState>),
    Run(String, String, Option<RunState>),
    Mac(MacJob),
}

pub struct WorkApp {
    client: Client,
    tab: Tab,
    notice: Option<String>,
    runs: Option<Vec<WorkRun>>,
    land: Option<String>,
    environments: Option<Vec<Environment>>,
    environments_here: bool,
    jobs: Option<Vec<MacJob>>,
    open: Open,
    issue: Entity<TextInput>,
    message: Entity<TextInput>,
    _list: Task<()>,
    detail: Option<Task<()>>,
}

impl WorkApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let issue = cx.new(|cx| TextInput::new("owner/repo#123", cx));
        cx.subscribe_in(&issue, window, |this, _, event: &Submitted, _, cx| {
            this.work_issue(event.0.clone(), cx)
        })
        .detach();
        let message = cx.new(|cx| TextInput::new("Message the setup agent", cx));
        cx.subscribe_in(&message, window, |this, _, event: &Submitted, _, cx| {
            this.message_environment(event.0.clone(), cx)
        })
        .detach();
        let list = cx.spawn(async move |this, cx| {
            loop {
                let Ok((client, tab)) =
                    this.read_with(cx, |this, _| (this.client.clone(), this.tab))
                else {
                    break;
                };
                match tab {
                    Tab::Work => {
                        let runs = client.get::<Value>("/v1/work").await;
                        let land = client.get::<Value>("/v1/land").await;
                        let gone = this
                            .update(cx, |this, cx| {
                                match runs {
                                    Ok(v) => {
                                        this.runs = Some(
                                            serde_json::from_value(v["runs"].clone())
                                                .unwrap_or_default(),
                                        )
                                    }
                                    Err(error) => this.refused(error),
                                }
                                this.land = land.ok().map(|v| model::land_line(&v));
                                cx.notify();
                            })
                            .is_err();
                        if gone {
                            break;
                        }
                    }
                    Tab::Environments => {
                        let found = client.get::<Value>("/environments/list.json").await;
                        let gone = this
                            .update(cx, |this, cx| {
                                match found {
                                    Ok(v) => {
                                        this.environments_here =
                                            v["available"].as_bool().unwrap_or(false);
                                        this.environments = Some(
                                            serde_json::from_value(v["environments"].clone())
                                                .unwrap_or_default(),
                                        );
                                    }
                                    // Environments are for accounts allowed
                                    // agent work; the rest get the site's
                                    // not-found page.
                                    Err(_) => {
                                        this.environments = Some(Vec::new());
                                        this.environments_here = false;
                                    }
                                }
                                cx.notify();
                            })
                            .is_err();
                        if gone {
                            break;
                        }
                    }
                    Tab::Mac => {
                        let found = client.get::<Value>("/v1/mac-jobs").await;
                        let gone = this
                            .update(cx, |this, cx| {
                                match found {
                                    Ok(v) => {
                                        this.jobs = Some(
                                            serde_json::from_value(v["jobs"].clone())
                                                .unwrap_or_default(),
                                        )
                                    }
                                    Err(error) => {
                                        this.jobs = Some(Vec::new());
                                        this.refused(error)
                                    }
                                }
                                cx.notify();
                            })
                            .is_err();
                        if gone {
                            break;
                        }
                    }
                }
                cx.background_executor().timer(REFRESH).await;
            }
        });
        Self {
            client: Client::from_page(),
            tab: Tab::Work,
            notice: None,
            runs: None,
            land: None,
            environments: None,
            environments_here: true,
            jobs: None,
            open: Open::Nothing,
            issue,
            message,
            _list: list,
            detail: None,
        }
    }

    fn refused(&mut self, error: anyhow::Error) {
        self.notice = Some(error.to_string());
    }

    fn switch(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        self.close_detail();
        cx.notify();
    }

    /// Open the item an address names (#12332): `work:ID`, `env:ID`,
    /// `env:ID/run:RUN`, or `mac:ID`, on its tab.
    pub fn focus(&mut self, target: &str, cx: &mut Context<Self>) {
        if let Some(id) = target.strip_prefix("work:") {
            self.tab = Tab::Work;
            self.open_work(
                WorkRun {
                    id: id.to_owned(),
                    ..WorkRun::default()
                },
                cx,
            );
        } else if let Some(rest) = target.strip_prefix("env:") {
            self.tab = Tab::Environments;
            match rest.split_once("/run:") {
                Some((env, run)) => self.open_run(env.to_owned(), run.to_owned(), cx),
                None => self.open_environment(rest.to_owned(), cx),
            }
        } else if let Some(id) = target.strip_prefix("mac:") {
            self.tab = Tab::Mac;
            self.open_job(
                MacJob {
                    id: id.to_owned(),
                    ..MacJob::default()
                },
                cx,
            );
        } else if target == "work" {
            self.switch(Tab::Work, cx);
        } else if target == "env" {
            self.switch(Tab::Environments, cx);
        } else if target == "mac" {
            self.switch(Tab::Mac, cx);
        }
    }

    fn close_detail(&mut self) {
        self.open = Open::Nothing;
        self.detail = None;
    }

    /// A one-off write; the open detail and the list pick up its effect.
    fn post(
        &mut self,
        path: String,
        body: Option<Value>,
        form: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let sent = match form {
                Some(form) => client.post_form(&path, &form).await,
                None => client
                    .send::<Value>("POST", &path, Some(&body.unwrap_or_else(|| json!({}))))
                    .await
                    .map(drop),
            };
            if let Err(error) = sent {
                this.update(cx, |this, cx| {
                    this.refused(error);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    // ── work runs ────────────────────────────────────────────────────────

    fn work_issue(&mut self, text: String, cx: &mut Context<Self>) {
        let Some((repo, issue)) = model::issue_of(&text) else {
            self.notice = Some("Write the issue as owner/repo#123.".into());
            cx.notify();
            return;
        };
        self.issue.update(cx, |field, cx| {
            field.take(cx);
        });
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let sent = client
                .send::<Value>(
                    "POST",
                    "/v1/work",
                    Some(&json!({"repo": repo, "issue": issue, "source": "web"})),
                )
                .await
                .and_then(|v| {
                    Ok(serde_json::from_value::<WorkRun>(
                        v.get("run").cloned().unwrap_or(v),
                    )?)
                });
            this.update(cx, |this, cx| {
                match sent {
                    Ok(run) if !run.id.is_empty() => this.open_work(run, cx),
                    Ok(_) => {}
                    Err(error) => this.refused(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_work(&mut self, run: WorkRun, cx: &mut Context<Self>) {
        let (client, id) = (self.client.clone(), run.id.clone());
        self.open = Open::Work(WorkRun {
            lines: Vec::new(),
            next: 0,
            ..run
        });
        let mut after = 0;
        self.detail = Some(cx.spawn(async move |this, cx| {
            loop {
                let path = format!("/v1/work/{id}?after={after}&wait=20");
                let read = client.get::<WorkRun>(&path).await;
                let done = this
                    .update(cx, |this, cx| {
                        let Open::Work(open) = &mut this.open else {
                            return true;
                        };
                        match read {
                            Ok(newer) => {
                                after = newer.next;
                                open.merge(newer);
                            }
                            Err(error) => {
                                this.notice = Some(error.to_string());
                                return true;
                            }
                        }
                        cx.notify();
                        matches!(&this.open, Open::Work(run) if run.finished())
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        }));
        cx.notify();
    }

    // ── environments ─────────────────────────────────────────────────────

    fn open_environment(&mut self, id: String, cx: &mut Context<Self>) {
        self.open = Open::Environment(id.clone(), None);
        let client = self.client.clone();
        self.detail = Some(cx.spawn(async move |this, cx| {
            loop {
                let read = client
                    .get::<EnvironmentState>(&format!("/environments/{id}/state.json"))
                    .await;
                let gone = this
                    .update(cx, |this, cx| {
                        let Open::Environment(_, state) = &mut this.open else {
                            return true;
                        };
                        match read {
                            Ok(read) => *state = Some(read),
                            Err(error) => this.notice = Some(error.to_string()),
                        }
                        cx.notify();
                        false
                    })
                    .unwrap_or(true);
                if gone {
                    break;
                }
                cx.background_executor().timer(FOLLOW).await;
            }
        }));
        cx.notify();
    }

    fn message_environment(&mut self, text: String, cx: &mut Context<Self>) {
        let Open::Environment(id, _) = &self.open else {
            return;
        };
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let path = format!("/environments/{id}/message");
        self.message.update(cx, |field, cx| {
            field.take(cx);
        });
        self.post(path, None, Some(model::form(&[("q", &text)])), cx);
    }

    fn open_run(&mut self, environment: String, run: String, cx: &mut Context<Self>) {
        self.open = Open::Run(environment.clone(), run.clone(), None);
        let client = self.client.clone();
        self.detail = Some(cx.spawn(async move |this, cx| {
            loop {
                let read = client
                    .get::<RunState>(&format!(
                        "/environments/{environment}/runs/{run}/state.json"
                    ))
                    .await;
                let gone = this
                    .update(cx, |this, cx| {
                        let Open::Run(_, _, state) = &mut this.open else {
                            return true;
                        };
                        match read {
                            Ok(read) => *state = Some(read),
                            Err(error) => this.notice = Some(error.to_string()),
                        }
                        cx.notify();
                        false
                    })
                    .unwrap_or(true);
                if gone {
                    break;
                }
                cx.background_executor().timer(FOLLOW).await;
            }
        }));
        cx.notify();
    }

    // ── Mac jobs ─────────────────────────────────────────────────────────

    fn open_job(&mut self, job: MacJob, cx: &mut Context<Self>) {
        let (client, id) = (self.client.clone(), job.id.clone());
        self.open = Open::Mac(MacJob {
            lines: Vec::new(),
            next: 0,
            ..job
        });
        let mut after = 0;
        self.detail = Some(cx.spawn(async move |this, cx| {
            loop {
                let path = format!("/v1/mac-jobs/{id}?after={after}&wait=20");
                let read = client.get::<MacJob>(&path).await;
                let done = this
                    .update(cx, |this, cx| {
                        let Open::Mac(open) = &mut this.open else {
                            return true;
                        };
                        match read {
                            Ok(newer) => {
                                after = newer.next;
                                open.merge(newer);
                            }
                            Err(error) => {
                                this.notice = Some(error.to_string());
                                return true;
                            }
                        }
                        cx.notify();
                        matches!(&this.open, Open::Mac(job) if job.finished())
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        }));
        cx.notify();
    }

    // ── drawing ──────────────────────────────────────────────────────────

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = |id: &'static str, label: &'static str, which: Tab, this: &Self| {
            div()
                .id(id)
                .px(px(10.))
                .h(px(30.))
                .flex()
                .items_center()
                .rounded(px(8.))
                .text_size(px(13.))
                .cursor_pointer()
                .when(this.tab == which, |d| d.bg(theme::element_active()))
                .hover(|s| s.bg(theme::element_hover()))
                .child(label)
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .px(px(10.))
            .py(px(8.))
            .border_b_1()
            .border_color(theme::border())
            .child(
                tab("tab-work", "Work", Tab::Work, self).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| this.switch(Tab::Work, cx)),
                ),
            )
            .child(
                tab("tab-environments", "Environments", Tab::Environments, self).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| this.switch(Tab::Environments, cx)),
                ),
            )
            .child(
                tab("tab-mac", "Mac jobs", Tab::Mac, self)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.switch(Tab::Mac, cx))),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("work-close")
                    .px(px(8.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .rounded(px(7.))
                    .text_size(px(13.))
                    .text_color(theme::text_muted())
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::element_hover()))
                    .on_click(|_: &ClickEvent, _, _| signal("oa-work-close"))
                    .child("Close"),
            )
    }

    fn row(
        &self,
        id: String,
        title: String,
        line: String,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(SharedString::from(id))
            .px(px(10.))
            .py(px(7.))
            .rounded(px(9.))
            .cursor_pointer()
            .hover(|s| s.bg(theme::element_hover()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| on_click(this, cx)))
            .child(div().truncate().text_size(px(14.)).child(title))
            .when(!line.is_empty(), |d| {
                d.child(
                    div()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(theme::text_faint())
                        .child(line),
                )
            })
            .into_any_element()
    }

    fn render_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut rows: Vec<AnyElement> = Vec::new();
        let empty = |text: &'static str| footer_text(text).into_any_element();
        match self.tab {
            Tab::Work => {
                rows.push(
                    div()
                        .mx(px(10.))
                        .mb(px(8.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(9.))
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::input_bg())
                        .text_size(px(13.5))
                        .child(self.issue.clone())
                        .into_any_element(),
                );
                if let Some(land) = &self.land {
                    rows.push(
                        div()
                            .px(px(12.))
                            .pb(px(6.))
                            .child(footer_text(land.clone()))
                            .into_any_element(),
                    );
                }
                match &self.runs {
                    None => rows.push(empty("Loading your work…")),
                    Some(runs) if runs.is_empty() => {
                        rows.push(empty("Work an issue: write it above and press Enter."))
                    }
                    Some(runs) => {
                        for run in runs {
                            let opened = run.clone();
                            rows.push(self.row(
                                format!("run-{}", run.id),
                                run.name(),
                                state_words(&run.state).into(),
                                move |this, cx| this.open_work(opened.clone(), cx),
                                cx,
                            ));
                        }
                    }
                }
            }
            Tab::Environments => match &self.environments {
                None => rows.push(empty("Loading your environments…")),
                Some(_) if !self.environments_here => {
                    rows.push(empty("Environments aren't set up here."))
                }
                Some(list) if list.is_empty() => rows.push(empty(
                    "Your environments show here. Make one on the Environments page.",
                )),
                Some(list) => {
                    for env in list {
                        let id = env.id.clone();
                        rows.push(self.row(
                            format!("env-{}", env.id),
                            env.repository.clone(),
                            format!("{} · {}", env.branch, env.status),
                            move |this, cx| this.open_environment(id.clone(), cx),
                            cx,
                        ));
                    }
                }
            },
            Tab::Mac => match &self.jobs {
                None => rows.push(empty("Loading your Mac jobs…")),
                Some(jobs) if jobs.is_empty() => rows.push(empty("Your Mac jobs show here.")),
                Some(jobs) => {
                    for job in jobs {
                        let opened = job.clone();
                        let line = format!(
                            "{} · {}",
                            job.computer,
                            job.last_line
                                .clone()
                                .unwrap_or_else(|| state_words(&job.state).into())
                        );
                        rows.push(self.row(
                            format!("job-{}", job.id),
                            job.title.clone(),
                            line,
                            move |this, cx| this.open_job(opened.clone(), cx),
                            cx,
                        ));
                    }
                }
            },
        }
        div()
            .id("work-list")
            .flex_1()
            .overflow_y_scroll()
            .py(px(10.))
            .px(px(4.))
            .children(rows)
            .into_any_element()
    }

    fn render_detail(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let back = div()
            .id("work-back")
            .text_size(px(13.))
            .text_color(theme::text_muted())
            .cursor_pointer()
            .hover(|s| s.text_color(theme::text()))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.close_detail();
                cx.notify();
            }))
            .child("Back");
        let (title, state, lines, actions, extra): (
            String,
            String,
            Vec<Item>,
            Vec<AnyElement>,
            Vec<AnyElement>,
        ) = match &self.open {
            Open::Nothing => return None,
            Open::Work(run) => {
                let mut actions = Vec::new();
                if !run.finished() {
                    let path = format!("/v1/work/{}/cancel", run.id);
                    actions.push(
                        button("work-stop", "Stop", false)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.post(path.clone(), None, None, cx)
                            }))
                            .into_any_element(),
                    );
                }
                let mut state = state_words(&run.state).to_owned();
                if let Some(why) = &run.why {
                    state = format!("{state}. {why}");
                }
                if let Some(pr) = run.pull_request() {
                    state = format!("{state} · {pr}");
                }
                let lines = run
                    .lines
                    .iter()
                    .map(|l| Item {
                        kind: "step".into(),
                        text: l.text.clone(),
                    })
                    .collect();
                (run.name(), state, lines, actions, Vec::new())
            }
            Open::Mac(job) => {
                let mut actions = Vec::new();
                if let Some(question) = &job.question {
                    for (key, label, primary) in
                        [("approve", "Approve", true), ("deny", "Deny", false)]
                    {
                        let path = format!("/v1/mac-jobs/{}/answer", job.id);
                        let body = json!({"question": question.id, "decision": key});
                        actions.push(
                            button(SharedString::from(format!("job-{key}")), label, primary)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.post(path.clone(), Some(body.clone()), None, cx)
                                }))
                                .into_any_element(),
                        );
                    }
                }
                if !job.finished() {
                    let path = format!("/v1/mac-jobs/{}/cancel", job.id);
                    actions.push(
                        button("job-stop", "Stop", false)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.post(path.clone(), None, None, cx)
                            }))
                            .into_any_element(),
                    );
                }
                let mut state = state_words(&job.state).to_owned();
                if let Some(text) = job.question.as_ref().map(|q| q.text.clone()) {
                    state = format!("{state}: {text}");
                } else if let Some(summary) = job.summary.clone().or(job.why.clone()) {
                    state = format!("{state}. {summary}");
                }
                let lines = job
                    .lines
                    .iter()
                    .map(|l| Item {
                        kind: "step".into(),
                        text: l.clone(),
                    })
                    .collect();
                (job.title.clone(), state, lines, actions, Vec::new())
            }
            Open::Environment(id, state) => {
                let Some(state) = state else {
                    return Some(
                        div()
                            .p(px(16.))
                            .child(back)
                            .child(footer_text("Loading…"))
                            .into_any_element(),
                    );
                };
                let mut extra = Vec::new();
                for run in &state.runs {
                    let (env, run_id) = (id.clone(), run.id.clone());
                    extra.push(self.row(
                        format!("env-run-{}", run.id),
                        run.prompt.clone(),
                        run.state.clone(),
                        move |this, cx| this.open_run(env.clone(), run_id.clone(), cx),
                        cx,
                    ));
                }
                extra.push(
                    div()
                        .mt(px(6.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(9.))
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::input_bg())
                        .text_size(px(13.5))
                        .child(self.message.clone())
                        .into_any_element(),
                );
                let mut status = state.environment.status.clone();
                if let Some(question) = &state.question {
                    status = format!("{status}: {question}");
                }
                (
                    state.environment.repository.clone(),
                    status,
                    state.items.clone(),
                    Vec::new(),
                    extra,
                )
            }
            Open::Run(env, run, state) => {
                let Some(state) = state else {
                    return Some(
                        div()
                            .p(px(16.))
                            .child(back)
                            .child(footer_text("Loading…"))
                            .into_any_element(),
                    );
                };
                let mut actions = Vec::new();
                for choice in &state.choices {
                    let path = format!("/environments/{env}/runs/{run}/choose");
                    let form = model::form(&[("choice", &choice.key)]);
                    actions.push(
                        button(
                            SharedString::from(format!("choose-{}", choice.key)),
                            leak(&choice.label),
                            true,
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.post(path.clone(), None, Some(form.clone()), cx)
                        }))
                        .into_any_element(),
                    );
                }
                if !state.finished {
                    let path = format!("/environments/{env}/runs/{run}/stop");
                    actions.push(
                        button("run-stop", "Stop", false)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.post(path.clone(), None, Some(String::new()), cx)
                            }))
                            .into_any_element(),
                    );
                }
                let status = match &state.paused {
                    Some(paused) => paused.clone(),
                    None => state.state.clone(),
                };
                (
                    "Claude Code".into(),
                    status,
                    state.items.clone(),
                    actions,
                    Vec::new(),
                )
            }
        };
        Some(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .px(px(14.))
                        .py(px(10.))
                        .border_b_1()
                        .border_color(theme::border())
                        .child(back)
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(theme::text_muted())
                                .child(state),
                        )
                        .when(!actions.is_empty(), |d| {
                            d.child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap(px(8.))
                                    .pt(px(4.))
                                    .children(actions),
                            )
                        }),
                )
                .child(
                    div()
                        .id("work-lines")
                        .flex_1()
                        .overflow_y_scroll()
                        .px(px(14.))
                        .py(px(10.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .children(lines.into_iter().map(render_item))
                        .children(extra),
                )
                .into_any_element(),
        )
    }
}

/// A button label from the site's words (`button` takes `&'static str`).
fn leak(label: &str) -> &'static str {
    match label {
        "Continue on your key" => "Continue on your key",
        "Continue with OpenAgents" => "Continue with OpenAgents",
        _ => "Continue",
    }
}

fn render_item(item: Item) -> AnyElement {
    let base = div().text_size(px(13.5)).child(item.text);
    match item.kind.as_str() {
        "user" => base
            .px(px(10.))
            .py(px(6.))
            .rounded(px(10.))
            .bg(theme::surface_card())
            .into_any_element(),
        "step" => base
            .font_family(theme::MONO)
            .text_size(px(12.5))
            .text_color(theme::text_muted())
            .into_any_element(),
        "note" => base.text_color(theme::text_faint()).into_any_element(),
        _ => base.into_any_element(),
    }
}

impl Render for WorkApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notice = self.notice.clone().map(|line| {
            div()
                .id("notice")
                .mx(px(10.))
                .mt(px(8.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(8.))
                .bg(theme::danger().opacity(0.08))
                .text_size(px(12.5))
                .text_color(theme::danger())
                .cursor_pointer()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.notice = None;
                    cx.notify();
                }))
                .child(line)
        });
        let body = match self.render_detail(cx) {
            Some(detail) => detail,
            None => self.render_list(cx),
        };
        let mut root = div()
            .id("openagents-web-work")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .border_l_1()
            .border_color(theme::border())
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY))
            .child(self.render_tabs(cx))
            .children(notice)
            .child(body);
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        root
    }
}
