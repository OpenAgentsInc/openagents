//! A saved-chat reference follows the normal new-session folder and harness pickers.
//! The Runs on control sits in the composer footer and lists the chat's homes
//! in a menu drawn from the shared `chat_home::view` (#11898, #11959).
use crate::{
    composer::Composer,
    icons::{self, icon},
    motion, popover,
    theme::Theme,
};
use gpui::{prelude::*, *};
use openagents_account::{history::ExistingChat, openagents::continuations::Source};
use rust_native::{Activation, Element, TextRole};
use std::{collections::HashMap, sync::Arc};

struct Attempt {
    target: ExistingChat,
    chat: String,
    message: String,
}
#[derive(Default)]
pub struct Drafts(HashMap<String, Arc<Attempt>>, HomeControl);
#[derive(Default)]
struct HomeControl {
    revision: u64,
    fingerprint: String,
    choices: Option<rust_native::ValidatedView<chat_home::Home>>,
    parts: Option<rust_native::ValidatedView<chat_home::PartRef>>,
    attempts: HashMap<String, chat_home::PartRef>,
    menu: popover::Popup<()>,
}
struct HomeContext {
    chat: String,
    lease: openagents_account::Lease,
    current: chat_home::view::Destination,
}
/// One entry of a shared home view, drawn as a menu row or a note.
enum MenuItem<I> {
    Note(String),
    Choice {
        activation: Activation,
        label: String,
        enabled: bool,
        intent: I,
    },
}
impl<I> MenuItem<I> {
    fn is_choice(&self) -> bool {
        matches!(self, Self::Choice { .. })
    }
}
fn menu_items<I: Clone>(view: &rust_native::ValidatedView<I>) -> Vec<MenuItem<I>> {
    let view = view.view();
    let Element::Stack { children, .. } = &view.root.element else {
        return Vec::new();
    };
    children
        .iter()
        .filter_map(|node| match &node.element {
            Element::Text {
                value,
                role: TextRole::Status,
            } => Some(MenuItem::Note(value.clone())),
            Element::Button {
                label,
                enabled,
                intent,
                ..
            } => Some(MenuItem::Choice {
                activation: Activation {
                    instance: view.instance.clone(),
                    revision: view.revision,
                    node: node.key.clone(),
                },
                label: label.clone(),
                enabled: *enabled,
                intent: intent.clone(),
            }),
            _ => None,
        })
        .collect()
}
fn home_icon(home: &chat_home::Home) -> &'static str {
    match home {
        chat_home::Home::Cloud => icons::CLOUD,
        _ => icons::LAPTOP,
    }
}
impl Drafts {
    pub fn lease(&self, key: &str) -> Option<openagents_account::Lease> {
        self.0.get(key).map(|a| a.target.lease.clone())
    }
    pub fn native_source(&self, key: &str) -> Option<(String, ExistingChat)> {
        let target = &self.0.get(key)?.target;
        let chat_home::Home::Computer(device) = &target.summary.home else {
            return None;
        };
        matches!(&target.summary.source, openagents_account::openagents::ChatSource::Other(kind) if kind == "session")
            .then(|| (device.clone(), target.clone()))
    }
    pub fn install(&mut self, target: ExistingChat) {
        self.install_at(target, uuid::Uuid::new_v4().to_string());
    }
    pub fn install_at(&mut self, target: ExistingChat, chat: String) {
        self.0.insert(
            String::new(),
            Arc::new(Attempt {
                target,
                chat,
                message: uuid::Uuid::new_v4().to_string(),
            }),
        );
    }
    pub fn validate(&self, key: &str, state: &crate::state::AppState) -> Result<(), &'static str> {
        let Some(source) = self.0.get(key) else {
            return Ok(());
        };
        if !source.target.is_current() {
            return Err("Sign in again before continuing.");
        }
        let engine = state
            .engine()
            .ok_or("Connect this computer before continuing.")?;
        if !engine
            .engine_info()
            .supports(openagents_proto::capabilities::OPENAGENTS_CONTINUATIONS_V1)
        {
            return Err("Update OpenAgents on this computer before continuing.");
        }
        if !key.is_empty() {
            return Ok(());
        }
        let Some(project) = state.selected_space_row() else {
            return Err("Choose a project folder on this computer.");
        };
        if let Some((source, _)) = self.native_source(key) {
            if project.device_id == source {
                return Err("Choose another computer to continue this chat.");
            }
            if !state.devices.iter().any(|d| {
                d.id == project.device_id
                    && d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1)
            }) {
                return Err("Update OpenAgents on the selected computer before continuing.");
            }
        }
        if !state.devices.iter().any(|device| {
            device.id == project.device_id
                && device.supports(openagents_proto::capabilities::OPENAGENTS_CONTINUATIONS_V1)
        }) && state.local_device_id.as_deref() != Some(project.device_id.as_str())
        {
            return Err("Update OpenAgents on the selected computer before continuing.");
        }
        Ok(())
    }
    pub fn preferred_chat(&self, key: &str) -> Option<String> {
        self.0.get(key).map(|a| a.chat.clone())
    }
    pub fn message(&self, key: &str) -> Option<String> {
        self.0.get(key).map(|a| a.message.clone())
    }
    pub fn accepted(&mut self, chat: &str) {
        if self.0.get("").is_some_and(|a| a.chat == chat) {
            self.0.remove("");
        }
        self.0.remove(chat);
    }
    pub fn observe_active(&mut self, key: &str, state: &crate::state::AppState) {
        if !key.is_empty()
            && state
                .selected_chat_row()
                .and_then(|r| r.config.as_ref())
                .is_some_and(|c| {
                    !c.model_options
                        .contains_key(openagents_account::openagents::continuations::REQUEST_OPTION)
                })
        {
            self.accepted(key);
        }
    }
    pub fn bind(&mut self, key: &str, chat: &str) -> Option<Source> {
        if key != chat {
            if let Some(a) = self.0.get(key).cloned() {
                self.0.insert(chat.into(), a);
            }
        }
        self.0.get(chat).map(|a| Source {
            account: a.target.chat.account().into(),
            chat: a.target.chat.id().into(),
            revision: a.target.summary.revision,
        })
    }
    pub fn banner(&self, key: &str, cx: &mut Context<Composer>) -> Option<AnyElement> {
        let attempt = self.0.get(key).filter(|a| a.target.is_current())?;
        let target = &attempt.target;
        let hover = Theme::of(cx).glass_hover();
        Some(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.))
                .py(px(4.))
                .text_size(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(SharedString::from(format!("From {}", target.summary.title))),
                )
                .when(key.is_empty(), |line| {
                    line.child(
                        div()
                            .id("cancel-saved-continuation")
                            .cursor_pointer()
                            .px(px(6.))
                            .rounded(px(4.))
                            .hover(move |s| s.bg(hover))
                            .child("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.continuations.0.remove(&this.current_key);
                                cx.notify();
                            })),
                    )
                })
                .into_any_element(),
        )
    }
}
/// What a Runs on control shows: the chat's current home, the shared choice
/// and part views, and the menu's open state.
pub(crate) struct RunsOn<'a> {
    pub id: &'static str,
    pub current: &'a chat_home::view::Destination,
    pub choices: Option<&'a rust_native::ValidatedView<chat_home::Home>>,
    pub parts: Option<&'a rust_native::ValidatedView<chat_home::PartRef>>,
    pub menu: &'a popover::Popup<()>,
}

