//! GPUI controls for shared Rust Native action views.
use gpui::{prelude::*, *};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView};
use std::rc::Rc;

type Action<V> = Rc<dyn Fn(&mut V, Activation, &mut Context<V>)>;

pub fn mount<I: 'static, V: 'static>(
    view: &ValidatedView<I>,
    action: impl Fn(&mut V, Activation, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> AnyElement {
    mount_node(
        &view.view().root,
        &view.view().instance,
        view.view().revision,
        Rc::new(action),
        cx,
    )
}

fn mount_node<I: 'static, V: 'static>(
    node: &Node<I>,
    instance: &str,
    revision: u64,
    action: Action<V>,
    cx: &mut Context<V>,
) -> AnyElement {
    let theme = crate::theme::Theme::of(cx).clone();
    match &node.element {
        Element::Stack { axis, children } => {
            let mut stack = div().flex().gap(px(6.));
            stack = if *axis == Axis::Horizontal {
                stack.flex_row()
            } else {
                stack.flex_col()
            };
            for child in children {
                stack = stack.child(mount_node(child, instance, revision, action.clone(), cx));
            }
            stack.into_any_element()
        }
        Element::Text { value, role } => div()
            .text_size(crate::typography::ui_rems(if *role == TextRole::Heading {
                14.
            } else {
                12.
            }))
            .text_color(theme.text_muted)
            .child(value.clone())
            .into_any_element(),
        Element::Button { label, enabled, .. } => {
            let activation = Activation {
                instance: instance.into(),
                revision,
                node: node.key.clone(),
            };
            div()
                .id(SharedString::from(node.key.clone()))
                .px(px(10.))
                .py(px(4.))
                .rounded_md()
                .text_size(crate::typography::ui_rems(12.))
                .text_color(theme.text)
                .bg(theme.surface)
                .when(*enabled, |button| {
                    button
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            action(this, activation.clone(), cx);
                        }))
                })
                .child(label.clone())
                .into_any_element()
        }
        _ => div().into_any_element(),
    }
}
