mod text_input;

use gpui::*;
use std::borrow::Cow;
use text_input::TextInput;

struct CommanderView {
    input: Entity<TextInput>,
}

impl CommanderView {
    fn new(cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new("Message OpenAgents", cx)
                .on_submit(|text, _cx| {
                    println!("Submitted: {}", text);
                })
        });

        Self { input }
    }
}

impl Render for CommanderView {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x000000))
            .justify_end()
            .items_center()
            .pb(px(40.0))
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
                    .child(self.input.clone())
            )
    }
}

impl Focusable for CommanderView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        // Load Berkeley Mono fonts
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Regular.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Bold.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Italic.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-BoldItalic.ttf").as_slice()),
            ])
            .unwrap();

        // Bind keyboard shortcuts
        cx.bind_keys([
            KeyBinding::new("enter", text_input::Submit, None),
            KeyBinding::new("cmd-a", text_input::SelectAll, None),
            KeyBinding::new("cmd-x", text_input::Cut, None),
            KeyBinding::new("cmd-c", text_input::Copy, None),
            KeyBinding::new("cmd-v", text_input::Paste, None),
            KeyBinding::new("backspace", text_input::Backspace, None),
            KeyBinding::new("delete", text_input::Delete, None),
            KeyBinding::new("left", text_input::Left, None),
            KeyBinding::new("right", text_input::Right, None),
            KeyBinding::new("home", text_input::Home, None),
            KeyBinding::new("end", text_input::End, None),
        ]);

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
            |_, cx| cx.new(|cx| CommanderView::new(cx)),
        )
        .unwrap();

        cx.activate(true);
    });
}