type MenuState<V> = fn(&mut V) -> &mut popover::Popup<()>;

fn close_menu<V: 'static>(this: &mut V, menu: MenuState<V>, cx: &mut Context<V>) {
    if menu(this).begin_close() {
        popover::reap_popup(cx, menu);
        cx.notify();
    }
}

/// A small "Runs on This Mac" control. Its menu lists the chat's current home,
/// the reason it can't move (only when one applies), the places it can move
/// to, and links to its other parts. Shows nothing when there is nothing to
/// choose.
pub(crate) fn runs_on<V: 'static>(
    spec: RunsOn,
    menu_state: MenuState<V>,
    choose: impl Fn(&mut V, Activation, &mut Context<V>) + 'static,
    open_part: impl Fn(&mut V, Activation, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> Option<AnyElement> {
    let choices = spec.choices.map(menu_items).unwrap_or_default();
    let parts = spec.parts.map(menu_items).unwrap_or_default();
    if !choices.iter().any(MenuItem::is_choice) && !parts.iter().any(MenuItem::is_choice) {
        return None;
    }
    let theme = Theme::of(cx).clone();
    let id = spec.id;
    let trigger_key = SharedString::from(format!("{id}-trigger"));
    let open = spec.menu.is_open();
    let label = spec.current.label.clone();
    let ink = if open {
        theme.text_muted
    } else {
        motion::hover_blend(
            &trigger_key,
            theme.text_muted.opacity(0.6),
            theme.text_muted,
        )
    };
    let selector = trigger_key.to_string();
    let mut trigger = div()
        .id(trigger_key.clone())
        .debug_selector(move || selector.clone())
        .role(gpui::Role::Button)
        .aria_label(format!("Runs on {label}"))
        .aria_expanded(open)
        .tab_index(0)
        .focus_visible(|s| s.border_2().border_color(theme.accent))
        .relative()
        .flex_none()
        .h(px(20.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.))
        .px(px(6.))
        .rounded(px(6.))
        .text_size(crate::typography::ui_rems(12.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(ink)
        .bg(if open {
            theme.glass_hover()
        } else {
            motion::hover_blend(
                &trigger_key,
                theme.glass_hover().opacity(0.0),
                theme.glass_hover(),
            )
        })
        .on_hover(motion::hover_listener(trigger_key.clone()))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, _| menu_state(this).note_trigger_press()),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            if menu_state(this).take_press_was_open() {
                close_menu(this, menu_state, cx);
            } else {
                menu_state(this).open(());
            }
            cx.notify();
        }))
        .child(
            icon(home_icon(&spec.current.home))
                .size(px(12.))
                .flex_none()
                .text_color(ink),
        )
        .child(
            div()
                .whitespace_nowrap()
                .child(SharedString::from(format!("Runs on {label}"))),
        )
        .child(
            icon(icons::ALT_ARROW_DOWN)
                .size(px(10.))
                .flex_none()
                .text_color(ink),
        );
    if spec.menu.get().is_some() {
        let theme = theme.for_popup();
        let choose = std::rc::Rc::new(choose);
        let open_part = std::rc::Rc::new(open_part);
        let mut menu = popover::popover_card(&theme)
            .w(px(256.))
            .on_mouse_down_out(cx.listener(move |this, _, _, cx| close_menu(this, menu_state, cx)))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(popover::menu_heading(&theme, "Runs on"))
            .child(
                popover::menu_row(&theme, true, format!("{id}-current"))
                    .cursor_default()
                    .child(
                        icon(home_icon(&spec.current.home))
                            .size(px(16.))
                            .text_color(theme.text_muted),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .child(
                        icon(icons::CHECK)
                            .size(px(14.))
                            .text_color(theme.text_muted),
                    ),
            );
        let mut moves = Vec::new();
        for item in choices {
            match item {
                MenuItem::Note(reason) => {
                    menu = menu.child(
                        div()
                            .id(SharedString::from(format!("{id}-reason")))
                            .px(px(8.))
                            .pt(px(2.))
                            .pb(px(4.))
                            .text_size(crate::typography::ui_rems(12.))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(reason)),
                    );
                }
                MenuItem::Choice {
                    activation,
                    label,
                    enabled,
                    intent,
                } => {
                    let key = SharedString::from(format!("{id}-{}", activation.node));
                    let row = if enabled {
                        let choose = choose.clone();
                        popover::menu_row(&theme, false, key.clone())
                            .id(key)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                close_menu(this, menu_state, cx);
                                choose(this, activation.clone(), cx);
                            }))
                    } else {
                        div()
                            .id(key)
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(8.))
                            .py(px(6.))
                            .text_size(crate::typography::ui_rems(13.))
                            .text_color(theme.text_muted.opacity(0.6))
                    };
                    moves.push(
                        row.child(
                            icon(home_icon(&intent))
                                .size(px(16.))
                                .text_color(theme.text_muted),
                        )
                        .child(div().min_w_0().truncate().child(label))
                        .into_any_element(),
                    );
                }
            }
        }
        if !moves.is_empty() {
            menu = menu
                .child(popover::menu_separator())
                .child(popover::menu_heading(&theme, "Move to"))
                .children(moves);
        }
        let parts: Vec<_> = parts
            .into_iter()
            .filter_map(|item| match item {
                MenuItem::Choice {
                    activation, label, ..
                } => Some((activation, label)),
                MenuItem::Note(_) => None,
            })
            .collect();
        if !parts.is_empty() {
            menu = menu.child(popover::menu_separator());
            for (activation, label) in parts {
                let key = SharedString::from(format!("{id}-part-{}", activation.node));
                let open_part = open_part.clone();
                menu = menu.child(
                    popover::menu_row(&theme, false, key.clone())
                        .id(key)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            close_menu(this, menu_state, cx);
                            open_part(this, activation.clone(), cx);
                        }))
                        .child(
                            icon(icons::ARROW_UP_RIGHT)
                                .size(px(16.))
                                .text_color(theme.text_muted),
                        )
                        .child(label),
                );
            }
        }
        trigger = trigger.child(popover::anchored_menu_above_end(
            SharedString::from(format!("{id}-menu")),
            menu.into_any_element(),
            spec.menu.closing_since(),
        ));
    }
    Some(trigger.into_any_element())
}

