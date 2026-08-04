#![cfg_attr(not(target_family = "wasm"), allow(dead_code, unused_imports))]

#[cfg(target_family = "wasm")]
mod web_app {
    use std::time::Duration;

    use futures::{StreamExt as _, channel::mpsc};
    use gpui::prelude::*;
    use gpui::{
        App, Bounds, Context, Font, FontWeight, InteractiveElement, Pixels, SharedString,
        StatefulInteractiveElement, Task, Window, WindowBounds, WindowOptions, div, px, rgb, size,
    };
    use immortal::client::{
        ConnectionState, ProjectActivity, ProjectActivityKind, ProjectClient, ProjectClientConfig,
        ProjectSnapshot,
    };
    use immortal::domain::OpenAgentsProject;
    use theme::{ActiveTheme as _, ThemeSettingsProvider, UiDensity};
    use ui::{
        Button, ButtonCommon as _, ButtonStyle, Clickable as _, Color, Divider, DividerColor,
        Label, LabelCommon as _, LabelSize, h_flex, v_flex,
    };
    use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
    use web_sys::{CloseEvent, Event, MessageEvent, WebSocket, console};
    use web_time::{SystemTime, UNIX_EPOCH};

    const RELAY_URL: &str = "wss://relay.openagents.com";
    const PROGRAM_AUTHORITY: &str =
        "e841147f262799821bbaa2930fcca982a575458f0e043e064a26ed8aba2046ed";
    const ORGANIZATION_REF: &str = "org-openagents";
    const PROJECT_REF: &str = "operation-diamond-hands";
    const SUBSCRIPTION_ID: &str = "dh-project-v1";
    const ENERGY_BLUE: u32 = 0x3a7bff;
    const ENERGY_CYAN: u32 = 0x4fd0ff;
    const SOFT_BLUE: u32 = 0x8fb6ff;
    const VOID: u32 = 0x05070d;
    const SURFACE: u32 = 0x0c0f13;
    const RAISED: u32 = 0x11161d;
    const BODY: u32 = 0xc9d2dd;
    const SECONDARY: u32 = 0xaeb9c6;
    const FAINT: u32 = 0x7e8a98;
    const HAIRLINE: u32 = 0x1d2530;

    struct WebThemeSettings {
        ui_font: Font,
        buffer_font: Font,
    }

    impl ThemeSettingsProvider for WebThemeSettings {
        fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
            &self.ui_font
        }

