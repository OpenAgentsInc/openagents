//! Computers in the account registry.
use super::*;

impl ChatApp {
    pub(super) fn render_computers(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div()
            .flex_1()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child("Computers")
            .child(
                button("computers-back", "Back to chat", false).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.show_computers = false;
                        this.earning = None;
                        cx.notify();
                    },
                )),
            );
        if let Some(registry) = &self.registry {
            for device in registry.read(|r| r.devices()) {
                let online = registry.read(|r| r.online(&device, now_ms()));
                let id = device.id.clone();
                let rename_id = id.clone();
                let name = device.name.clone();
                let mut row = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(format!(
                        "{} · {} · {}",
                        device.name,
                        device.platform,
                        if online { "Online" } else { "Offline" }
                    ))
                    .child(footer_text(
                        device
                            .last_seen_at
                            .map(|at| format!("Last seen {}", at.format("%Y-%m-%d %H:%M UTC")))
                            .unwrap_or_else(|| "Never seen".into()),
                    ));
                if let Some((_, field)) = self
                    .rename
                    .as_ref()
                    .filter(|(key, _)| key == &format!("device:{id}"))
                {
                    row = row.child(field.clone());
                } else {
                    row = row.child(
                        button(
                            SharedString::from(format!("rename-device-{id}")),
                            "Rename",
                            false,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.rename_computer(rename_id.clone(), name.clone(), window, cx);
                            },
                        )),
                    );
                }
                if online {
                    let earn_id = id.clone();
                    row = row.child(
                        button(
                            SharedString::from(format!("earn-device-{id}")),
                            "Earn",
                            false,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_earn(earn_id.clone(), cx);
                        })),
                    );
                }
                row = row.children(self.render_earn_card(&id, cx));
                row = row.child(
                    button(
                        SharedString::from(format!("forget-device-{id}")),
                        "Revoke this browser's access",
                        false,
                    )
                    .on_click(cx.listener(move |_, _, _, _| {
                        let id = id.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            let _ = openagents_web_client::computer::computer_forget(id).await;
                        });
                    })),
                );
                page = page.child(row);
            }
        }
        page.into_any_element()
    }

    fn rename_computer(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field = cx.new(|cx| {
            let mut field = TextInput::new("Computer name", cx);
            field.set_text(name, cx);
            field
        });
        let key = format!("device:{id}");
        cx.subscribe(&field, move |this, _, event: &Submitted, cx| {
            this.registry_write(cx, |r| r.rename_device(&id, &event.0));
            this.rename = None;
            cx.notify();
        })
        .detach();
        cx.subscribe(&field, |this, _, _: &Cancelled, cx| {
            this.rename = None;
            cx.notify();
        })
        .detach();
        window.focus(&field.focus_handle(cx), cx);
        self.rename = Some((key, field));
        cx.notify();
    }
}
