use gpui::*;
use std::borrow::Cow;

struct CommanderView;

impl CommanderView {
    fn new(_cx: &mut ViewContext<Self>) -> Self {
        Self
    }
}

impl Render for CommanderView {
    fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .bg(rgb(0x000000))
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(px(48.0))
                    .font_family("Berkeley Mono")
                    .child("OpenAgents")
            )
    }
}

fn main() {
    App::new().run(|cx: &mut AppContext| {
        // Load Berkeley Mono fonts
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Regular.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Bold.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Italic.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-BoldItalic.ttf")),
            ])
            .unwrap();

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("OpenAgents Commander".into()),
                    ..Default::default()
                }),
                focus: true,
                show: true,
                ..Default::default()
            },
            |cx| cx.new_view(|cx| CommanderView::new(cx)),
        )
        .unwrap();

        cx.activate(true);
    });
}