impl Composer {
    pub fn continue_saved_chat_at(
        &mut self,
        target: ExistingChat,
        chat: String,
        cx: &mut Context<Self>,
    ) {
        self.continuations.install_at(target, chat);
        self.input
            .update(cx, |input, cx| input.set_text("Continue from here.", cx));
        cx.notify();
    }

    /// Rebuild the shared Runs on and part views when their inputs change.
    fn refresh_home_views(&mut self, cx: &mut Context<Self>) -> Option<HomeContext> {
        use chat_home::{Home, view::Destination};
        let state = self.state.read(cx);
        let chat = state.selected_chat_row()?.clone();
        if !chat.is_top_level()
            || chat
                .config
                .as_ref()
                .is_some_and(|config| config.model_options.contains_key(chat_home::REQUEST_OPTION))
        {
            return None;
        }
        let lease = crate::openagents_account::AppAccount::global(cx)
            .and_then(|a| a.read(cx).controller.clone())
            .and_then(|a| a.lease())?;
        let local = state.local_device_id.as_deref();
        let current = Destination {
            home: Home::Computer(chat.device_id.clone()),
            available: true,
            label: if local == Some(chat.device_id.as_str()) {
                "This Mac".into()
            } else {
                state
                    .devices
                    .iter()
                    .find(|d| d.id == chat.device_id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "Computer".into())
            },
        };
        let mut destinations = vec![Destination {
            home: Home::Cloud,
            label: "OpenAgents".into(),
            available: true,
        }];
        destinations.extend(
            state
                .devices
                .iter()
                .filter(|d| !matches!(d.platform.as_str(), "ios" | "android" | "ipados"))
                .filter(|d| d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1))
                .filter_map(|d| {
                    Some(Destination {
                        home: Home::computer(&d.id)?,
                        label: if local == Some(d.id.as_str()) {
                            "This Mac".into()
                        } else {
                            d.name.clone()
                        },
                        available: true,
                    })
                }),
        );
        let answering = state
            .sessions
            .iter()
            .find(|s| s.chat_id == chat.id)
            .is_some_and(|s| {
                matches!(
                    s.status,
                    openagents_proto::SessionStatus::Working
                        | openagents_proto::SessionStatus::AwaitingInput
                )
            });
        let can_move = state.devices.iter().any(|d| {
            d.id == chat.device_id
                && d.supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1)
        }) || (local == Some(chat.device_id.as_str())
            && state.engine().is_some_and(|e| {
                e.engine_info()
                    .supports(openagents_proto::capabilities::CHAT_HOME_TRANSFER_V1)
            }));
        if !can_move {
            destinations.clear();
        }
        let blocked = if chat.links.continued_by.is_some() {
            Some("This chat continues elsewhere.")
        } else if answering {
            Some("Stop the answer to move this chat.")
        } else if !state.queue.is_empty() {
            Some("Send or clear your queued messages to move this chat.")
        } else if !self.input.read(cx).text().trim().is_empty()
            || self
                .attachments
                .get(&self.current_key)
                .is_some_and(|a| !a.is_empty())
        {
            Some("Send or clear your draft to move this chat.")
        } else {
            None
        };
        let ui = &mut self.continuations.1;
        let fingerprint = serde_json::to_string(&(
            &chat.id,
            &chat.links,
            lease.generation,
            &current.home,
            &current.label,
            blocked,
            destinations
                .iter()
                .map(|d| (&d.home, &d.label, d.available))
                .collect::<Vec<_>>(),
        ))
        .ok()?;
        if ui.fingerprint != fingerprint {
            ui.revision = ui.revision.wrapping_add(1);
            ui.choices = chat_home::view::choices(
                &format!("home-{}", chat.id),
                ui.revision,
                &current,
                &destinations,
                blocked,
            )
            .ok();
            ui.parts =
                chat_home::view::parts(&format!("parts-{}", chat.id), ui.revision, &chat.links)
                    .ok();
            ui.fingerprint = fingerprint;
        }
        Some(HomeContext {
            chat: chat.id,
            lease,
            current,
        })
    }

    /// A chat whose next part lives elsewhere: one line in place of the input.
    pub(crate) fn chat_home_controls(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let home = self.refresh_home_views(cx)?;
        let theme = Theme::of(cx).clone();
        let parts = self
            .continuations
            .1
            .parts
            .as_ref()
            .map(menu_items)
            .unwrap_or_default();
        let mut row = div()
            .id("chat-continues-elsewhere")
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .text_size(crate::typography::ui_rems(12.))
            .text_color(theme.text_muted)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child("This chat continues elsewhere."),
            );
        for item in parts {
            let MenuItem::Choice {
                activation,
                label,
                enabled,
                ..
            } = item
            else {
                continue;
            };
            let key = SharedString::from(format!("chat-part-{}", activation.node));
            let (chat, lease) = (home.chat.clone(), home.lease.clone());
            row = row.child(
                div()
                    .id(key.clone())
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(6.))
                    .text_color(theme.text)
                    .bg(motion::hover_blend(
                        &key,
                        theme.glass_hover().opacity(0.6),
                        theme.glass_hover(),
                    ))
                    .on_hover(motion::hover_listener(key.clone()))
                    .when(enabled, |part| {
                        part.cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_part(activation.clone(), &chat, &lease, cx);
                            }))
                    })
                    .child(label)
                    .child(icon(icons::ARROW_UP_RIGHT).size(px(11.)).flex_none()),
            );
        }
        Some(row.into_any_element())
    }

    /// Runs on: a small control in the composer footer. Its menu holds the
    /// places this chat can move to, and the reason it can't move right now.
    pub(crate) fn runs_on_control(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let home = self.refresh_home_views(cx)?;
        let ui = &self.continuations.1;
        let (chat, lease) = (home.chat.clone(), home.lease.clone());
        let (part_chat, part_lease) = (home.chat, home.lease);
        runs_on(
            RunsOn {
                id: "runs-on",
                current: &home.current,
                choices: ui.choices.as_ref(),
                parts: ui.parts.as_ref(),
                menu: &ui.menu,
            },
            |this: &mut Self| &mut this.continuations.1.menu,
            move |this: &mut Self, event, cx| this.choose_home(event, &chat, &lease, cx),
            move |this: &mut Self, event, cx| this.open_part(event, &part_chat, &part_lease, cx),
            cx,
        )
    }

    fn choose_home(
        &mut self,
        event: Activation,
        source: &str,
        lease: &openagents_account::Lease,
        cx: &mut Context<Self>,
    ) {
        use chat_home::PartRef;
        if !lease.is_current()
            || self.state.read(cx).selected_chat.as_deref() != Some(source)
            || !self.input.read(cx).text().trim().is_empty()
        {
            return;
        }
        let Some(home) = self
            .continuations
            .1
            .choices
            .as_ref()
            .and_then(|v| v.activate(&event).ok())
            .cloned()
        else {
            return;
        };
        let target = self
            .continuations
            .1
            .attempts
            .entry(format!(
                "{}:{}",
                source,
                serde_json::to_string(&home).unwrap()
            ))
            .or_insert_with(|| PartRef {
                id: uuid::Uuid::new_v4().to_string(),
                home,
            })
            .clone();
        cx.emit(crate::composer::ComposerEvent::ContinueOn {
            chat: source.into(),
            target,
        });
    }

    fn open_part(
        &mut self,
        event: Activation,
        source: &str,
        lease: &openagents_account::Lease,
        cx: &mut Context<Self>,
    ) {
        if !lease.is_current() || self.state.read(cx).selected_chat.as_deref() != Some(source) {
            return;
        }
        if let Some(part) = self
            .continuations
            .1
            .parts
            .as_ref()
            .and_then(|v| v.activate(&event).ok())
            .cloned()
        {
            cx.emit(crate::composer::ComposerEvent::OpenPart(part));
        }
    }

    #[cfg(feature = "account-fixture")]
    pub fn fixture_open_runs_on(&mut self, cx: &mut Context<Self>) {
        self.continuations.1.menu.open(());
        cx.notify();
    }
    pub fn continue_saved_chat(&mut self, target: ExistingChat, cx: &mut Context<Self>) {
        self.continuations.install(target);
        self.input
            .update(cx, |input, cx| input.set_text("Continue from here.", cx));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{MenuItem, menu_items};
    use chat_home::{Home, view::Destination};

    fn place(home: Home, label: &str) -> Destination {
        Destination {
            home,
            label: label.into(),
            available: true,
        }
    }

    #[test]
    fn the_runs_on_menu_shows_a_reason_only_when_the_chat_cannot_move() {
        let current = place(Home::Computer("mac".into()), "This Mac");
        let places = [place(Home::Cloud, "OpenAgents"), current.clone()];
        let idle = chat_home::view::choices("fixture", 1, &current, &places, None).unwrap();
        let items = menu_items(&idle);
        assert!(!items.iter().any(|item| matches!(item, MenuItem::Note(_))));
        assert!(matches!(
            items.as_slice(),
            [MenuItem::Choice {
                enabled: true,
                intent: Home::Cloud,
                ..
            }]
        ));
        let busy = chat_home::view::choices(
            "fixture",
            2,
            &current,
            &places,
            Some("Stop the answer to move this chat."),
        )
        .unwrap();
        let items = menu_items(&busy);
        assert!(matches!(
            items.as_slice(),
            [MenuItem::Note(reason), MenuItem::Choice { enabled: false, .. }]
                if reason == "Stop the answer to move this chat."
        ));
        let MenuItem::Choice { activation, .. } = &items[1] else {
            unreachable!()
        };
        assert!(busy.activate(activation).is_err());
    }
}
