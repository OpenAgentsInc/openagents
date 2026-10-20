//! The GPUI adapter for Rust Native views (#12218): cards and sheets the
//! phone, the website, the desktop, and the GPUI web app draw from one view
//! tree. The desktop (`openagents_ui::native_actions`) and the web apps
//! under `apps/openagents/web` mount views through this one module.
//!
//! A node's [`Style`] applies only where it sets a field, so a view built
//! with default styles draws as it always has. Elements the adapter has no
//! control for yet (`Field`, `Surface`, and the transcript elements) draw
//! nothing; a native surface such as CAD or Robot is placed by its host.

use gpui::{prelude::*, *};
use rust_native::markdown::{Block, Span};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Activation, Axis, Element, Node, RichRun, TextRole, ValidatedView};
use std::rc::Rc;

use crate::theme::Theme;
use crate::typography::ui_rems;

type Action<V> = Rc<dyn Fn(&mut V, Activation, &mut Context<V>)>;

/// Mount a validated view. `action` receives each activation; the caller
/// resolves it against the view it mounted (`ValidatedView::activate`).
pub fn mount<I: 'static, V: 'static>(
    view: &ValidatedView<I>,
    action: impl Fn(&mut V, Activation, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> AnyElement {
    mount_root(
        &view.view().root,
        &view.view().instance,
        view.view().revision,
        action,
        cx,
    )
}

/// Mount a node tree that the caller validated or built itself, such as a
/// decision panel, under `instance` and `revision`. Activations name the
/// pressed node's key.
pub fn mount_root<I: 'static, V: 'static>(
    root: &Node<I>,
    instance: &str,
    revision: u64,
    action: impl Fn(&mut V, Activation, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> AnyElement {
    let theme = Theme::of(cx).clone();
    let mount = Mount {
        instance,
        revision,
        action: Rc::new(action),
        theme: &theme,
    };
    mount.node(root, cx)
}

struct Mount<'a, V> {
    instance: &'a str,
    revision: u64,
    action: Action<V>,
    theme: &'a Theme,
}

/// Points for a [`Space`] step, as every Rust Native adapter maps them.
pub fn space(value: Space) -> f32 {
    match value {
        Space::None => 0.,
        Space::Xs => 4.,
        Space::Sm => 8.,
        Space::Md => 16.,
        Space::Lg => 24.,
    }
}

/// A Rust Native color as a GPUI color.
pub fn color(value: Color) -> Hsla {
    Rgba {
        r: f32::from(value.red) / 255.,
        g: f32::from(value.green) / 255.,
        b: f32::from(value.blue) / 255.,
        a: f32::from(value.alpha) / 255.,
    }
    .into()
}

fn weight(value: TextWeight) -> FontWeight {
    match value {
        TextWeight::Normal => FontWeight::NORMAL,
        TextWeight::Medium => FontWeight::MEDIUM,
        TextWeight::Semibold => FontWeight::SEMIBOLD,
        TextWeight::Bold => FontWeight::BOLD,
    }
}

/// Apply the box properties a node's style sets: fill, padding, gap,
/// corners, border, and sizing. Unset fields leave `el` as it is.
fn boxed<E: Styled>(mut el: E, style: &Style) -> E {
    if let Some(value) = style.background {
        el = el.bg(color(value));
    }
    if let Some(value) = style.padding_top {
        el = el.pt(px(space(value)));
    }
    if let Some(value) = style.padding_bottom {
        el = el.pb(px(space(value)));
    }
    if let Some(value) = style.padding_start {
        el = el.pl(px(space(value)));
    }
    if let Some(value) = style.padding_end {
        el = el.pr(px(space(value)));
    }
    if let Some([top, end, bottom, start]) = style.padding_points {
        el = el
            .pt(px(f32::from(top)))
            .pr(px(f32::from(end)))
            .pb(px(f32::from(bottom)))
            .pl(px(f32::from(start)));
    }
    if let Some(value) = style.gap {
        el = el.gap(px(space(value)));
    }
    if let Some(value) = style.gap_points {
        el = el.gap(px(f32::from(value)));
    }
    if let Some(value) = style.radius {
        el = el.rounded(px(f32::from(value)));
    }
    if let Some(value) = style.border {
        el = el.border_1().border_color(color(value));
    }
    if style.intrinsic_width == Some(true) {
        el = el.flex_none();
    }
    if style.fill_height == Some(true) {
        el = el.flex_1();
    }
    if let Some(value) = style.min_height {
        el = el.min_h(px(f32::from(value)));
    }
    el
}

/// Apply the text properties a node's style sets.
fn texted<E: Styled>(mut el: E, style: &Style) -> E {
    if let Some(value) = style.foreground {
        el = el.text_color(color(value));
    }
    if let Some(value) = style.weight {
        el = el.font_weight(weight(value));
    }
    if let Some(value) = style.text_size {
        el = el.text_size(px(f32::from(value)));
    }
    if let Some(value) = style.line_height {
        el = el.line_height(px(f32::from(value)));
    }
    el
}

impl<V: 'static> Mount<'_, V> {
    fn activation(&self, key: &str) -> Activation {
        Activation {
            instance: self.instance.into(),
            revision: self.revision,
            node: key.into(),
        }
    }

    fn node<I: 'static>(&self, node: &Node<I>, cx: &mut Context<V>) -> AnyElement {
        let theme = self.theme;
        let style = &node.style;
        match &node.element {
            Element::Stack { axis, children } => {
                let mut stack = div().flex().gap(px(6.));
                stack = match axis {
                    Axis::Horizontal => stack.flex_row(),
                    Axis::Wrap => stack.flex_row().flex_wrap(),
                    Axis::Vertical => stack.flex_col(),
                };
                stack = texted(boxed(stack, style), style);
                for child in children {
                    stack = stack.child(self.node(child, cx));
                }
                stack.into_any_element()
            }
            Element::List { children, .. } => {
                let mut list = texted(boxed(div().flex().flex_col().gap(px(6.)), style), style);
                for child in children {
                    list = list.child(self.node(child, cx));
                }
                list.into_any_element()
            }
            Element::Text { value, role } => {
                let el = div()
                    .text_size(ui_rems(if *role == TextRole::Heading { 14. } else { 12. }))
                    .text_color(theme.text_muted)
                    .when(
                        *role == TextRole::Code || *role == TextRole::Terminal,
                        |el| el.font_family(theme.font_mono.clone()),
                    );
                texted(boxed(el, style), style)
                    .child(value.clone())
                    .into_any_element()
            }
            Element::RichText { runs, role } => {
                let el = div()
                    .text_size(ui_rems(if *role == TextRole::Heading { 14. } else { 12. }))
                    .text_color(theme.text_muted);
                texted(boxed(el, style), style)
                    .child(rich_text(runs))
                    .into_any_element()
            }
            Element::Markdown { blocks } => {
                let el = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .text_size(ui_rems(12.))
                    .text_color(theme.text);
                texted(boxed(el, style), style)
                    .children(blocks.iter().map(|block| markdown_block(block, theme)))
                    .into_any_element()
            }
            Element::Button { label, enabled, .. } => {
                let activation = self.activation(&node.key);
                let action = self.action.clone();
                let el = div()
                    .id(SharedString::from(node.key.clone()))
                    .debug_selector(|| format!("rn-{}", node.key))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded_md()
                    .text_size(ui_rems(12.))
                    .text_color(theme.text)
                    .bg(theme.surface);
                texted(boxed(el, style), style)
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
            Element::Choice {
                label,
                selected,
                enabled,
                children,
                ..
            } => {
                let activation = self.activation(&node.key);
                let action = self.action.clone();
                let el = div()
                    .id(SharedString::from(node.key.clone()))
                    .debug_selector(|| format!("rn-{}", node.key))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded_md()
                    .border_1()
                    .border_color(if *selected {
                        theme.accent.opacity(0.6)
                    } else {
                        theme.border
                    })
                    .when(*selected, |el| el.bg(theme.accent_wash))
                    .text_size(ui_rems(12.))
                    .text_color(theme.text);
                let mut el = texted(boxed(el, style), style)
                    .when(*enabled, |choice| {
                        choice
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.element_hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                action(this, activation.clone(), cx);
                            }))
                    })
                    .child(label.clone());
                for child in children {
                    el = el.child(self.node(child, cx));
                }
                el.into_any_element()
            }
            Element::Dialog { open, children, .. } => {
                if !*open {
                    return div().into_any_element();
                }
                let el = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .p(px(16.))
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border_strong)
                    .bg(theme.surface_card);
                let mut el = texted(boxed(el, style), style);
                for child in children {
                    el = el.child(self.node(child, cx));
                }
                el.into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}

fn rich_text(runs: &[RichRun]) -> StyledText {
    let mut text = String::new();
    let mut highlights = Vec::new();
    for run in runs {
        let start = text.len();
        text.push_str(&run.text);
        let mut style = HighlightStyle::default();
        if let Some(value) = run.foreground {
            style.color = Some(color(value));
        }
        if let Some(value) = run.background {
            style.background_color = Some(color(value));
        }
        if run.bold {
            style.font_weight = Some(FontWeight::BOLD);
        }
        if run.italic {
            style.font_style = Some(FontStyle::Italic);
        }
        if run.underline {
            style.underline = Some(UnderlineStyle {
                thickness: px(1.),
                ..UnderlineStyle::default()
            });
        }
        if run.strike {
            style.strikethrough = Some(StrikethroughStyle {
                thickness: px(1.),
                ..StrikethroughStyle::default()
            });
        }
        if run.dim {
            style.fade_out = Some(0.4);
        }
        highlights.push((start..text.len(), style));
    }
    StyledText::new(text).with_highlights(highlights)
}

fn spans_text(spans: &[Span], theme: &Theme) -> StyledText {
    let mut text = String::new();
    let mut highlights = Vec::new();
    for span in spans {
        let start = text.len();
        text.push_str(&span.text);
        let mut style = HighlightStyle::default();
        if span.bold {
            style.font_weight = Some(FontWeight::BOLD);
        }
        if span.italic {
            style.font_style = Some(FontStyle::Italic);
        }
        if span.strike {
            style.strikethrough = Some(StrikethroughStyle {
                thickness: px(1.),
                ..StrikethroughStyle::default()
            });
        }
        if span.code {
            style.color = Some(theme.code_text);
            style.background_color = Some(theme.code_wash);
        }
        if span.link.is_some() {
            style.color = Some(theme.accent);
        }
        highlights.push((start..text.len(), style));
    }
    StyledText::new(text).with_highlights(highlights)
}

fn markdown_block(block: &Block, theme: &Theme) -> AnyElement {
    match block {
        Block::Heading { spans, .. } => div()
            .font_weight(FontWeight::SEMIBOLD)
            .child(spans_text(spans, theme))
            .into_any_element(),
        Block::Paragraph { spans } => div().child(spans_text(spans, theme)).into_any_element(),
        Block::List {
            ordered,
            start,
            items,
        } => div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(items.iter().enumerate().map(|(ix, item)| {
                let marker = if *ordered {
                    format!("{}.", *start + ix as u64)
                } else {
                    match item.checked {
                        Some(true) => "☑".into(),
                        Some(false) => "☐".into(),
                        None => "•".into(),
                    }
                };
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.))
                    .child(div().flex_none().text_color(theme.text_muted).child(marker))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .children(item.blocks.iter().map(|b| markdown_block(b, theme))),
                    )
            }))
            .into_any_element(),
        Block::Code { text, .. } => div()
            .px(px(10.))
            .py(px(8.))
            .rounded_md()
            .bg(theme.surface)
            .font_family(theme.font_mono.clone())
            .text_size(px(theme.code_font_size))
            .text_color(theme.text)
            .child(text.clone())
            .into_any_element(),
        Block::Quote { blocks } => div()
            .pl(px(10.))
            .border_l_2()
            .border_color(theme.border_strong)
            .text_color(theme.text_muted)
            .flex()
            .flex_col()
            .gap(px(6.))
            .children(blocks.iter().map(|b| markdown_block(b, theme)))
            .into_any_element(),
        Block::Table { header, rows, .. } => div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .child(table_row(header, theme, true))
            .children(rows.iter().map(|row| table_row(row, theme, false)))
            .into_any_element(),
        Block::Rule => div().h(px(1.)).bg(theme.border).into_any_element(),
    }
}

