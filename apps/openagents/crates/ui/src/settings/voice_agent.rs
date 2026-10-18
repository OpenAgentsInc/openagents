//! Desktop adapter for the shared voice-agent picker.
use gpui::{AnyElement, Context, Render, SharedString, Subscription, Task, Window, div, prelude::*, px};
use gpui_tokio::Tokio;
use openagents_account::voice_agent::{Action, Picker};
use rust_native::{Activation, Element, Node, View};
use std::sync::Arc;
use crate::{openagents_account::AppAccount, theme::Theme};

pub struct VoiceAgentCard {
    picker: Option<Arc<Picker>>,
    view: Option<View<Action>>,
    task: Option<Task<()>>,
    _refresh: Task<()>,
    _account: Option<Subscription>,
}
impl VoiceAgentCard {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let refresh = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(5))
                    .await;
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break;
                }
            }
        });
        let account = AppAccount::global(cx).map(|account| {
            cx.observe(&account, |this: &mut Self, _, cx| {
                this.view = this.picker.as_ref().map(|p| p.view());
                this.refresh(cx);
                cx.notify();
            })
        });
        let mut this = Self {
            picker: None,
            view: None,
            task: None,
            _refresh: refresh,
            _account: account,
        };
        this.refresh(cx);
        this
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let controller = AppAccount::global(cx).and_then(|a| a.read(cx).controller.clone());
        let Some(controller) = controller else {
            self.picker = None;
            self.view = None;
            return;
        };
        let picker = self
            .picker
            .get_or_insert_with(|| Picker::new(controller))
            .clone();
        if self.task.is_some() {
            return;
        }
        let owned = picker.clone();
        let job = Tokio::spawn(cx, async move {
            let _ = owned.refresh().await;
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let _ = job.await;
            this.update(cx, |this, cx| {
                this.view = Some(picker.view());
                this.task = None;
                cx.notify();
            })
            .ok();
        }));
    }
    fn activate(&mut self, key: String, cx: &mut Context<Self>) {
        let (Some(picker), Some(view)) = (self.picker.clone(), self.view.as_ref()) else {
            return;
        };
        if self.task.is_some() {
            return;
        }
        let event = Activation {
            instance: view.instance.clone(),
            revision: view.revision,
            node: key,
        };
        let owned = picker.clone();
        let job = Tokio::spawn(cx, async move {
            let _ = owned.activate(event).await;
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let _ = job.await;
            this.update(cx, |this, cx| {
                this.view = Some(picker.view());
                this.task = None;
                cx.notify();
            })
            .ok();
        }));
    }
    fn mount(&self, node: &Node<Action>, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).clone();
        match &node.element {
            Element::Stack { children, .. } => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(children.iter().map(|n| self.mount(n, cx)))
                .into_any_element(),
            Element::Text { value, .. } => div()
                .text_color(theme.text_muted)
                .child(value.clone())
                .into_any_element(),
            Element::Button { label, enabled, .. } => {
                let key = node.key.clone();
                div()
                    .id(SharedString::from(key.clone()))
                    .role(gpui::Role::Button)
                    .aria_label(label.clone())
                    .tab_index(0)
                    .px(px(10.))
                    .py(px(6.))
                    .rounded_md()
                    .bg(theme.surface)
                    .text_color(theme.text)
                    .when(*enabled && self.task.is_none(), |d| {
                        d.cursor_pointer().on_click(
                            cx.listener(move |this, _, _, cx| this.activate(key.clone(), cx)),
                        )
                    })
                    .child(label.clone())
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}
impl Render for VoiceAgentCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.view = self.picker.as_ref().map(|p| p.view());
        match self.view.clone() {
            Some(v) => self.mount(&v.root, cx),
            None => div()
                .child("Sign in to choose a voice agent.")
                .into_any_element(),
        }
    }
}
