use gpui::*;
use std::borrow::Cow;

struct CommanderView {
    message: SharedString,
}

impl Render for CommanderView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x000000))
            .justify_center()
            .items_center()
            .child(
                div()
                    .w(px(600.0))
                    .h(px(44.0))
                    .bg(hsla(0., 0., 1., 0.05))
                    .border_1()
                    .border_color(hsla(0., 0., 1., 0.1))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .text_color(rgb(0xffffff))
                    .font_family("Berkeley Mono")
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .child(format!("{}", &self.message))
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
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
            |_, cx| {
                cx.new(|_| CommanderView {
                    message: "OpenAgents Commander".into(),
                })
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
