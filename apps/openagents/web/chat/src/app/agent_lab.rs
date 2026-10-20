//! Agent Lab's frame, beside the chat's existing panels.
use super::*;

impl ChatApp {
    pub(super) fn render_agent_lab(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let wide = window.viewport_size().width > px(820.);
        let main = div().flex_1().min_w_0().h_full().p(px(8.)).child(
            div()
                .size_full()
                .flex()
                .flex_col()
                .rounded(px(14.))
                .border_1()
                .border_color(theme::border())
                .bg(theme::bg())
                .overflow_hidden()
                .child(
                    div()
                        .id("close-agent-lab")
                        .role(gpui::Role::Button)
                        .aria_label("Back to chat")
                        .tab_index(0)
                        .p(px(12.))
                        .cursor_pointer()
                        .child("Back to chat")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.lab_open = false;
                            route::set_title("Chat");
                            cx.notify();
                        }))
                        .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                this.lab_open = false;
                                route::set_title("Chat");
                                cx.notify();
                                cx.stop_propagation();
                            }
                        })),
                )
                .child(div().flex_1().min_h_0().child(self.agent_lab.clone())),
        );
        let mut root = div()
            .id("openagents-web-chat")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .bg(theme::surface())
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY));
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        let root = self.shortcut_actions(root, cx);
        if wide && self.settings.sidebar_hidden {
            root.child(main)
        } else if wide {
            root.child(self.render_sidebar(false, cx)).child(main)
        } else if self.sidebar_open {
            root.child(self.render_sidebar(true, cx))
        } else {
            root.child(main)
        }
    }
}