        fn buffer_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
            &self.buffer_font
        }

        fn ui_font_size(&self, _cx: &App) -> Pixels {
            px(14.)
        }

        fn buffer_font_size(&self, _cx: &App) -> Pixels {
            px(13.)
        }

        fn ui_density(&self, _cx: &App) -> UiDensity {
            UiDensity::Default
        }
    }

    // The exact UI-facing values from Omega's Aiur theme at the pinned commit.
    // Embedding them avoids pulling the native-only theme_settings filesystem
    // stack into the browser build.
    const AIUR_JSON: &str = r##"{
      "themes": [{
        "style": {
          "border": "#1f2b45ff",
          "border.variant": "#16203aff",
          "border.focused": "#60a5faff",
          "elevated_surface.background": "#141f36ff",
          "surface.background": "#0b1220ff",
          "background": "#0b1220ff",
          "panel.background": "#0b1220ff",
          "editor.background": "#05070dff",
          "element.background": "#141f36ff",
          "element.hover": "#8fb3ff14",
          "element.active": "#8fb3ff21",
          "element.selected": "#3b82f629",
          "ghost_element.hover": "#8fb3ff14",
          "ghost_element.selected": "#3b82f629",
          "text": "#eef3ffff",
          "text.muted": "#a9b1d6ff",
          "text.placeholder": "#8990adff",
          "text.disabled": "#55648aff",
          "text.accent": "#3b82f6ff",
          "icon": "#eef3ffff",
          "icon.muted": "#a9b1d6ff",
          "icon.accent": "#3b82f6ff",
          "status_bar.background": "#0b1220ff",
          "title_bar.background": "#0b1220ff",
          "toolbar.background": "#05070dff",
          "tab_bar.background": "#0b1220ff",
          "tab.active_background": "#05070dff",
          "tab.inactive_background": "#0b1220ff"
        }
      }]
    }"##;

    enum RelayInput {
        Opened,
        Text(String),
        Closed { code: u16, reason: String },
        Error(String),
    }

    struct BrowserRelay {
        socket: WebSocket,
        _open: Closure<dyn FnMut(Event)>,
        _message: Closure<dyn FnMut(MessageEvent)>,
        _close: Closure<dyn FnMut(CloseEvent)>,
        _error: Closure<dyn FnMut(Event)>,
    }

    impl BrowserRelay {
        fn connect(
            url: &str,
            request: String,
            sender: mpsc::UnboundedSender<RelayInput>,
        ) -> Result<Self, String> {
            let socket = WebSocket::new(url)
                .map_err(|error| format!("browser refused relay WebSocket: {error:?}"))?;

            let open_socket = socket.clone();
            let open_sender = sender.clone();
            let open = Closure::<dyn FnMut(Event)>::new(move |_event| {
                if let Err(error) = open_socket.send_with_str(&request) {
                    send_relay_input(
                        &open_sender,
                        RelayInput::Error(format!("failed to send Nostr REQ: {error:?}")),
                    );
                    return;
                }
                send_relay_input(&open_sender, RelayInput::Opened);
            });
            socket.set_onopen(Some(open.as_ref().unchecked_ref()));

            let message_sender = sender.clone();
            let message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                if let Some(text) = event.data().as_string() {
                    send_relay_input(&message_sender, RelayInput::Text(text));
                } else {
                    send_relay_input(
                        &message_sender,
                        RelayInput::Error("relay sent a non-text WebSocket frame".to_owned()),
                    );
                }
            });
            socket.set_onmessage(Some(message.as_ref().unchecked_ref()));

            let close_sender = sender.clone();
            let close = Closure::<dyn FnMut(CloseEvent)>::new(move |event: CloseEvent| {
                send_relay_input(
                    &close_sender,
                    RelayInput::Closed {
                        code: event.code(),
                        reason: event.reason(),
                    },
                );
            });
            socket.set_onclose(Some(close.as_ref().unchecked_ref()));

            let error_sender = sender;
            let error = Closure::<dyn FnMut(Event)>::new(move |_event| {
                send_relay_input(
                    &error_sender,
                    RelayInput::Error("browser reported a relay WebSocket error".to_owned()),
                );
            });
            socket.set_onerror(Some(error.as_ref().unchecked_ref()));

            Ok(Self {
                socket,
                _open: open,
                _message: message,
                _close: close,
                _error: error,
            })
        }
    }

    impl Drop for BrowserRelay {
        fn drop(&mut self) {
            self.socket.set_onopen(None);
            self.socket.set_onmessage(None);
            self.socket.set_onclose(None);
            self.socket.set_onerror(None);
            if matches!(
                self.socket.ready_state(),
                WebSocket::CONNECTING | WebSocket::OPEN
            ) && let Err(error) = self.socket.close()
            {
                console::warn_1(&JsValue::from_str(&format!(
                    "failed to close project relay socket: {error:?}"
                )));
            }
        }
    }

    fn send_relay_input(sender: &mpsc::UnboundedSender<RelayInput>, input: RelayInput) {
        if sender.unbounded_send(input).is_err() {
            console::warn_1(&JsValue::from_str(
                "project relay event arrived after the GPUI view closed",
            ));
        }
    }

    struct DiamondHands {
        client: Option<ProjectClient>,
        configuration_error: Option<String>,
        sender: mpsc::UnboundedSender<RelayInput>,
        relay: Option<BrowserRelay>,
        reconnect_attempt: u32,
        reconnect_task: Option<Task<()>>,
        transport_message: Option<String>,
        _incoming_task: Task<()>,
        _stale_task: Task<()>,
    }

    impl DiamondHands {
        fn new(cx: &mut Context<Self>) -> Self {
            let configured_client = ProjectClient::new(ProjectClientConfig {
                relay_url: RELAY_URL.to_owned(),
                pinned_authority: PROGRAM_AUTHORITY.to_owned(),
                organization_ref: ORGANIZATION_REF.to_owned(),
                project_ref: PROJECT_REF.to_owned(),
                subscription_id: SUBSCRIPTION_ID.to_owned(),
                max_events: 192,
                max_activity: 40,
            });
            let (client, configuration_error) = match configured_client {
                Ok(client) => (Some(client), None),
                Err(error) => {
                    console::error_1(&JsValue::from_str(&format!(
                        "invalid Operation Diamond Hands client configuration: {error}"
                    )));
                    (None, Some(error))
                }
            };
            let (sender, mut receiver) = mpsc::unbounded();

            let incoming_task = cx.spawn(async move |this, cx| {
                while let Some(input) = receiver.next().await {
                    match this.update(cx, |this, cx| this.handle_relay_input(input, cx)) {
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
            });
            let stale_task = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_secs(15))
                        .await;
                    match this.update(cx, |this, cx| {
                        if this
                            .client
                            .as_mut()
                            .is_some_and(|client| client.mark_stale(now_seconds(), 45))
                        {
                            cx.notify();
                        }
                    }) {
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
            });

            let mut view = Self {
                client,
                configuration_error,
                sender,
                relay: None,
                reconnect_attempt: 0,
                reconnect_task: None,
                transport_message: None,
                _incoming_task: incoming_task,
                _stale_task: stale_task,
            };
            view.open_relay(cx);
            view
        }

        fn open_relay(&mut self, cx: &mut Context<Self>) {
            let Some(client) = self.client.as_mut() else {
                cx.notify();
                return;
            };
            client.begin_connect();
            self.transport_message = None;
            let request = client.subscription_request();
            match BrowserRelay::connect(RELAY_URL, request, self.sender.clone()) {
                Ok(relay) => self.relay = Some(relay),
                Err(error) => {
                    self.relay = None;
                    if let Some(client) = self.client.as_mut() {
                        client.disconnected();
                    }
                    self.transport_message = Some(error);
                    self.schedule_reconnect(cx);
                }
            }
            cx.notify();
        }

        fn schedule_reconnect(&mut self, cx: &mut Context<Self>) {
            if self.reconnect_task.is_some() {
                return;
            }
            self.reconnect_attempt = self.reconnect_attempt.saturating_add(1);
            let exponent = self.reconnect_attempt.saturating_sub(1).min(5);
            let seconds = 2_u64.saturating_pow(exponent).min(30);
            self.reconnect_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_secs(seconds))
                    .await;
                if let Ok(()) = this.update(cx, |this, cx| {
                    this.reconnect_task = None;
                    this.open_relay(cx);
                }) {}
            }));
        }

        fn handle_relay_input(&mut self, input: RelayInput, cx: &mut Context<Self>) {
            match input {
                RelayInput::Opened => {
                    self.reconnect_attempt = 0;
                    self.transport_message = None;
                    if let Some(client) = self.client.as_mut() {
                        client.opened(now_seconds());
                    }
                }
                RelayInput::Text(text) => {
                    if let Some(Err(error)) = self
                        .client
                        .as_mut()
                        .map(|client| client.ingest_text(&text, now_seconds()))
                    {
                        self.transport_message = Some(format!("Excluded relay frame: {error}"));
                    }
                }
                RelayInput::Closed { code, reason } => {
                    self.relay = None;
                    if let Some(client) = self.client.as_mut() {
                        client.disconnected();
                    }
                    self.transport_message = Some(if reason.is_empty() {
                        format!("Relay connection closed ({code})")
                    } else {
                        format!("Relay connection closed ({code}): {reason}")
                    });
                    self.schedule_reconnect(cx);
                }
                RelayInput::Error(message) => {
                    self.transport_message = Some(message);
                }
            }
            cx.notify();
        }

        fn retry_now(&mut self, cx: &mut Context<Self>) {
            self.reconnect_task = None;
            self.reconnect_attempt = 0;
            self.relay = None;
            self.open_relay(cx);
        }

        fn connection_label(&self) -> &'static str {
            match self.connection_state() {
                ConnectionState::Connecting => "CONNECTING",
                ConnectionState::Snapshotting => "SYNCING SNAPSHOT",
                ConnectionState::Live => "LIVE",
                ConnectionState::Reconnecting => "RECONNECTING",
                ConnectionState::Stale => "STALE",
                ConnectionState::Unavailable => "UNAVAILABLE",
            }
        }

        fn connection_color(&self) -> u32 {
            match self.connection_state() {
                ConnectionState::Live => ENERGY_CYAN,
                ConnectionState::Connecting
                | ConnectionState::Snapshotting
                | ConnectionState::Reconnecting => SOFT_BLUE,
                ConnectionState::Stale => 0xf2c572,
                ConnectionState::Unavailable => 0xf08a8a,
            }
        }

        fn connection_state(&self) -> ConnectionState {
            self.client
                .as_ref()
                .map_or(ConnectionState::Unavailable, ProjectClient::state)
        }

        fn render_header(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            h_flex()
                .w_full()
                .justify_between()
                .gap_4()
                .flex_wrap()
                .child(
                    Button::new("openagents-home", "OPENAGENTS")
                        .style(ButtonStyle::Subtle)
                        .on_click(|_event, _window, cx| cx.open_url("https://openagents.com/")),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(self.connection_color()))
                        .child(
                            div()
                                .size(px(7.))
                                .rounded_full()
                                .bg(rgb(self.connection_color())),
                        )
                        .child(
                            Label::new(self.connection_label())
                                .size(LabelSize::XSmall)
                                .weight(FontWeight::SEMIBOLD)
                                .color(Color::Custom(rgb(self.connection_color()).into())),
                        ),
                )
        }

        fn render_intro(&self) -> impl IntoElement {
            v_flex()
                .gap_4()
                .pt_12()
                .pb_10()
                .child(
                    Label::new("OPERATION DIAMOND HANDS")
                        .size(LabelSize::XSmall)
                        .weight(FontWeight::SEMIBOLD)
                        .color(Color::Custom(rgb(SOFT_BLUE).into())),
                )
                .child(
                    div()
                        .text_size(px(54.))
                        .line_height(px(58.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child("Hardening Bitcoin OSS in public."),
                )
                .child(
                    div()
                        .max_w(px(760.))
                        .text_size(px(17.))
                        .line_height(px(29.))
                        .text_color(rgb(BODY))
                        .child(
                            "A signed project record for coordinated, reproducible security work. This page is a freshness-aware projection from the OpenAgents Nostr relay.",
                        ),
                )
                .child(
                    div()
                        .w(px(180.))
                        .h(px(1.))
                        .bg(rgb(ENERGY_BLUE)),
                )
        }

        fn render_waiting(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let message = if self.configuration_error.is_some() {
                "The project reader configuration is invalid."
            } else {
                match self.connection_state() {
                    ConnectionState::Connecting => "Opening the relay WebSocket…",
                    ConnectionState::Snapshotting => "Verifying signed records until EOSE…",
                    ConnectionState::Reconnecting => {
                        "Reconnecting. The last complete snapshot remains visible when available."
                    }
                    ConnectionState::Stale => "The relay stream has not advanced recently.",
                    ConnectionState::Unavailable => "The signed project stream is unavailable.",
                    ConnectionState::Live => "No Project record exists at the pinned coordinate.",
                }
            };
            v_flex()
                .gap_4()
                .py_12()
                .border_t_1()
                .border_b_1()
                .border_color(rgb(HAIRLINE))
                .child(
                    Label::new(message)
                        .size(LabelSize::Large)
                        .color(Color::Custom(rgb(BODY).into())),
                )
                .when_some(self.transport_message.clone(), |view, detail| {
                    view.child(
                        Label::new(detail)
                            .size(LabelSize::Small)
                            .color(Color::Custom(rgb(SECONDARY).into())),
                    )
                })
                .when_some(self.configuration_error.clone(), |view, detail| {
                    view.child(
                        Label::new(format!("Configuration error: {detail}"))
                            .size(LabelSize::Small)
                            .color(Color::Warning),
                    )
                })
                .when(
                    matches!(
                        self.connection_state(),
                        ConnectionState::Unavailable
                            | ConnectionState::Stale
                            | ConnectionState::Reconnecting
                    ),
                    |view| {
                        view.child(
                            Button::new("retry-relay", "Reconnect now")
                                .style(ButtonStyle::Outlined)
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.retry_now(cx);
                                })),
                        )
                    },
                )
        }

        fn render_project(
            &self,
            snapshot: &ProjectSnapshot,
            project: &OpenAgentsProject,
        ) -> impl IntoElement {
            let organization_name = snapshot
                .organization
                .as_ref()
                .map_or(ORGANIZATION_REF, |organization| organization.name.as_str());
            let status_name = snapshot
                .status
                .as_ref()
                .map_or("Status record unavailable", |status| status.name.as_str());
            let status_category = snapshot
                .status
                .as_ref()
                .map_or("unknown", |status| status.category.as_str());
            let progress = project.progress.map_or_else(
                || "Not projected".to_owned(),
                |(done, total)| format!("{done} of {total} work records"),
            );

            v_flex()
                .gap_8()
                .child(
                    h_flex()
                        .items_start()
                        .justify_between()
                        .gap_6()
                        .flex_wrap()
                        .py_7()
                        .border_t_1()
                        .border_b_1()
                        .border_color(rgb(HAIRLINE))
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    Label::new(project.name.clone())
                                        .size(LabelSize::Large)
                                        .weight(FontWeight::BOLD)
                                        .color(Color::Custom(rgb(0xffffff).into())),
                                )
                                .child(
                                    Label::new(format!("Published by {organization_name}"))
                                        .size(LabelSize::Small)
                                        .color(Color::Custom(rgb(SECONDARY).into())),
                                ),
                        )
                        .child(
                            v_flex()
                                .items_end()
                                .gap_1()
                                .child(
                                    Label::new(status_name.to_owned())
                                        .weight(FontWeight::SEMIBOLD)
                                        .color(Color::Custom(rgb(SOFT_BLUE).into())),
                                )
                                .child(
                                    Label::new(status_category.to_owned())
                                        .size(LabelSize::XSmall)
                                        .color(Color::Custom(rgb(SECONDARY).into())),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .items_start()
                        .gap_8()
                        .flex_wrap()
                        .child(fact("PROJECTED PROGRESS", progress))
                        .child(fact("PROJECT REVISION", format!("r{}", project.revision)))
                        .child(fact(
                            "PROJECT PUBLISHED",
                            format_timestamp(project.published_at),
                        ))
                        .child(fact(
                            "SNAPSHOT COMPLETE",
                            format_timestamp(snapshot.eose_at),
                        )),
                )
                .when_some(snapshot.latest_update.as_ref(), |view, update| {
                    view.child(
                        v_flex()
                            .gap_4()
                            .p_6()
                            .rounded_lg()
                            .bg(rgb(RAISED))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .gap_4()
                                    .flex_wrap()
                                    .child(
                                        Label::new("Latest authored update")
                                            .weight(FontWeight::SEMIBOLD)
                                            .color(Color::Custom(rgb(0xffffff).into())),
                                    )
                                    .child(
                                        Label::new(format!(
                                            "{} · {}",
                                            update.health.replace('_', " "),
                                            format_timestamp(update.published_at)
                                        ))
                                        .size(LabelSize::Small)
                                        .color(Color::Custom(rgb(SOFT_BLUE).into())),
                                    ),
                            )
                            .child(
                                div()
                                    .max_w(px(760.))
                                    .text_size(px(16.))
                                    .line_height(px(27.))
                                    .text_color(rgb(BODY))
                                    .child(update.body.clone()),
                            )
                            .child(
                                Label::new(format!(
                                    "Authored by {} · update r{}",
                                    shorten(&update.author_pubkey),
                                    update.revision
                                ))
                                .size(LabelSize::XSmall)
                                .color(Color::Custom(rgb(FAINT).into())),
                            ),
                    )
                })
                .child(
                    h_flex()
                        .items_start()
                        .gap_8()
                        .flex_wrap()
                        .child(fact(
                            "OWNERS",
                            list_or_none(project.owner_pubkeys.iter().map(|value| shorten(value))),
                        ))
                        .child(fact(
                            "LEADS",
                            list_or_none(project.lead_pubkeys.iter().map(|value| shorten(value))),
                        ))
                        .child(fact(
                            "TEAMS",
                            list_or_none(project.team_refs.iter().cloned()),
                        ))
                        .child(fact(
                            "PUBLIC REFS",
                            list_or_none(project.linked_addresses.iter().map(
                                |(marker, address)| format!("{marker}: {}", shorten(address)),
                            )),
                        )),
                )
        }

        fn render_activity(&self, snapshot: &ProjectSnapshot) -> impl IntoElement {
            let activity = if snapshot.recent_activity.is_empty() {
                v_flex().py_8().child(
                    Label::new("No public project activity arrived in this snapshot.")
                        .color(Color::Custom(rgb(SECONDARY).into())),
                )
            } else {
                snapshot
                    .recent_activity
                    .iter()
                    .fold(v_flex(), |list, activity| {
                        list.child(activity_row(activity))
                    })
            };
            v_flex()
                .gap_4()
                .pt_8()
                .child(
                    h_flex()
                        .justify_between()
                        .gap_4()
                        .child(
                            Label::new("Recent relay activity")
                                .size(LabelSize::Large)
                                .weight(FontWeight::SEMIBOLD)
                                .color(Color::Custom(rgb(0xffffff).into())),
                        )
                        .child(
                            Label::new(format!("{} events", snapshot.recent_activity.len()))
                                .size(LabelSize::Small)
                                .color(Color::Custom(rgb(FAINT).into())),
                        ),
                )
                .child(Divider::horizontal().color(DividerColor::BorderVariant))
                .child(activity)
        }

        fn render_diagnostics(&self) -> impl IntoElement {
            let diagnostics = self
                .client
                .as_ref()
                .map_or(&[][..], ProjectClient::diagnostics);
            v_flex().when(!diagnostics.is_empty(), |view| {
                view.mt_8()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(SURFACE))
                    .border_1()
                    .border_color(rgb(HAIRLINE))
                    .child(
                        Label::new(format!(
                            "{} invalid signed record{} excluded",
                            diagnostics.len(),
                            if diagnostics.len() == 1 { "" } else { "s" }
                        ))
                        .size(LabelSize::Small)
                        .color(Color::Warning),
                    )
            })
        }

        fn render_source(&self, snapshot: Option<&ProjectSnapshot>) -> impl IntoElement {
            let last_event = snapshot
                .and_then(|snapshot| snapshot.last_event_at)
                .map_or("No accepted event yet".to_owned(), format_timestamp);
            v_flex()
                .gap_3()
                .mt_12()
                .pt_6()
                .pb_10()
                .border_t_1()
                .border_color(rgb(HAIRLINE))
                .child(
                    h_flex()
                        .justify_between()
                        .gap_4()
                        .flex_wrap()
                        .child(
                            Label::new("SOURCE  wss://relay.openagents.com")
                                .size(LabelSize::XSmall)
                                .color(Color::Custom(rgb(SOFT_BLUE).into())),
                        )
                        .child(
                            Label::new(format!("LAST ACCEPTED  {last_event}"))
                                .size(LabelSize::XSmall)
                                .color(Color::Custom(rgb(FAINT).into())),
                        ),
                )
                .child(
                    Label::new(format!("PINNED AUTHORITY  {}", shorten(PROGRAM_AUTHORITY)))
                        .size(LabelSize::XSmall)
                        .color(Color::Custom(rgb(FAINT).into())),
                )
        }
    }

    impl Render for DiamondHands {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let _aiur_theme = cx.theme();
            let snapshot = self.client.as_ref().and_then(ProjectClient::snapshot);
            v_flex()
                .id("dh-project-scroll")
                .size_full()
                .overflow_y_scroll()
                .bg(rgb(VOID))
                .text_color(rgb(BODY))
                .child(
                    v_flex()
                        .w_full()
                        .max_w(px(1080.))
                        .mx_auto()
                        .px_8()
                        .pt_6()
                        .child(self.render_header(cx))
                        .child(self.render_intro())
                        .when_some(
                            snapshot.and_then(|snapshot| {
                                snapshot.project.as_ref().map(|project| (snapshot, project))
                            }),
                            |view, (snapshot, project)| {
                                view.child(self.render_project(snapshot, project))
                            },
                        )
                        .when(
                            snapshot.is_none_or(|snapshot| snapshot.project.is_none()),
                            |view| view.child(self.render_waiting(cx)),
                        )
                        .when_some(snapshot, |view, snapshot| {
                            view.child(self.render_activity(snapshot))
                        })
                        .child(self.render_diagnostics())
                        .child(self.render_source(snapshot)),
                )
        }
    }

    fn fact(label: &'static str, value: String) -> impl IntoElement {
        v_flex()
            .w(px(220.))
            .gap_2()
            .child(
                Label::new(label)
                    .size(LabelSize::XSmall)
                    .weight(FontWeight::SEMIBOLD)
                    .color(Color::Custom(rgb(FAINT).into())),
            )
            .child(
                Label::new(value)
                    .size(LabelSize::Small)
                    .color(Color::Custom(rgb(BODY).into())),
            )
    }

    fn activity_row(activity: &ProjectActivity) -> impl IntoElement {
        let (label, summary): (&str, SharedString) = match &activity.kind {
            ProjectActivityKind::ProjectUpdate(update) => (
                "PROJECT UPDATE",
                format!("{} · {}", update.health.replace('_', " "), update.body).into(),
            ),
            ProjectActivityKind::Unknown { kind } => (
                "UNKNOWN VERIFIED EVENT",
                format!("Kind {kind}; retained without claiming project meaning").into(),
            ),
        };
        v_flex()
            .gap_2()
            .py_5()
            .border_b_1()
            .border_color(rgb(HAIRLINE))
            .child(
                h_flex()
                    .justify_between()
                    .gap_4()
                    .flex_wrap()
                    .child(
                        Label::new(label)
                            .size(LabelSize::XSmall)
                            .weight(FontWeight::SEMIBOLD)
                            .color(Color::Custom(rgb(SOFT_BLUE).into())),
                    )
                    .child(
                        Label::new(format_timestamp(activity.event.created_at))
                            .size(LabelSize::XSmall)
                            .color(Color::Custom(rgb(FAINT).into())),
                    ),
            )
            .child(
                Label::new(summary)
                    .size(LabelSize::Small)
                    .color(Color::Custom(rgb(BODY).into())),
            )
            .child(
                Label::new(format!(
                    "kind {} · author {} · event {}",
                    activity.event.kind,
                    shorten(&activity.event.pubkey),
                    shorten(&activity.event.id)
                ))
                .size(LabelSize::XSmall)
                .color(Color::Custom(rgb(FAINT).into())),
            )
    }

    fn list_or_none(values: impl Iterator<Item = String>) -> String {
        let values = values.collect::<Vec<_>>();
        if values.is_empty() {
            "None declared".to_owned()
        } else {
            values.join(", ")
        }
    }

    fn shorten(value: &str) -> String {
        if value.len() <= 18 {
            value.to_owned()
        } else {
            format!("{}…{}", &value[..10], &value[value.len() - 6..])
        }
    }

    fn now_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs())
    }

    fn format_timestamp(timestamp: u64) -> String {
        let days = timestamp / 86_400;
        let seconds = timestamp % 86_400;
        let (year, month, day) = civil_from_days(days as i64);
        format!(
            "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
            seconds / 3_600,
            (seconds % 3_600) / 60
        )
    }

    fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
        let days = days_since_epoch + 719_468;
        let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
        let day_of_era = days - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let mut year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_prime = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
        let month = month_prime + if month_prime < 10 { 3 } else { -9 };
        year += i64::from(month <= 2);
        (year, month as u32, day as u32)
    }

    fn hex_to_hsla(hex: &str) -> Option<gpui::Hsla> {
        let raw = hex.trim_start_matches('#');
        let (red, green, blue, alpha) = match raw.len() {
            8 => (
                u8::from_str_radix(&raw[0..2], 16).ok()?,
                u8::from_str_radix(&raw[2..4], 16).ok()?,
                u8::from_str_radix(&raw[4..6], 16).ok()?,
                u8::from_str_radix(&raw[6..8], 16).ok()?,
            ),
            6 => (
                u8::from_str_radix(&raw[0..2], 16).ok()?,
                u8::from_str_radix(&raw[2..4], 16).ok()?,
                u8::from_str_radix(&raw[4..6], 16).ok()?,
                255,
            ),
            _ => return None,
        };
        Some(gpui::Hsla::from(gpui::Rgba {
            r: f32::from(red) / 255.,
            g: f32::from(green) / 255.,
            b: f32::from(blue) / 255.,
            a: f32::from(alpha) / 255.,
        }))
    }

    fn apply_aiur_theme(cx: &mut App) {
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(AIUR_JSON) else {
            console::error_1(&JsValue::from_str("embedded Aiur theme is invalid"));
            return;
        };
        let Some(style) = parsed
            .get("themes")
            .and_then(|themes| themes.get(0))
            .and_then(|theme| theme.get("style"))
            .and_then(serde_json::Value::as_object)
        else {
            console::error_1(&JsValue::from_str("embedded Aiur theme has no style"));
            return;
        };
        let get = |key: &str| -> Option<gpui::Hsla> {
            style
                .get(key)
                .and_then(serde_json::Value::as_str)
                .and_then(hex_to_hsla)
        };
        let current = <App as theme::ActiveTheme>::theme(cx).clone();
        let mut next = (*current).clone();
        next.name = "Aiur".into();
        let colors = &mut next.styles.colors;

        macro_rules! set {
            ($field:ident, $key:expr) => {
                if let Some(color) = get($key) {
                    colors.$field = color;
                }
            };
        }

        set!(background, "background");
        set!(surface_background, "surface.background");
        set!(elevated_surface_background, "elevated_surface.background");
        set!(panel_background, "panel.background");
        set!(editor_background, "editor.background");
        set!(element_background, "element.background");
        set!(element_hover, "element.hover");
        set!(element_active, "element.active");
        set!(element_selected, "element.selected");
        set!(ghost_element_hover, "ghost_element.hover");
        set!(ghost_element_selected, "ghost_element.selected");
        set!(border, "border");
        set!(border_variant, "border.variant");
        set!(border_focused, "border.focused");
        set!(text, "text");
        set!(text_muted, "text.muted");
        set!(text_placeholder, "text.placeholder");
        set!(text_disabled, "text.disabled");
        set!(text_accent, "text.accent");
        set!(icon, "icon");
        set!(icon_muted, "icon.muted");
        set!(icon_accent, "icon.accent");
        set!(status_bar_background, "status_bar.background");
        set!(title_bar_background, "title_bar.background");
        set!(toolbar_background, "toolbar.background");
        set!(tab_bar_background, "tab_bar.background");
        set!(tab_active_background, "tab.active_background");
        set!(tab_inactive_background, "tab.inactive_background");

        theme::GlobalTheme::update_theme(cx, std::sync::Arc::new(next));
    }

    pub fn run() {
        gpui_platform::web_init();
        let handle = gpui_platform::application().run_embedded(|cx: &mut App| {
            theme::set_theme_settings_provider(
                Box::new(WebThemeSettings {
                    ui_font: gpui::font("IBM Plex Sans"),
                    buffer_font: gpui::font("Lilex"),
                }),
                cx,
            );
            theme::init(theme::LoadThemes::JustBase, cx);
            apply_aiur_theme(cx);
            let bounds = Bounds::centered(None, size(px(1180.), px(820.)), cx);
            match cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(DiamondHands::new),
            ) {
                Ok(_) => cx.activate(true),
                Err(error) => console::error_1(&JsValue::from_str(&format!(
                    "failed to open Operation Diamond Hands: {error:#}"
                ))),
            }
        });
        std::mem::forget(handle);
    }
}

#[cfg(target_family = "wasm")]
fn main() {
    web_app::run();
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    eprintln!("diamond_hands_web builds for wasm32-unknown-unknown");
}
