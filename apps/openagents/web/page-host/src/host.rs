use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, AppContext, Application, AsyncApp, ClickEvent, ClipboardItem, Context, Entity,
    FocusHandle, Focusable as _, IntoElement, ParentElement, Render, Styled, WeakEntity, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use openagents_chat_view::rust_native_gpui;
use openagents_chat_view::settings_forms::{Intent, Paints};
use openagents_chat_view::theme::Theme;
use openagents_gpui_web::{FetchHttpClient, WebFonts, WebPlatform};
use openagents_web_chat_view::composer::{Cancelled, Submitted, TextInput};
use openagents_web_chat_view::{assets, theme};
use openagents_web_client::store;
use openagents_web_client::web::Client;
use rust_native::Node;

/// A text the page asks for: what it's for, its words, and its first value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    /// The page's own name for it, handed back with the text.
    pub purpose: String,
    pub label: String,
    pub placeholder: String,
    pub value: String,
}

/// One page of the GPUI app over the shared forms.
pub trait Page: Sized + 'static {
    /// The module's name: its box, its events, and its address (`?page=`).
    const NAME: &'static str;
    fn new() -> Self;
    /// The header's title.
    fn title(&self) -> String;
    fn view(&self, paints: &Paints) -> Node<Intent>;
    /// Show `path` (after the module's name: `""`, or `ID`, or `ID/more`).
    fn open(host: &mut Host<Self>, path: &str, cx: &mut Context<Host<Self>>);
    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Context<Host<Self>>);
    /// The text the person entered for `prompt`.
    fn submit(
        _host: &mut Host<Self>,
        _prompt: &Prompt,
        _text: String,
        _cx: &mut Context<Host<Self>>,
    ) {
    }
}

pub struct Host<P: Page> {
    pub page: P,
    pub client: Client,
    pub notice: Option<String>,
    revision: u64,
    focus: FocusHandle,
    field: Option<(Prompt, Entity<TextInput>)>,
    focus_field: bool,
}

impl<P: Page> Host<P> {
    fn new(path: &str, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            page: P::new(),
            client: Client::from_page(),
            notice: None,
            revision: 1,
            focus: cx.focus_handle(),
            field: None,
            focus_field: false,
        };
        P::open(&mut this, path, cx);
        this
    }

    /// Draw again: the page's state changed.
    pub fn changed(&mut self, cx: &mut Context<Self>) {
        self.revision += 1;
        cx.notify();
    }

    /// Say `line` above the page; `None` clears it.
    pub fn say(&mut self, line: Option<String>, cx: &mut Context<Self>) {
        self.notice = line;
        self.changed(cx);
    }

    /// Keep the address at `?page=NAME/path`.
    pub fn moved(&self, path: &str) {
        let full = if path.is_empty() {
            P::NAME.to_string()
        } else {
            format!("{}/{path}", P::NAME)
        };
        store::signal("oa-page-path", &full);
    }

    pub fn copy(&mut self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.say(Some("Copied.".into()), cx);
    }

    /// Leave the canvas for another page of the site.
    pub fn go(&self, url: &str) {
        if let Some(window) = web_sys::window() {
            let _ = window.location().assign(url);
        }
    }

    /// Ask for a text; the page's `submit` receives it.
    pub fn ask(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            let mut input = TextInput::new(prompt.placeholder.clone(), cx);
            input.set_text(prompt.value.clone(), cx);
            input
        });
        cx.subscribe(&input, |this: &mut Self, _, event: &Submitted, cx| {
            if let Some((prompt, _)) = this.field.take() {
                P::submit(this, &prompt, event.0.clone(), cx);
            }
            this.changed(cx);
        })
        .detach();
        cx.subscribe(&input, |this: &mut Self, _, _: &Cancelled, cx| {
            this.field = None;
            this.changed(cx);
        })
        .detach();
        self.field = Some((prompt, input));
        self.focus_field = true;
        self.changed(cx);
    }

    /// Stop asking.
    pub fn unask(&mut self, cx: &mut Context<Self>) {
        self.field = None;
        self.changed(cx);
    }

    fn submit_field(&mut self, cx: &mut Context<Self>) {
        if let Some((prompt, input)) = self.field.take() {
            let text = input.update(cx, |input, cx| input.take(cx));
            P::submit(self, &prompt, text, cx);
        }
        self.changed(cx);
    }
}

/// The forms' colors from the theme the page draws with.
fn paints(theme: &Theme) -> Paints {
    let color = |c: gpui::Hsla| {
        let rgba = c.to_rgb();
        rust_native::style::Color {
            red: (rgba.r * 255.).round() as u8,
            green: (rgba.g * 255.).round() as u8,
            blue: (rgba.b * 255.).round() as u8,
            alpha: (rgba.a * 255.).round() as u8,
        }
    };
    Paints {
        text: color(theme.text),
        muted: color(theme.text_muted),
        faint: color(theme.text_faint),
        card: color(theme.surface_card),
        border: color(theme.border),
        danger: color(theme.danger),
    }
}

