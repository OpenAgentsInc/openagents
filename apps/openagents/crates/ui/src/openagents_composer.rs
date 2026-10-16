//! The normal text input with account-cloud send and stop actions.
use crate::{
    composer::{ComposerInput, ComposerInputEvent},
    theme::Theme,
};
use coder_account::{openagents::ChatSource, reader::Reader, turns::Turns};
use gpui::{prelude::*, *};
use gpui_tokio::Tokio;
use std::{future::Future, sync::Arc};

pub struct AccountComposer {
    reader: Arc<Reader>,
    pub turns: Arc<Turns>,
    input: Entity<ComposerInput>,
    accepted: u64,
    submitted: String,
    _events: Subscription,
    _watch: Task<()>,
    _follow: Task<()>,
}
impl AccountComposer {
    pub fn new(reader: Arc<Reader>, turns: Arc<Turns>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| ComposerInput::new("Message OpenAgents", cx));
        let events = cx.subscribe(&input, |this, _, event, cx| match event {
            ComposerInputEvent::Submitted => this.submit(cx),
            ComposerInputEvent::Edited => cx.notify(),
            _ => {}
        });
        let mut updates = turns.watch();
        let watch = cx.spawn(async move |this, cx| {
            while updates.changed().await.is_ok() {
                let update = updates.borrow_and_update().clone();
                if this
                    .update(cx, |this, cx| {
                        if update.accepted > this.accepted
                            && this.input.read(cx).text() == this.submitted
                        {
                            this.input.update(cx, |input, cx| input.set_text("", cx));
                        }
                        this.accepted = update.accepted;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let follow = cx.spawn({
            let turns = turns.clone();
            async move |_, cx| loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                if Tokio::spawn(cx, turns.follow()).await.is_err() {
                    break;
                }
            }
        });
        Self {
            reader,
            turns,
            input,
            accepted: 0,
            submitted: String::new(),
            _events: events,
            _watch: watch,
            _follow: follow,
        }
    }
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.turns.clear();
        self.submitted.clear();
        self.accepted = 0;
        self.input.update(cx, |i, cx| i.set_text("", cx));
        cx.notify();
    }
    fn launch(&self, f: impl Future<Output = ()> + Send + 'static, cx: &mut Context<Self>) {
        let task = Tokio::spawn(cx, f);
        cx.spawn(async move |_, _| {
            let _ = task.await;
        })
        .detach();
    }
    #[cfg(feature = "account-fixture")]
    pub fn fixture_submit(&mut self, text: String, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text(text, cx));
        self.submit(cx);
    }
    fn submit(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.turns.snapshot();
        let text = self.input.read(cx).text().to_owned();
        if snapshot.busy || snapshot.retry || text.trim().is_empty() {
            return;
        }
        self.submitted = text.clone();
        self.launch(self.turns.submit(text), cx);
        cx.notify();
    }
}
impl Render for AccountComposer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let snapshot = self.turns.snapshot();
        if self
            .reader
            .snapshot()
            .summary
            .as_ref()
            .is_none_or(|s| s.source != ChatSource::Web)
        {
            return div()
                .px(px(24.))
                .py(px(12.))
                .text_color(theme.text_muted)
                .child("Continue this chat on its computer.")
                .into_any_element();
        }
        let mut body = div()
            .id("cloud-composer")
            .mx(px(24.))
            .mb(px(16.))
            .p(px(12.))
            .rounded(px(16.))
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(8.));
        if let Some(error) = snapshot.error.as_ref() {
            body = body.child(
                div()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .child(SharedString::from(error.clone())),
            );
        }
        body = body.child(
            div()
                .min_h(px(36.))
                .max_h(px(160.))
                .child(self.input.clone()),
        );
        let mut controls = div().flex().justify_between().items_center().child(
            div()
                .text_sm()
                .text_color(theme.text_muted)
                .child("OpenAgents"),
        );
        let label = if snapshot.stopping {
            "Stopping…"
        } else if snapshot.busy {
            "Stop"
        } else if snapshot.retry {
            "Retry"
        } else {
            "Send"
        };
        let enabled = !snapshot.stopping
            && (snapshot.busy || snapshot.retry || !self.input.read(cx).text().trim().is_empty());
        controls = controls.child(
            div()
                .id("cloud-send")
                .role(gpui::Role::Button)
                .tab_index(0)
                .px(px(12.))
                .py(px(6.))
                .rounded(px(10.))
                .border_1()
                .border_color(theme.border)
                .when(enabled, |button| {
                    button
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let state = this.turns.snapshot();
                            if state.busy {
                                this.launch(this.turns.stop(), cx);
                            } else if state.retry {
                                this.launch(this.turns.retry(), cx);
                            } else {
                                this.submit(cx);
                            }
                        }))
                })
                .child(label),
        );
        body.child(controls).into_any_element()
    }
}
