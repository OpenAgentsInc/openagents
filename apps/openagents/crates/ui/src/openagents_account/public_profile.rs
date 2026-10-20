//! Your ID > Public profile (#12298): the shared Rust Native profile form
//! (`openagents_account::public::ProfileForm`) drawn in a dialog. The form
//! owns the fields, the Save button's state, and every word; this file only
//! mounts its fields as text inputs and passes Save on.
use super::AppAccount;
use crate::{
    composer::{ComposerInput, ComposerInputEvent},
    popover,
    theme::Theme,
};
use gpui::{
    AnyElement, AppContext, Context, Entity, Role, SharedString, Subscription, Task, div,
    prelude::*, px,
};
use gpui_tokio::Tokio;
use openagents_account::public::{Intent, KeySource, ProfileForm};
use rust_native::{Activation, Element, FieldChange, Node, TextRole};
use std::{collections::HashMap, sync::Arc};

/// The open dialog.
pub(super) struct ProfileDialog {
    form: Arc<ProfileForm>,
    inputs: HashMap<String, (Entity<ComposerInput>, String, Subscription)>,
    _watch: Task<()>,
    _load: Task<()>,
}

impl AppAccount {
    /// The key this computer signs with, for Agent Lab's owner tags.
    pub fn owner_key(&self) -> Option<KeySource> {
        let backend = self.id.backend.clone()?;
        Some(Arc::new(move || backend.key()))
    }

    pub(super) fn open_profile(&mut self, cx: &mut Context<Self>) {
        self.close_menu(cx);
        let (Some(controller), Some(key)) = (self.controller.clone(), self.owner_key()) else {
            return;
        };
        let form = ProfileForm::new(controller, key);
        let mut updates = form.watch();
        let watch = cx.spawn(async move |this, cx| {
            while updates.changed().await.is_ok() {
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        let load = Tokio::spawn(cx, {
            let form = form.clone();
            async move { form.load().await }
        });
        // A failed load shows in the form itself.
        let load = cx.spawn(async move |_, _| {
            let _ = load.await;
        });
        self.id.profile = Some(ProfileDialog {
            form,
            inputs: HashMap::new(),
            _watch: watch,
            _load: load,
        });
        cx.notify();
    }

    pub(super) fn close_profile(&mut self, cx: &mut Context<Self>) {
        self.id.profile = None;
        cx.notify();
    }

    fn profile_edit(&mut self, key: &str, value: String) {
        let Some(dialog) = &self.id.profile else {
            return;
        };
        let view = dialog.form.view();
        let _ = dialog.form.change_field(&FieldChange {
            activation: Activation {
                instance: view.instance,
                revision: view.revision,
                node: key.into(),
            },
            value,
        });
    }

    fn profile_save(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &self.id.profile else {
            return;
        };
        let form = dialog.form.clone();
        let view = form.view();
        let event = Activation {
            instance: view.instance,
            revision: view.revision,
            node: "save".into(),
        };
        // The outcome shows in the form.
        Tokio::spawn(cx, async move { form.activate(&event).await }).detach();
    }

    pub(super) fn profile_card(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.id.profile.as_ref()?.form.clone();
        let theme = Theme::of(cx).for_popup();
        let view = form.view();
        let rust_native::Element::Stack { children, .. } = &view.root.element else {
            return None;
        };
        let mut card = popover::dialog_card(&theme)
            .w(px(440.))
            .debug_selector(|| "openagents-profile".into())
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.close_profile(cx);
                    cx.stop_propagation();
                }
            }));
        let mut save: Option<(String, bool)> = None;
        for node in children {
            match &node.element {
                Element::Text { value, role } => {
                    card = card.child(match role {
                        TextRole::Heading => popover::dialog_title(&theme, value),
                        TextRole::Status => div()
                            .mt(px(8.))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(value.clone())),
                        _ => div()
                            .mt(px(4.))
                            .font_family(theme.font_mono.clone())
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(value.clone())),
                    });
                }
                Element::Field { .. } => {
                    let input = self.profile_input(node, cx);
                    let Element::Field { label, .. } = &node.element else {
                        continue;
                    };
                    card = card.child(
                        div()
                            .mt(px(12.))
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(label.clone())),
                    );
                    if let Some(input) = input {
                        card = card.child(
                            div()
                                .mt(px(4.))
                                .child(popover::dialog_field(input.into_any_element())),
                        );
                    }
                }
                Element::Button { label, enabled, .. } => save = Some((label.clone(), *enabled)),
                _ => {}
            }
        }
        let mut buttons = div().mt(px(16.)).flex().justify_end().gap(px(8.)).child(
            popover::btn_ghost(&theme, "Close", "openagents-profile-close")
                .id("openagents-profile-close")
                .on_click(cx.listener(|this, _, _, cx| this.close_profile(cx))),
        );
        if let Some((label, enabled)) = save {
            let mut button = popover::btn_primary(&theme, &label)
                .id("openagents-profile-save")
                .debug_selector(|| "openagents-profile-save".into());
            if enabled {
                button = button
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.profile_save(cx)));
            } else {
                button = button.opacity(0.5);
            }
            buttons = buttons.child(button);
        }
        Some(card.child(buttons).into_any_element())
    }

    /// The text input for one of the form's fields, kept in step with it.
    fn profile_input(
        &mut self,
        node: &Node<Intent>,
        cx: &mut Context<Self>,
    ) -> Option<Entity<ComposerInput>> {
        let Element::Field {
            label,
            value,
            multiline,
            enabled,
            secret: false,
            ..
        } = &node.element
        else {
            return None;
        };
        let dialog = self.id.profile.as_mut()?;
        if !dialog.inputs.contains_key(&node.key) {
            let entity = cx.new(|cx| {
                let input = ComposerInput::with_context(
                    label.clone(),
                    if *multiline {
                        "AgentLabInput"
                    } else {
                        "AgentLabSingleLine"
                    },
                    cx,
                )
                .with_text_metrics(13., 20.)
                .with_tab_stop()
                .with_accessibility_role(if *multiline {
                    Role::MultilineTextInput
                } else {
                    Role::TextInput
                });
                let mut input = if *multiline {
                    input.with_viewport_height(96.)
                } else {
                    input.with_single_line()
                };
                input.set_text(value.clone(), cx);
                input
            });
            let key = node.key.clone();
            let events = cx.subscribe(&entity, move |this: &mut Self, input, event, cx| {
                if matches!(event, ComposerInputEvent::Edited) {
                    let text = input.read(cx).text().to_owned();
                    this.profile_edit(&key, text);
                }
            });
            dialog
                .inputs
                .insert(node.key.clone(), (entity, value.clone(), events));
        }
        let (entity, shown, _) = dialog.inputs.get_mut(&node.key)?;
        // Only a changed form value replaces the text, so a repaint can't
        // overwrite an edit still on its way.
        let changed = shown != value;
        *shown = value.clone();
        let entity = entity.clone();
        let read_only = !*enabled;
        if changed || entity.read(cx).read_only != read_only {
            let value = value.clone();
            entity.update(cx, |input, cx| {
                input.read_only = read_only;
                if changed && input.text() != value {
                    input.set_text(value, cx);
                }
            });
        }
        Some(entity)
    }
}