fn table_row(cells: &[Vec<Span>], theme: &Theme, header: bool) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .when(header, |row| row.font_weight(FontWeight::SEMIBOLD))
        .children(cells.iter().map(|cell| {
            div()
                .flex_1()
                .px(px(8.))
                .py(px(4.))
                .child(spans_text(cell, theme))
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{color, mount, space};
    use crate::theme::Theme;
    use gpui::{Context, IntoElement, ParentElement as _, Render, Rgba, Styled as _, Window, div};
    use rust_native::style::{Color, Space, Style, TextWeight};
    use rust_native::{Axis, Element, Node, TextRole, ValidatedView, View};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn node(key: &str, style: Style, element: Element<u8>) -> Node<u8> {
        Node {
            key: key.into(),
            style,
            element,
        }
    }

    fn card() -> ValidatedView<u8> {
        let button = |key: &str, label: &str, intent| {
            node(
                key,
                Style {
                    intrinsic_width: Some(true),
                    ..Style::default()
                },
                Element::Button {
                    label: label.into(),
                    enabled: true,
                    icon: None,
                    shortcut: None,
                    intent,
                },
            )
        };
        let root = node(
            "card",
            Style {
                background: Some(Color::rgb(14, 14, 14)),
                padding_top: Some(Space::Md),
                gap: Some(Space::Xs),
                ..Style::default()
            },
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    node(
                        "title",
                        Style {
                            weight: Some(TextWeight::Bold),
                            ..Style::default()
                        },
                        Element::Text {
                            value: "Coder asks to go ahead".into(),
                            role: TextRole::Body,
                        },
                    ),
                    node(
                        "prompt",
                        Style::default(),
                        Element::Markdown {
                            blocks: rust_native::markdown::parse(
                                "Run `cargo test`?\n\n```\ncargo test -p ui\n```",
                            ),
                        },
                    ),
                    node(
                        "options",
                        Style::default(),
                        Element::Stack {
                            axis: Axis::Horizontal,
                            children: vec![
                                button("approve", "Allow once", 1),
                                button("deny", "Deny", 2),
                            ],
                        },
                    ),
                ],
            },
        );
        View::new("card", 7, root).validate().unwrap()
    }

    struct Host {
        view: ValidatedView<u8>,
        pressed: Rc<RefCell<Vec<u8>>>,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let view = self.view.clone();
            let pressed = self.pressed.clone();
            div().size_full().child(mount(
                &self.view,
                move |_: &mut Self, activation, _| {
                    if let Ok(intent) = view.activate(&activation) {
                        pressed.borrow_mut().push(*intent);
                    }
                },
                cx,
            ))
        }
    }

    #[test]
    fn spaces_and_colors_match_the_other_adapters() {
        assert_eq!(
            [Space::None, Space::Xs, Space::Sm, Space::Md, Space::Lg].map(space),
            [0., 4., 8., 16., 24.]
        );
        let red = Rgba::from(color(Color {
            red: 255,
            green: 0,
            blue: 0,
            alpha: 51,
        }));
        assert!((red.r - 1.).abs() < 1e-3 && red.g.abs() < 1e-3 && (red.a - 0.2).abs() < 1e-3);
    }

    #[gpui::test]
    fn a_card_draws_and_its_buttons_activate_their_intents(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::dark()));
        let pressed = Rc::new(RefCell::new(Vec::new()));
        let host_pressed = pressed.clone();
        let (_host, cx) = cx.add_window_view(|_, _| Host {
            view: card(),
            pressed: host_pressed,
        });
        cx.run_until_parked();
        let deny = cx
            .debug_bounds("rn-deny")
            .expect("the deny button is drawn");
        let approve = cx
            .debug_bounds("rn-approve")
            .expect("the approve button is drawn");
        assert!(
            approve.origin.x < deny.origin.x,
            "a horizontal stack lays out in order"
        );
        cx.simulate_click(deny.center(), gpui::Modifiers::default());
        cx.simulate_click(approve.center(), gpui::Modifiers::default());
        assert_eq!(*pressed.borrow(), vec![2, 1]);
    }
}
