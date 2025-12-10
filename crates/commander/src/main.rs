mod text_input;

use fm_bridge::FMClient;
use gpui::*;
use std::borrow::Cow;
use std::sync::Arc;
use text_input::TextInput;

struct CommanderView {
    input: Entity<TextInput>,
    fm_client: Arc<FMClient>,
}

impl CommanderView {
    fn new(cx: &mut Context<Self>) -> Self {
        let fm_client = Arc::new(FMClient::new());
        let fm_client_clone = fm_client.clone();

        let input = cx.new(|cx| {
            TextInput::new("Message OpenAgents", cx)
                .on_submit(move |text, _cx| {
                    let client = fm_client_clone.clone();
                    let prompt = text.to_string();

                    // Spawn async task to call FM API
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(async {
                            println!("Sending to FM API: {}", prompt);
                            match client.complete(&prompt, None).await {
                                Ok(response) => {
                                    if let Some(choice) = response.choices.first() {
                                        println!("FM Response: {}", choice.message.content);
                                    }
                                }
                                Err(e) => {
                                    println!("FM API Error: {:?}", e);
                                }
                            }
                        });
                    });
                })
        });

        Self { input, fm_client }
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

        let window = cx.open_window(
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
            |window, cx| {
                let view = cx.new(|cx| CommanderView::new(cx));
                // Focus the input
                let focus_handle = view.read(cx).input.focus_handle(cx);
                window.focus(&focus_handle);
                view
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