/// The intent of the control `key` names in `root`.
fn find_intent(root: &Node<Intent>, key: &str) -> Option<Intent> {
    use rust_native::Element;
    if root.key == key {
        return match &root.element {
            Element::Button {
                intent,
                enabled: true,
                ..
            }
            | Element::Choice {
                intent,
                enabled: true,
                ..
            } => Some(intent.clone()),
            _ => None,
        };
    }
    match &root.element {
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Choice { children, .. }
        | Element::Dialog { children, .. } => children.iter().find_map(|c| find_intent(c, key)),
        _ => None,
    }
}

impl<P: Page> Render for Host<P> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_field)
            && let Some((_, input)) = &self.field
        {
            window.focus(&input.focus_handle(cx), cx);
        }
        let paints = paints(&theme::desktop());
        let root = self.page.view(&paints);
        let body = rust_native_gpui::mount_root(
            &root,
            P::NAME,
            self.revision,
            move |this: &mut Self, activation, cx| {
                let root = this.page.view(&paints);
                if let Some(intent) = find_intent(&root, &activation.node) {
                    this.notice = None;
                    P::intent(this, intent, cx);
                    this.changed(cx);
                }
            },
            cx,
        );
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .h(px(52.))
            .px(px(16.))
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .id("page-back")
                    .px(px(8.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_size(px(13.5))
                    .text_color(theme::text_muted())
                    .hover(|s| s.bg(theme::element_hover()).text_color(theme::text()))
                    .on_click(|_: &ClickEvent, _, _| store::signal("oa-page-close", P::NAME))
                    .child("\u{2190} Back to chat"),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(14.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(self.page.title()),
            );
        let notice = self.notice.clone().map(|line| {
            div()
                .id("page-notice")
                .mx(px(16.))
                .mt(px(10.))
                .px(px(12.))
                .py(px(8.))
                .rounded(px(10.))
                .border_1()
                .border_color(theme::border_strong())
                .bg(theme::surface_card())
                .text_size(px(13.))
                .cursor_pointer()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.say(None, cx)))
                .child(line)
        });
        let field: Option<AnyElement> = self.field.as_ref().map(|(prompt, input)| {
            div()
                .mx(px(16.))
                .mt(px(10.))
                .p(px(12.))
                .rounded(px(12.))
                .border_1()
                .border_color(theme::border_strong())
                .bg(theme::surface_card())
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme::text_muted())
                        .child(prompt.label.clone()),
                )
                .child(input.clone())
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.))
                        .child(
                            openagents_web_chat_view::labels::button("page-field-ok", "Save", true)
                                .on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.submit_field(cx)
                                    }),
                                ),
                        )
                        .child(
                            openagents_web_chat_view::labels::button(
                                "page-field-cancel",
                                "Cancel",
                                false,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.unask(cx))),
                        ),
                )
                .into_any_element()
        });
        let mut root = div()
            .id(gpui::SharedString::from(format!(
                "openagents-web-{}",
                P::NAME
            )))
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY));
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        root.child(header).children(notice).children(field).child(
            div()
                .id("page-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(div().w_full().max_w(px(820.)).mx_auto().child(body)),
        )
    }
}

thread_local! {
    static OPEN: RefCell<Option<(Rc<dyn Fn(String)>, AsyncApp)>> = const { RefCell::new(None) };
}

/// Starts page `P` in the page's box at `path`, with the page's fonts, one
/// `Uint8Array` per face.
pub fn mount<P: Page>(fonts: js_sys::Array, path: String) {
    openagents_gpui_web::init_logging();
    let faces: Vec<Cow<'static, [u8]>> = fonts
        .iter()
        .map(|face| Cow::Owned(js_sys::Uint8Array::new(&face).to_vec()))
        .collect();
    let platform = WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces,
        },
    );
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .with_http_client(Arc::new(FetchHttpClient::new()))
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            openagents_web_chat_view::composer::bind_keys(cx);
            let bounds = gpui::Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                move |_, cx| cx.new(|cx| Host::<P>::new(&path, cx)),
            );
            if let Ok(entity) = opened.and_then(|window| window.entity(cx)) {
                let weak: WeakEntity<Host<P>> = entity.downgrade();
                let async_cx = cx.to_async();
                let again = async_cx.clone();
                let show: Rc<dyn Fn(String)> = Rc::new(move |path: String| {
                    let _ = weak.update(&mut again.clone(), |host, cx| {
                        P::open(host, &path, cx);
                        host.changed(cx);
                    });
                });
                OPEN.with(|open| *open.borrow_mut() = Some((show, async_cx)));
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// Shows `path` again in the mounted page.
pub fn show(path: String) {
    let show = OPEN.with(|open| open.borrow().as_ref().map(|(show, _)| show.clone()));
    if let Some(show) = show {
        show(path);
    }
}
