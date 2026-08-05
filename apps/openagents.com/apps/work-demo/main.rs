#![cfg_attr(not(target_family = "wasm"), allow(dead_code, unused_imports))]

#[cfg(target_family = "wasm")]
mod web_app {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use futures::{StreamExt as _, channel::mpsc};
    use gpui::prelude::*;
    use gpui::{
        App, Bounds, Context, Font, FontWeight, InteractiveElement, Pixels, SharedString,
        StatefulInteractiveElement, Task, Window, WindowBounds, WindowOptions, div, px, rgb, size,
    };
    use immortal::domain::Event as NostrEvent;
    use theme::{ActiveTheme as _, ThemeSettingsProvider, UiDensity};
    use ui::{
        Button, ButtonCommon as _, ButtonStyle, Clickable as _, Color, Indicator, Label,
        LabelCommon as _, LabelSize, h_flex, v_flex,
    };
    use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
    use web_sys::{CloseEvent, Event, MessageEvent, WebSocket, console};

    /// Default relay. Override with `?relay=wss://...`.
    const RELAY_URL: &str = "wss://relay.openagents.com";

    /// PLACEHOLDER — the NIP-WK/NIP-PI authority pubkey this demo pins.
    ///
    /// The real dev authority key is produced by the seed side
    /// (OpenAgentsInc/immortal#33, running in parallel). Wiring it in means
    /// replacing this one constant; nothing else in the app needs to change.
    /// Until then this is 64 zero nibbles, which is not a valid x-only key,
    /// so every relay event is refused and the UI says so honestly.
    /// Override at runtime with `?authority=<64-hex-pubkey>`.
    const AUTHORITY_PUBKEY: &str =
        "0000000000000000000000000000000000000000000000000000000000000000";

    const ISSUES_SUBSCRIPTION_ID: &str = "work-items-v1";
    const KIND_WORK_RECORD: u16 = 32170;
    const KIND_WORK_EVENT: u16 = 32171;
    const KIND_ISSUE_PROJECTION: u16 = 32200;

    /// NIP-WK 1.3 baseline Work State vocabulary, in board order.
    const BASELINE_STATES: [&str; 9] = [
        "draft",
        "planned",
        "active",
        "blocked",
        "in_review",
        "done",
        "canceled",
        "superseded",
        "archived",
    ];

    /// NIP-WK 2.3 recommended Work Event vocabulary. Anything else is
    /// preserved and displayed as unknown, never reinterpreted.
    const KNOWN_EVENT_KINDS: [&str; 20] = [
        "created",
        "objective_revised",
        "classified",
        "related",
        "assigned",
        "delegated",
        "delegation_revoked",
        "state_changed",
        "blocked",
        "unblocked",
        "session_started",
        "session_ended",
        "activity_recorded",
        "evidence_attached",
        "verification_recorded",
        "disposition_recorded",
        "closed",
        "reopened",
        "superseded",
        "archived",
    ];

    /// Tag names this renderer understands per record kind. Everything else
    /// is preserved and shown in the UNKNOWN TAGS panel.
    const KNOWN_ISSUE_TAGS: [&str; 18] = [
        "d",
        "org",
        "team",
        "identifier",
        "identifier_alias",
        "title",
        "state",
        "revision",
        "published_at",
        "archived_at",
        "priority",
        "estimate",
        "due",
        "p",
        "label",
        "a",
        "e",
        "sla",
    ];
    const KNOWN_WORK_EVENT_TAGS: [&str; 12] = [
        "d",
        "work",
        "seq",
        "event",
        "p",
        "occurred_at",
        "admitted_at",
        "e",
        "a",
        "revision",
        "x",
        "reason",
    ];

    const FIXTURES_JSON: &str = include_str!("fixtures.json");

    // Terminal palette shared with the market demo: black void, amber
    // command surfaces, green/red honesty markers.
    const VOID: u32 = 0x000000;
    const PANEL: u32 = 0x0a0a08;
    const RAISED: u32 = 0x12110c;
    const HAIRLINE: u32 = 0x2a2417;
    const AMBER: u32 = 0xff9f0a;
    const AMBER_DIM: u32 = 0xb8770e;
    const BODY: u32 = 0xd8d4c8;
    const SECONDARY: u32 = 0x9a958a;
    const FAINT: u32 = 0x6a665c;
    const GREEN: u32 = 0x2fd575;
    const RED: u32 = 0xff4d42;
    const CYAN: u32 = 0x4fd0ff;
    const SOFT_BLUE: u32 = 0x8fb6ff;

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
            px(13.)
        }

        fn buffer_font_size(&self, _cx: &App) -> Pixels {
            px(12.)
        }

        fn ui_density(&self, _cx: &App) -> UiDensity {
            UiDensity::Compact
        }
    }

    // Terminal-tuned surface values applied over Omega's theme machinery.
    const TERMINAL_JSON: &str = r##"{
      "themes": [{
        "style": {
          "border": "#2a2417ff",
          "border.variant": "#1c1810ff",
          "border.focused": "#ff9f0aff",
          "elevated_surface.background": "#12110cff",
          "surface.background": "#0a0a08ff",
          "background": "#000000ff",
          "panel.background": "#0a0a08ff",
          "editor.background": "#000000ff",
          "element.background": "#12110cff",
          "element.hover": "#ff9f0a14",
          "element.active": "#ff9f0a21",
          "element.selected": "#ff9f0a29",
          "ghost_element.hover": "#ff9f0a14",
          "ghost_element.selected": "#ff9f0a29",
          "text": "#d8d4c8ff",
          "text.muted": "#9a958aff",
          "text.placeholder": "#6a665cff",
          "text.disabled": "#4c483fff",
          "text.accent": "#ff9f0aff",
          "icon": "#d8d4c8ff",
          "icon.muted": "#9a958aff",
          "icon.accent": "#ff9f0aff",
          "status_bar.background": "#0a0a08ff",
          "title_bar.background": "#0a0a08ff",
          "toolbar.background": "#000000ff",
          "tab_bar.background": "#0a0a08ff",
          "tab.active_background": "#000000ff",
          "tab.inactive_background": "#0a0a08ff"
        }
      }]
    }"##;

    // ------------------------------------------------------------------
    // Configuration
    // ------------------------------------------------------------------

    #[derive(Clone)]
    struct DemoConfig {
        relay_url: String,
        authority: String,
        fixture: bool,
    }

    fn is_lower_hex_64(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    fn resolve_config() -> (DemoConfig, Vec<String>) {
        let mut config = DemoConfig {
            relay_url: RELAY_URL.to_owned(),
            authority: AUTHORITY_PUBKEY.to_owned(),
            fixture: false,
        };
        let mut problems = Vec::new();
        let search = web_sys::window()
            .and_then(|window| window.location().search().ok())
            .unwrap_or_default();
        if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search) {
            if let Some(relay) = params.get("relay") {
                if (relay.starts_with("wss://") || relay.starts_with("ws://"))
                    && relay.len() <= 2_048
                {
                    config.relay_url = relay;
                } else {
                    problems.push(format!(
                        "ignored ?relay override — not a bounded ws(s) URL: {relay}"
                    ));
                }
            }
            if let Some(authority) = params.get("authority") {
                if is_lower_hex_64(&authority) {
                    config.authority = authority;
                } else {
                    problems.push(
                        "ignored ?authority override — needs 64 lowercase hex characters"
                            .to_owned(),
                    );
                }
            }
            if let Some(fixture) = params.get("fixture") {
                config.fixture = fixture == "1" || fixture == "true";
            }
        }
        (config, problems)
    }

    fn authority_is_placeholder(authority: &str) -> bool {
        authority == AUTHORITY_PUBKEY
    }

    // ------------------------------------------------------------------
    // Relay transport: the browser owns the WebSocket; state stays here.
    // ------------------------------------------------------------------

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
            initial_request: String,
            sender: mpsc::UnboundedSender<RelayInput>,
        ) -> Result<Self, String> {
            let socket = WebSocket::new(url)
                .map_err(|error| format!("browser refused relay WebSocket: {error:?}"))?;

            let open_socket = socket.clone();
            let open_sender = sender.clone();
            let open = Closure::<dyn FnMut(Event)>::new(move |_event| {
                if let Err(error) = open_socket.send_with_str(&initial_request) {
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

        fn send(&self, frame: &str) -> Result<(), String> {
            if self.socket.ready_state() != WebSocket::OPEN {
                return Err("relay socket is not open".to_owned());
            }
            self.socket
                .send_with_str(frame)
                .map_err(|error| format!("failed to send relay frame: {error:?}"))
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
                    "failed to close work relay socket: {error:?}"
                )));
            }
        }
    }

    fn send_relay_input(sender: &mpsc::UnboundedSender<RelayInput>, input: RelayInput) {
        if sender.unbounded_send(input).is_err() {
            console::warn_1(&JsValue::from_str(
                "work relay event arrived after the GPUI view closed",
            ));
        }
    }

    // ------------------------------------------------------------------
    // Verified projection state
    // ------------------------------------------------------------------

    #[derive(Clone)]
    struct Participant {
        pubkey: String,
        marker: String,
    }

    #[derive(Clone)]
    struct AddressRef {
        coordinate: String,
        marker: String,
    }

    /// One kind-32200 Issue Projection, parsed but never reinterpreted:
    /// unknown tags ride along verbatim.
    #[derive(Clone)]
    struct IssueProjection {
        work_ref: String,
        identifier: Option<String>,
        title: Option<String>,
        state: Option<String>,
        revision: Option<u64>,
        priority: Option<String>,
        estimate: Option<String>,
        due: Option<String>,
        org: Option<String>,
        team: Option<String>,
        published_at: Option<u64>,
        participants: Vec<Participant>,
        labels: Vec<String>,
        refs: Vec<AddressRef>,
        head: Option<String>,
        content: String,
        unknown_tags: Vec<Vec<String>>,
        created_at: u64,
        fixture: bool,
    }

    /// The head of the matching kind-32170 Work Record, kept only for the
    /// revision cross-check and gap honesty.
    #[derive(Clone)]
    struct WorkRecordHead {
        revision: Option<u64>,
        created_at: u64,
    }

    #[derive(Clone)]
    struct WorkEventRow {
        d: String,
        seq: Option<u64>,
        event_kind: String,
        actor: Option<String>,
        occurred_at: Option<u64>,
        admitted_at: Option<u64>,
        revision: Option<u64>,
        reason: Option<String>,
        unknown_tags: Vec<Vec<String>>,
        fixture: bool,
    }

    struct Timeline {
        rows: Vec<WorkEventRow>,
        eose: bool,
        broad_fallback: bool,
    }

    enum Freshness {
        Fresh,
        IssueStale { issue: u64, work: u64 },
        WorkStale { issue: u64, work: u64 },
        NoWorkRecord,
        NoRevision,
    }

    enum ConnectionState {
        Fixture,
        Connecting,
        Syncing,
        Live,
        Reconnecting,
        Closed,
    }

    enum TimelineDisplayRow {
        Event(WorkEventRow),
        Duplicate(WorkEventRow),
        Gap { from: u64, to: u64 },
        LeadingUnknown { below: u64 },
    }

    pub struct WorkItems {
        config: DemoConfig,
        config_problems: Vec<String>,
        connection: ConnectionState,
        transport_message: Option<String>,
        sender: mpsc::UnboundedSender<RelayInput>,
        relay: Option<BrowserRelay>,
        reconnect_attempt: u32,
        reconnect_task: Option<Task<()>>,
        issues: BTreeMap<String, IssueProjection>,
        works: BTreeMap<String, WorkRecordHead>,
        timelines: BTreeMap<String, Timeline>,
        pending_timeline_subs: BTreeMap<String, String>,
        next_timeline_sub: u64,
        selected: Option<String>,
        verified_events: u64,
        refused_foreign: u64,
        invalid_events: u64,
        diagnostics: Vec<String>,
        issues_eose: bool,
        _incoming_task: Task<()>,
    }

    fn shorten(hex: &str) -> String {
        if hex.len() > 16 {
            format!("{}…{}", &hex[..8], &hex[hex.len() - 8..])
        } else {
            hex.to_owned()
        }
    }

    fn format_timestamp(seconds: u64) -> String {
        // Civil-from-days (Howard Hinnant) so the label needs no time crate.
        let days = (seconds / 86_400) as i64;
        let rem = seconds % 86_400;
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let year = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = if month <= 2 { year + 1 } else { year };
        format!(
            "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
            rem / 3_600,
            (rem % 3_600) / 60
        )
    }

    fn tag_first<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
        tags.iter()
            .find(|tag| tag.first().map(String::as_str) == Some(name))
            .and_then(|tag| tag.get(1))
            .map(String::as_str)
    }

    fn tag_u64(tags: &[Vec<String>], name: &str) -> Option<u64> {
        tag_first(tags, name).and_then(|value| value.parse().ok())
    }

    fn state_display(state: &str) -> String {
        // A NIP-WS state ref looks like `32215:<pubkey>:team-core:started`;
        // a baseline label is already bare. Display the last segment either
        // way and keep the full ref available in the detail view.
        state.rsplit(':').next().unwrap_or(state).to_owned()
    }

    fn state_bucket(state: Option<&str>) -> (usize, String) {
        let label = state.map(state_display);
        match label {
            Some(label) => match BASELINE_STATES.iter().position(|known| *known == label) {
                Some(index) => (index, label),
                None => (BASELINE_STATES.len(), format!("{label} (non-baseline)")),
            },
            None => (BASELINE_STATES.len() + 1, "no state tag".to_owned()),
        }
    }

    fn parse_tags(value: &serde_json::Value) -> Vec<Vec<String>> {
        value
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| {
                        tag.as_array().map(|parts| {
                            parts
                                .iter()
                                .map(|part| {
                                    part.as_str().map(str::to_owned).unwrap_or_else(|| {
                                        part.to_string()
                                    })
                                })
                                .collect()
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn unknown_tags_of(tags: &[Vec<String>], known: &[&str]) -> Vec<Vec<String>> {
        tags.iter()
            .filter(|tag| {
                tag.first()
                    .map(|name| !known.contains(&name.as_str()))
                    .unwrap_or(true)
            })
            .cloned()
            .collect()
    }

    fn parse_issue(
        tags: Vec<Vec<String>>,
        content: String,
        created_at: u64,
        fixture: bool,
    ) -> Option<IssueProjection> {
        let work_ref = tag_first(&tags, "d")?.to_owned();
        let mut participants = Vec::new();
        let mut labels = Vec::new();
        let mut refs = Vec::new();
        let mut head = None;
        for tag in &tags {
            match tag.first().map(String::as_str) {
                Some("p") => {
                    if let Some(pubkey) = tag.get(1) {
                        participants.push(Participant {
                            pubkey: pubkey.clone(),
                            marker: tag.get(3).cloned().unwrap_or_default(),
                        });
                    }
                }
                Some("label") => {
                    if let Some(value) = tag.get(1) {
                        labels.push(value.clone());
                    }
                }
                Some("a") => {
                    if let Some(coordinate) = tag.get(1) {
                        refs.push(AddressRef {
                            coordinate: coordinate.clone(),
                            marker: tag.get(3).cloned().unwrap_or_default(),
                        });
                    }
                }
                Some("e") => {
                    if tag.get(3).map(String::as_str) == Some("head") {
                        head = tag.get(1).cloned();
                    }
                }
                _ => {}
            }
        }
        let unknown_tags = unknown_tags_of(&tags, &KNOWN_ISSUE_TAGS);
        Some(IssueProjection {
            identifier: tag_first(&tags, "identifier").map(str::to_owned),
            title: tag_first(&tags, "title").map(str::to_owned),
            state: tag_first(&tags, "state").map(str::to_owned),
            revision: tag_u64(&tags, "revision"),
            priority: tag_first(&tags, "priority").map(str::to_owned),
            estimate: tag_first(&tags, "estimate").map(str::to_owned),
            due: tag_first(&tags, "due").map(str::to_owned),
            org: tag_first(&tags, "org").map(str::to_owned),
            team: tag_first(&tags, "team").map(str::to_owned),
            published_at: tag_u64(&tags, "published_at"),
            participants,
            labels,
            refs,
            head,
            content,
            unknown_tags,
            created_at,
            fixture,
            work_ref,
        })
    }

    fn parse_work_event(
        tags: Vec<Vec<String>>,
        fixture: bool,
    ) -> Option<(String, WorkEventRow)> {
        let work_ref = tag_first(&tags, "work")?.to_owned();
        let d = tag_first(&tags, "d")?.to_owned();
        let actor = tags
            .iter()
            .find(|tag| {
                tag.first().map(String::as_str) == Some("p")
                    && tag.get(3).map(String::as_str) == Some("actor")
            })
            .and_then(|tag| tag.get(1))
            .cloned();
        let unknown_tags = unknown_tags_of(&tags, &KNOWN_WORK_EVENT_TAGS);
        let row = WorkEventRow {
            seq: tag_u64(&tags, "seq"),
            event_kind: tag_first(&tags, "event").unwrap_or("(no event tag)").to_owned(),
            actor,
            occurred_at: tag_u64(&tags, "occurred_at"),
            admitted_at: tag_u64(&tags, "admitted_at"),
            revision: tag_u64(&tags, "revision"),
            reason: tag_first(&tags, "reason").map(str::to_owned),
            unknown_tags,
            fixture,
            d,
        };
        Some((work_ref, row))
    }

    fn timeline_display_rows(rows: &[WorkEventRow]) -> Vec<TimelineDisplayRow> {
        let mut sorted: Vec<&WorkEventRow> = rows.iter().collect();
        sorted.sort_by(|a, b| a.seq.cmp(&b.seq).then_with(|| a.d.cmp(&b.d)));
        let mut display = Vec::new();
        let mut previous: Option<u64> = None;
        for row in sorted {
            match row.seq {
                None => display.push(TimelineDisplayRow::Event(row.clone())),
                Some(seq) => {
                    match previous {
                        None => {
                            // WK 2.4: seq is dense. If the first observed
                            // event is not the `created` origin, everything
                            // below it is unreceived history, surfaced.
                            if seq > 0 && row.event_kind != "created" {
                                display.push(TimelineDisplayRow::LeadingUnknown { below: seq });
                            }
                            display.push(TimelineDisplayRow::Event(row.clone()));
                        }
                        Some(previous_seq) if seq == previous_seq => {
                            display.push(TimelineDisplayRow::Duplicate(row.clone()));
                        }
                        Some(previous_seq) => {
                            if seq > previous_seq + 1 {
                                display.push(TimelineDisplayRow::Gap {
                                    from: previous_seq + 1,
                                    to: seq - 1,
                                });
                            }
                            display.push(TimelineDisplayRow::Event(row.clone()));
                        }
                    }
                    previous = Some(seq);
                }
            }
        }
        display
    }

    impl WorkItems {
        pub fn new(cx: &mut Context<Self>) -> Self {
            let (config, config_problems) = resolve_config();
            let (sender, mut receiver) = mpsc::unbounded();

            let incoming_task = cx.spawn(async move |this, cx| {
                while let Some(input) = receiver.next().await {
                    match this.update(cx, |this, cx| this.handle_relay_input(input, cx)) {
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
            });

            let mut view = Self {
                config,
                config_problems,
                connection: ConnectionState::Connecting,
                transport_message: None,
                sender,
                relay: None,
                reconnect_attempt: 0,
                reconnect_task: None,
                issues: BTreeMap::new(),
                works: BTreeMap::new(),
                timelines: BTreeMap::new(),
                pending_timeline_subs: BTreeMap::new(),
                next_timeline_sub: 0,
                selected: None,
                verified_events: 0,
                refused_foreign: 0,
                invalid_events: 0,
                diagnostics: Vec::new(),
                issues_eose: false,
                _incoming_task: incoming_task,
            };
            if view.config.fixture {
                view.connection = ConnectionState::Fixture;
                view.load_fixtures();
            } else {
                view.open_relay(cx);
            }
            view
        }

        fn push_diagnostic(&mut self, line: String) {
            self.diagnostics.insert(0, line);
            self.diagnostics.truncate(8);
        }

        fn load_fixtures(&mut self) {
            let parsed: serde_json::Value = match serde_json::from_str(FIXTURES_JSON) {
                Ok(value) => value,
                Err(error) => {
                    self.push_diagnostic(format!("fixture file did not parse: {error}"));
                    return;
                }
            };
            let Some(events) = parsed.get("events").and_then(serde_json::Value::as_array)
            else {
                self.push_diagnostic("fixture file has no events array".to_owned());
                return;
            };
            for value in events {
                let kind = value.get("kind").and_then(serde_json::Value::as_u64);
                let tags = parse_tags(value);
                let content = value
                    .get("content")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let created_at = tag_u64(&tags, "published_at").unwrap_or(0);
                match kind {
                    Some(kind) if kind == u64::from(KIND_ISSUE_PROJECTION) => {
                        if let Some(issue) = parse_issue(tags, content, created_at, true) {
                            self.issues.insert(issue.work_ref.clone(), issue);
                        }
                    }
                    Some(kind) if kind == u64::from(KIND_WORK_RECORD) => {
                        if let Some(d) = tag_first(&tags, "d").map(str::to_owned) {
                            self.works.insert(
                                d,
                                WorkRecordHead {
                                    revision: tag_u64(&tags, "revision"),
                                    created_at,
                                },
                            );
                        }
                    }
                    Some(kind) if kind == u64::from(KIND_WORK_EVENT) => {
                        if let Some((work_ref, row)) = parse_work_event(tags, true) {
                            let timeline =
                                self.timelines.entry(work_ref).or_insert_with(|| Timeline {
                                    rows: Vec::new(),
                                    eose: true,
                                    broad_fallback: false,
                                });
                            timeline.rows.push(row);
                        }
                    }
                    _ => {
                        self.push_diagnostic(format!(
                            "fixture event with unhandled kind {kind:?} preserved unrendered"
                        ));
                    }
                }
            }
            self.issues_eose = true;
            self.selected = self.issues.keys().next().cloned();
        }

        fn issues_request(&self) -> String {
            serde_json::json!([
                "REQ",
                ISSUES_SUBSCRIPTION_ID,
                {
                    "kinds": [KIND_WORK_RECORD, KIND_ISSUE_PROJECTION],
                    "authors": [self.config.authority],
                }
            ])
            .to_string()
        }

        fn open_relay(&mut self, cx: &mut Context<Self>) {
            self.connection = ConnectionState::Connecting;
            self.issues_eose = false;
            self.transport_message = None;
            let request = self.issues_request();
            match BrowserRelay::connect(&self.config.relay_url, request, self.sender.clone()) {
                Ok(relay) => self.relay = Some(relay),
                Err(error) => {
                    self.relay = None;
                    self.connection = ConnectionState::Closed;
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
                    this.connection = ConnectionState::Reconnecting;
                    this.open_relay(cx);
                }) {}
            }));
        }

        fn retry_now(&mut self, cx: &mut Context<Self>) {
            self.reconnect_task = None;
            self.reconnect_attempt = 0;
            self.relay = None;
            self.open_relay(cx);
        }

        fn handle_relay_input(&mut self, input: RelayInput, cx: &mut Context<Self>) {
            match input {
                RelayInput::Opened => {
                    self.reconnect_attempt = 0;
                    self.transport_message = None;
                    self.connection = ConnectionState::Syncing;
                    self.pending_timeline_subs.clear();
                }
                RelayInput::Text(text) => self.ingest_frame(&text),
                RelayInput::Closed { code, reason } => {
                    self.relay = None;
                    self.connection = ConnectionState::Closed;
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

        fn ingest_frame(&mut self, text: &str) {
            if text.len() > 512 * 1024 {
                self.push_diagnostic("oversized relay frame excluded".to_owned());
                return;
            }
            let value: serde_json::Value = match serde_json::from_str(text) {
                Ok(value) => value,
                Err(error) => {
                    self.push_diagnostic(format!("non-JSON relay frame excluded: {error}"));
                    return;
                }
            };
            let Some(frame) = value.as_array() else {
                self.push_diagnostic("non-array relay frame excluded".to_owned());
                return;
            };
            match frame.first().and_then(serde_json::Value::as_str) {
                Some("EVENT") => {
                    if let Some(event) = frame.get(2) {
                        self.ingest_event(event.clone());
                    }
                }
                Some("EOSE") => {
                    let sub = frame
                        .get(1)
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    self.handle_eose(&sub);
                }
                Some("CLOSED") => {
                    let sub = frame
                        .get(1)
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    let message = frame
                        .get(2)
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    self.handle_closed_subscription(&sub, &message);
                }
                Some("NOTICE") => {
                    let message = frame
                        .get(1)
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    self.push_diagnostic(format!("relay notice: {message}"));
                }
                Some(other) => {
                    self.push_diagnostic(format!("unhandled relay frame type {other:?}"));
                }
                None => self.push_diagnostic("relay frame without a type excluded".to_owned()),
            }
        }

        fn ingest_event(&mut self, value: serde_json::Value) {
            let event: NostrEvent = match serde_json::from_value(value) {
                Ok(event) => event,
                Err(error) => {
                    self.invalid_events += 1;
                    self.push_diagnostic(format!("malformed event excluded: {error}"));
                    return;
                }
            };
            // NIP-01 id (sha256 of the canonical serialization) plus the
            // BIP-340 Schnorr signature, via the immortal client core.
            if let Err(error) = event.validate_structure() {
                self.invalid_events += 1;
                self.push_diagnostic(format!("structurally invalid event excluded: {error}"));
                return;
            }
            if let Err(error) = event.validate_crypto() {
                self.invalid_events += 1;
                self.push_diagnostic(format!("signature check failed, event excluded: {error}"));
                return;
            }
            if event.pubkey != self.config.authority {
                self.refused_foreign += 1;
                self.push_diagnostic(format!(
                    "refused event from non-authority key {}",
                    shorten(&event.pubkey)
                ));
                return;
            }
            self.verified_events += 1;
            let tags: Vec<Vec<String>> = event.tags.iter().map(|tag| tag.0.clone()).collect();
            match event.kind {
                KIND_ISSUE_PROJECTION => {
                    if let Some(issue) =
                        parse_issue(tags, event.content.clone(), event.created_at, false)
                    {
                        let keep = self
                            .issues
                            .get(&issue.work_ref)
                            .map(|existing| event.created_at >= existing.created_at)
                            .unwrap_or(true);
                        if keep {
                            self.issues.insert(issue.work_ref.clone(), issue);
                        }
                    } else {
                        self.push_diagnostic(
                            "verified 32200 without a d tag excluded".to_owned(),
                        );
                    }
                }
                KIND_WORK_RECORD => {
                    if let Some(d) = tag_first(&tags, "d").map(str::to_owned) {
                        let keep = self
                            .works
                            .get(&d)
                            .map(|existing| event.created_at >= existing.created_at)
                            .unwrap_or(true);
                        if keep {
                            self.works.insert(
                                d,
                                WorkRecordHead {
                                    revision: tag_u64(&tags, "revision"),
                                    created_at: event.created_at,
                                },
                            );
                        }
                    } else {
                        self.push_diagnostic(
                            "verified 32170 without a d tag excluded".to_owned(),
                        );
                    }
                }
                KIND_WORK_EVENT => {
                    if let Some((work_ref, row)) = parse_work_event(tags, false) {
                        let timeline =
                            self.timelines.entry(work_ref).or_insert_with(|| Timeline {
                                rows: Vec::new(),
                                eose: false,
                                broad_fallback: false,
                            });
                        if !timeline.rows.iter().any(|existing| existing.d == row.d) {
                            timeline.rows.push(row);
                        }
                    } else {
                        self.push_diagnostic(
                            "verified 32171 without work/d tags excluded".to_owned(),
                        );
                    }
                }
                other => {
                    self.push_diagnostic(format!(
                        "verified event of unrequested kind {other} preserved unrendered"
                    ));
                }
            }
        }

        fn handle_eose(&mut self, sub: &str) {
            if sub == ISSUES_SUBSCRIPTION_ID {
                self.issues_eose = true;
                self.connection = ConnectionState::Live;
                if self.selected.is_none() {
                    self.selected = self.issues.keys().next().cloned();
                }
                return;
            }
            if let Some(work_ref) = self.pending_timeline_subs.remove(sub) {
                if let Some(timeline) = self.timelines.get_mut(&work_ref) {
                    timeline.eose = true;
                } else {
                    self.timelines.insert(
                        work_ref,
                        Timeline {
                            rows: Vec::new(),
                            eose: true,
                            broad_fallback: false,
                        },
                    );
                }
                let close = serde_json::json!(["CLOSE", sub]).to_string();
                if let Some(relay) = &self.relay
                    && let Err(error) = relay.send(&close)
                {
                    self.push_diagnostic(error);
                }
            }
        }

        fn handle_closed_subscription(&mut self, sub: &str, message: &str) {
            if sub == ISSUES_SUBSCRIPTION_ID {
                self.push_diagnostic(format!("relay closed the issues subscription: {message}"));
                return;
            }
            let Some(work_ref) = self.pending_timeline_subs.remove(sub) else {
                return;
            };
            let already_fell_back = self
                .timelines
                .get(&work_ref)
                .map(|timeline| timeline.broad_fallback)
                .unwrap_or(false);
            if already_fell_back {
                self.push_diagnostic(format!(
                    "relay refused the fallback Work Event subscription too: {message}"
                ));
                return;
            }
            // NIP-PI's rendering contract filters on `#work`, but a relay
            // limited to single-letter tag selectors (immortal today) rejects
            // that field. Fall back to kinds+authors and filter client-side;
            // the projection stays honest either way because every event is
            // verified and routed by its own `work` tag.
            self.push_diagnostic(format!(
                "relay refused the #work filter ({message}) — falling back to client-side filtering"
            ));
            self.timelines
                .entry(work_ref.clone())
                .or_insert_with(|| Timeline {
                    rows: Vec::new(),
                    eose: false,
                    broad_fallback: false,
                })
                .broad_fallback = true;
            self.next_timeline_sub += 1;
            let sub_id = format!("work-evt-{}", self.next_timeline_sub);
            let request = serde_json::json!([
                "REQ",
                sub_id,
                {
                    "kinds": [KIND_WORK_EVENT],
                    "authors": [self.config.authority],
                }
            ])
            .to_string();
            self.pending_timeline_subs.insert(sub_id, work_ref);
            if let Some(relay) = &self.relay
                && let Err(error) = relay.send(&request)
            {
                self.push_diagnostic(error);
            }
        }

        fn request_timeline(&mut self, work_ref: &str) {
            if self.config.fixture {
                return;
            }
            if self
                .timelines
                .get(work_ref)
                .map(|timeline| timeline.eose)
                .unwrap_or(false)
            {
                return;
            }
            if self
                .pending_timeline_subs
                .values()
                .any(|pending| pending == work_ref)
            {
                return;
            }
            self.next_timeline_sub += 1;
            let sub_id = format!("work-evt-{}", self.next_timeline_sub);
            // The NIP-PI section 2 rendering-contract query, verbatim.
            let request = serde_json::json!([
                "REQ",
                sub_id,
                {
                    "kinds": [KIND_WORK_EVENT],
                    "authors": [self.config.authority],
                    "#work": [work_ref],
                }
            ])
            .to_string();
            self.pending_timeline_subs
                .insert(sub_id, work_ref.to_owned());
            if let Some(relay) = &self.relay {
                if let Err(error) = relay.send(&request) {
                    self.push_diagnostic(error);
                }
            } else {
                self.push_diagnostic(
                    "cannot request the Work Event timeline while disconnected".to_owned(),
                );
            }
        }

        fn select(&mut self, work_ref: String, cx: &mut Context<Self>) {
            self.request_timeline(&work_ref);
            self.selected = Some(work_ref);
            cx.notify();
        }

        fn freshness(&self, issue: &IssueProjection) -> Freshness {
            let Some(work) = self.works.get(&issue.work_ref) else {
                return Freshness::NoWorkRecord;
            };
            match (issue.revision, work.revision) {
                (Some(issue_revision), Some(work_revision)) => {
                    if issue_revision < work_revision {
                        Freshness::IssueStale {
                            issue: issue_revision,
                            work: work_revision,
                        }
                    } else if work_revision < issue_revision {
                        Freshness::WorkStale {
                            issue: issue_revision,
                            work: work_revision,
                        }
                    } else {
                        Freshness::Fresh
                    }
                }
                _ => Freshness::NoRevision,
            }
        }

        /// PI section 2: counts are display facts. Whenever any contributing
        /// record is stale or missing, or the snapshot is incomplete, the
        /// board must say so instead of presenting the totals as canonical.
        fn counts_qualifier(&self) -> Option<String> {
            if self.config.fixture {
                return Some("fixture data — not the live relay".to_owned());
            }
            if !self.issues_eose {
                return Some("snapshot incomplete (no EOSE yet)".to_owned());
            }
            if !matches!(self.connection, ConnectionState::Live) {
                return Some("relay stream interrupted".to_owned());
            }
            let mut stale = 0_usize;
            let mut missing = 0_usize;
            for issue in self.issues.values() {
                match self.freshness(issue) {
                    Freshness::IssueStale { .. } | Freshness::WorkStale { .. } => stale += 1,
                    Freshness::NoWorkRecord | Freshness::NoRevision => missing += 1,
                    Freshness::Fresh => {}
                }
            }
            if stale > 0 || missing > 0 {
                return Some(format!(
                    "not canonical — {stale} stale, {missing} without a matching 32170"
                ));
            }
            None
        }
    }

    // ------------------------------------------------------------------
    // Render helpers
    // ------------------------------------------------------------------

    fn mono(text: impl Into<SharedString>, color: u32) -> Label {
        Label::new(text)
            .size(LabelSize::XSmall)
            .color(Color::Custom(rgb(color).into()))
    }

    fn panel_frame(title: &'static str, right: Option<String>) -> gpui::Div {
        v_flex()
            .bg(rgb(PANEL))
            .border_1()
            .border_color(rgb(HAIRLINE))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .px_2()
                    .py_1()
                    .bg(rgb(RAISED))
                    .border_b_1()
                    .border_color(rgb(HAIRLINE))
                    .child(
                        Label::new(title)
                            .size(LabelSize::XSmall)
                            .weight(FontWeight::BOLD)
                            .color(Color::Custom(rgb(AMBER).into())),
                    )
                    .child(mono(right.unwrap_or_default(), FAINT)),
            )
    }

    fn tag_chip(text: String, color: u32) -> impl IntoElement {
        div()
            .px_1p5()
            .rounded_sm()
            .border_1()
            .border_color(rgb(color))
            .child(mono(text, color))
    }

    fn key_value(key: &'static str, value: String, value_color: u32) -> impl IntoElement {
        h_flex()
            .gap_2()
            .items_start()
            .child(div().min_w(px(96.)).flex_none().child(mono(key, FAINT)))
            .child(mono(value, value_color))
    }

    fn priority_color(priority: &str) -> u32 {
        match priority {
            "urgent" => RED,
            "high" => AMBER,
            "medium" => SOFT_BLUE,
            "low" => SECONDARY,
            _ => FAINT,
        }
    }

    impl WorkItems {
        fn connection_badge(&self) -> (Color, String, u32) {
            match &self.connection {
                ConnectionState::Fixture => {
                    (Color::Warning, "FIXTURE DATA — RELAY OFF".to_owned(), AMBER)
                }
                ConnectionState::Connecting => {
                    (Color::Muted, "RELAY: CONNECTING".to_owned(), FAINT)
                }
                ConnectionState::Syncing => {
                    (Color::Muted, "RELAY: SYNCING".to_owned(), SOFT_BLUE)
                }
                ConnectionState::Live => (Color::Success, "RELAY: LIVE".to_owned(), GREEN),
                ConnectionState::Reconnecting => {
                    (Color::Warning, "RELAY: RECONNECTING".to_owned(), AMBER)
                }
                ConnectionState::Closed => (Color::Warning, "RELAY: CLOSED".to_owned(), RED),
            }
        }

        fn render_top_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            let (dot, text, color) = self.connection_badge();
            h_flex()
                .justify_between()
                .items_center()
                .px_3()
                .py_1p5()
                .bg(rgb(RAISED))
                .border_b_1()
                .border_color(rgb(AMBER_DIM))
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            Label::new("OPENAGENTS WORK")
                                .size(LabelSize::Small)
                                .weight(FontWeight::EXTRA_BOLD)
                                .color(Color::Custom(rgb(AMBER).into())),
                        )
                        .child(mono("<OAWRK>", AMBER_DIM))
                        .child(mono("NIP-WK / NIP-PI ISSUE PROJECTION", SECONDARY)),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child(Indicator::dot().color(dot))
                                .child(mono(text, color)),
                        )
                        .child(mono(
                            format!("authority {}", shorten(&self.config.authority)),
                            if authority_is_placeholder(&self.config.authority) {
                                RED
                            } else {
                                BODY
                            },
                        ))
                        .child(
                            div()
                                .px_2()
                                .py_0p5()
                                .bg(rgb(0x2a1f0a))
                                .border_1()
                                .border_color(rgb(AMBER))
                                .child(mono("READ-ONLY — NO INTENTS, NO KEYS", AMBER)),
                        ),
                )
        }

        fn render_notices(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
            let mut notices: Vec<(String, u32)> = Vec::new();
            if authority_is_placeholder(&self.config.authority) && !self.config.fixture {
                notices.push((
                    "The pinned authority pubkey is still the placeholder. Every relay event \
                     is refused until the dev authority from immortal#33 replaces \
                     AUTHORITY_PUBKEY (or ?authority=<hex> overrides it). Add ?fixture=1 to \
                     render the checked-in spec examples."
                        .to_owned(),
                    RED,
                ));
            }
            for problem in &self.config_problems {
                notices.push((problem.clone(), AMBER));
            }
            if let Some(message) = &self.transport_message {
                notices.push((message.clone(), AMBER));
            }
            if notices.is_empty() {
                return None;
            }
            let mut block = v_flex().gap_1().px_3().py_1();
            for (text, color) in notices {
                block = block.child(mono(text, color));
            }
            if matches!(
                self.connection,
                ConnectionState::Closed | ConnectionState::Reconnecting
            ) {
                block = block.child(
                    h_flex().child(
                        Button::new("retry-relay", "RETRY NOW")
                            .style(ButtonStyle::Outlined)
                            .on_click(
                                cx.listener(|this, _event, _window, cx| this.retry_now(cx)),
                            ),
                    ),
                );
            }
            Some(block)
        }

        fn render_board(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let mut grouped: BTreeMap<(usize, String), Vec<&IssueProjection>> = BTreeMap::new();
            for issue in self.issues.values() {
                grouped
                    .entry(state_bucket(issue.state.as_deref()))
                    .or_default()
                    .push(issue);
            }
            for issues in grouped.values_mut() {
                issues.sort_by(|a, b| {
                    a.identifier
                        .as_deref()
                        .unwrap_or(&a.work_ref)
                        .cmp(b.identifier.as_deref().unwrap_or(&b.work_ref))
                });
            }

            let mut body = v_flex();
            if self.issues.is_empty() {
                body = body.child(div().px_2().py_2().child(mono(
                    if self.config.fixture {
                        "no fixture issues parsed"
                    } else if self.issues_eose {
                        "the relay returned no issue projections for this authority"
                    } else {
                        "waiting for verified issue projections…"
                    },
                    FAINT,
                )));
            }
            let mut row_index = 0_usize;
            for ((_, state_label), issues) in &grouped {
                body = body.child(
                    h_flex()
                        .px_2()
                        .py_0p5()
                        .gap_2()
                        .items_center()
                        .bg(rgb(RAISED))
                        .border_b_1()
                        .border_color(rgb(HAIRLINE))
                        .child(
                            Label::new(state_label.to_uppercase())
                                .size(LabelSize::XSmall)
                                .weight(FontWeight::BOLD)
                                .color(Color::Custom(rgb(SOFT_BLUE).into())),
                        )
                        .child(mono(format!("{}", issues.len()), FAINT)),
                );
                for issue in issues {
                    let selected = self.selected.as_deref() == Some(issue.work_ref.as_str());
                    let stale =
                        matches!(self.freshness(issue), Freshness::IssueStale { .. });
                    let work_ref = issue.work_ref.clone();
                    let mut markers = h_flex().gap_1().items_center();
                    if let Some(priority) = &issue.priority {
                        markers =
                            markers.child(tag_chip(priority.clone(), priority_color(priority)));
                    }
                    for participant in &issue.participants {
                        let symbol = match participant.marker.as_str() {
                            "assignee" => Some(("A", GREEN)),
                            "delegate" => Some(("D", CYAN)),
                            _ => None,
                        };
                        if let Some((symbol, color)) = symbol {
                            markers = markers.child(tag_chip(
                                format!("{symbol} {}", shorten(&participant.pubkey)),
                                color,
                            ));
                        }
                    }
                    if stale {
                        markers = markers.child(tag_chip("STALE".to_owned(), RED));
                    }
                    if issue.fixture {
                        markers = markers.child(tag_chip("FIXTURE".to_owned(), AMBER));
                    }
                    body = body.child(
                        div()
                            .id(("issue-row", row_index))
                            .px_2()
                            .py_1()
                            .cursor_pointer()
                            .bg(rgb(if selected { 0x1a1607 } else { PANEL }))
                            .border_b_1()
                            .border_color(rgb(0x14120c))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.select(work_ref.clone(), cx);
                            }))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(mono(
                                                issue
                                                    .identifier
                                                    .clone()
                                                    .unwrap_or_else(|| "(no identifier)".into()),
                                                if selected { AMBER } else { AMBER_DIM },
                                            ))
                                            .child(mono(
                                                issue
                                                    .title
                                                    .clone()
                                                    .unwrap_or_else(|| "(no title)".into()),
                                                BODY,
                                            )),
                                    )
                                    .child(markers),
                            ),
                    );
                    row_index += 1;
                }
            }

            let qualifier = self.counts_qualifier();
            panel_frame(
                "ISSUES · KIND 32200 GROUPED BY STATE",
                Some(match &qualifier {
                    Some(reason) => format!("{} shown · {reason}", self.issues.len()),
                    None => format!("{} shown", self.issues.len()),
                }),
            )
            .flex_1()
            .overflow_hidden()
            .child(
                div()
                    .id("board-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(body),
            )
            .child(div().px_2().py_1().border_t_1().border_color(rgb(HAIRLINE)).child(mono(
                match qualifier {
                    Some(reason) => format!("counts are display facts — {reason}"),
                    None => "counts are display facts at this snapshot's revisions".to_owned(),
                },
                FAINT,
            )))
        }

        fn render_timeline(&self, issue: &IssueProjection) -> gpui::Div {
            let timeline = self.timelines.get(&issue.work_ref);
            let pending = self
                .pending_timeline_subs
                .values()
                .any(|pending| pending == &issue.work_ref);
            let mut rows = v_flex().gap_1();
            match timeline {
                None => {
                    rows = rows.child(mono(
                        if pending {
                            "requesting the Work Event stream…"
                        } else if self.config.fixture {
                            "no fixture Work Events for this item"
                        } else {
                            "no Work Events received for this item"
                        },
                        FAINT,
                    ));
                }
                Some(timeline) => {
                    if timeline.rows.is_empty() {
                        rows = rows.child(mono(
                            if timeline.eose {
                                "the relay holds no Work Events for this item"
                            } else {
                                "waiting for Work Events…"
                            },
                            FAINT,
                        ));
                    }
                    for display_row in timeline_display_rows(&timeline.rows) {
                        rows = rows.child(match display_row {
                            TimelineDisplayRow::Gap { from, to } => h_flex()
                                .gap_2()
                                .items_center()
                                .px_2()
                                .py_1()
                                .bg(rgb(0x1a0d0d))
                                .border_1()
                                .border_color(rgb(0x3a1d1d))
                                .child(mono(
                                    if from == to {
                                        format!("seq {from} missing — explicit gap")
                                    } else {
                                        format!("seq {from}-{to} missing — explicit gap")
                                    },
                                    RED,
                                ))
                                .child(mono("history with holes is not complete", FAINT))
                                .into_any_element(),
                            TimelineDisplayRow::LeadingUnknown { below } => h_flex()
                                .gap_2()
                                .items_center()
                                .px_2()
                                .py_1()
                                .bg(rgb(0x1a0d0d))
                                .border_1()
                                .border_color(rgb(0x3a1d1d))
                                .child(mono(
                                    format!(
                                        "events below seq {below} not received — earliest \
                                         observed event is not `created`"
                                    ),
                                    RED,
                                ))
                                .into_any_element(),
                            TimelineDisplayRow::Duplicate(row) => h_flex()
                                .gap_2()
                                .items_center()
                                .px_2()
                                .py_1()
                                .bg(rgb(0x1a0d0d))
                                .border_1()
                                .border_color(rgb(0x3a1d1d))
                                .child(mono(
                                    format!(
                                        "seq {} duplicated by {} — fork shown, not merged",
                                        row.seq.unwrap_or_default(),
                                        row.d
                                    ),
                                    RED,
                                ))
                                .into_any_element(),
                            TimelineDisplayRow::Event(row) => {
                                let unknown_kind =
                                    !KNOWN_EVENT_KINDS.contains(&row.event_kind.as_str());
                                let mut line = h_flex()
                                    .gap_2()
                                    .items_center()
                                    .px_2()
                                    .py_1()
                                    .bg(rgb(RAISED))
                                    .border_1()
                                    .border_color(rgb(HAIRLINE))
                                    .child(div().min_w(px(52.)).flex_none().child(mono(
                                        match row.seq {
                                            Some(seq) => format!("seq {seq}"),
                                            None => "seq ?".to_owned(),
                                        },
                                        if row.seq.is_some() { SECONDARY } else { RED },
                                    )))
                                    .child(
                                        Label::new(row.event_kind.clone())
                                            .size(LabelSize::XSmall)
                                            .weight(FontWeight::BOLD)
                                            .color(Color::Custom(
                                                rgb(if unknown_kind { AMBER } else { BODY })
                                                    .into(),
                                            )),
                                    );
                                if unknown_kind {
                                    line = line
                                        .child(tag_chip("unknown kind — preserved".into(), AMBER));
                                }
                                if let Some(actor) = &row.actor {
                                    line = line.child(mono(
                                        format!("actor {}", shorten(actor)),
                                        FAINT,
                                    ));
                                }
                                if let Some(revision) = row.revision {
                                    line = line.child(mono(format!("r{revision}"), FAINT));
                                }
                                if let Some(occurred_at) = row.occurred_at {
                                    line = line
                                        .child(mono(format_timestamp(occurred_at), FAINT));
                                } else if let Some(admitted_at) = row.admitted_at {
                                    line = line.child(mono(
                                        format!("admitted {}", format_timestamp(admitted_at)),
                                        FAINT,
                                    ));
                                }
                                if let Some(reason) = &row.reason {
                                    line = line.child(mono(reason.clone(), SECONDARY));
                                }
                                for tag in &row.unknown_tags {
                                    line = line.child(tag_chip(
                                        format!("? {}", tag.join(" ")),
                                        FAINT,
                                    ));
                                }
                                if row.fixture {
                                    line = line.child(tag_chip("FIXTURE".into(), AMBER));
                                }
                                line.into_any_element()
                            }
                        });
                    }
                    if !timeline.eose {
                        rows = rows.child(mono(
                            "snapshot incomplete — no EOSE for this stream yet",
                            AMBER,
                        ));
                    }
                    if timeline.broad_fallback {
                        rows = rows.child(mono(
                            "relay refused the NIP-PI #work filter; events were fetched by \
                             kind+author and filtered client-side",
                            AMBER,
                        ));
                    }
                }
            }
            panel_frame(
                "WORK EVENTS · KIND 32171 ORDERED BY SEQ",
                Some("gaps surfaced, never papered over".into()),
            )
            .child(v_flex().px_2().py_1p5().gap_1().child(rows))
        }

        fn render_detail(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let Some(issue) = self
                .selected
                .as_ref()
                .and_then(|work_ref| self.issues.get(work_ref))
                .cloned()
            else {
                return v_flex().gap_2().child(
                    panel_frame("ISSUE DETAIL", None).child(
                        div()
                            .px_2()
                            .py_2()
                            .child(mono("select an issue from the board", FAINT)),
                    ),
                );
            };

            let freshness_line = match self.freshness(&issue) {
                Freshness::Fresh => (
                    format!(
                        "issue r{} matches the 32170 Work Record",
                        issue.revision.unwrap_or_default()
                    ),
                    GREEN,
                ),
                Freshness::IssueStale { issue: i, work } => (
                    format!("STALE — issue projection r{i} is behind Work Record r{work}"),
                    RED,
                ),
                Freshness::WorkStale { issue: i, work } => (
                    format!("Work Record r{work} is behind this projection r{i} — 32170 stale"),
                    AMBER,
                ),
                Freshness::NoWorkRecord => (
                    "no kind-32170 Work Record received for this d — freshness unverifiable"
                        .to_owned(),
                    AMBER,
                ),
                Freshness::NoRevision => (
                    "missing revision tags — freshness unverifiable".to_owned(),
                    AMBER,
                ),
            };

            let mut fields = v_flex()
                .px_2()
                .py_1p5()
                .gap_1()
                .child(key_value(
                    "work_ref",
                    issue.work_ref.clone(),
                    BODY,
                ))
                .child(key_value(
                    "state",
                    issue
                        .state
                        .clone()
                        .unwrap_or_else(|| "(no state tag)".into()),
                    SOFT_BLUE,
                ))
                .child(key_value(
                    "revision",
                    issue
                        .revision
                        .map(|revision| format!("r{revision}"))
                        .unwrap_or_else(|| "(none)".into()),
                    BODY,
                ))
                .child(key_value("freshness", freshness_line.0, freshness_line.1));
            if let Some(org) = &issue.org {
                fields = fields.child(key_value("org", org.clone(), SECONDARY));
            }
            if let Some(team) = &issue.team {
                fields = fields.child(key_value("team", team.clone(), SECONDARY));
            }
            if let Some(priority) = &issue.priority {
                fields = fields.child(key_value(
                    "priority",
                    priority.clone(),
                    priority_color(priority),
                ));
            }
            if let Some(estimate) = &issue.estimate {
                fields = fields.child(key_value("estimate", estimate.clone(), SECONDARY));
            }
            if let Some(due) = &issue.due {
                let display = due
                    .parse::<u64>()
                    .map(format_timestamp)
                    .unwrap_or_else(|_| due.clone());
                fields = fields.child(key_value("due", display, SECONDARY));
            }
            if let Some(published_at) = issue.published_at {
                fields = fields.child(key_value(
                    "published",
                    format_timestamp(published_at),
                    FAINT,
                ));
            }
            if let Some(head) = &issue.head {
                fields = fields.child(key_value("head", shorten(head), FAINT));
            }
            if !issue.content.is_empty() {
                let mut content = issue.content.clone();
                if content.len() > 600 {
                    content.truncate(600);
                    content.push('…');
                }
                fields = fields.child(key_value("content", content, SECONDARY));
            }

            let mut people = v_flex().px_2().py_1p5().gap_1();
            if issue.participants.is_empty() {
                people = people.child(mono("no p tags on this projection", FAINT));
            }
            for participant in &issue.participants {
                people = people.child(key_value(
                    match participant.marker.as_str() {
                        "assignee" => "assignee",
                        "delegate" => "delegate",
                        "subscriber" => "subscriber",
                        _ => "participant",
                    },
                    format!(
                        "{}{}",
                        shorten(&participant.pubkey),
                        if participant.marker.is_empty()
                            || ["assignee", "delegate", "subscriber"]
                                .contains(&participant.marker.as_str())
                        {
                            String::new()
                        } else {
                            format!(" · marker {:?} shown as-is", participant.marker)
                        }
                    ),
                    match participant.marker.as_str() {
                        "assignee" => GREEN,
                        "delegate" => CYAN,
                        _ => SECONDARY,
                    },
                ));
            }

            let mut refs = v_flex().px_2().py_1p5().gap_1();
            if issue.labels.is_empty() && issue.refs.is_empty() {
                refs = refs.child(mono("no label or planning refs", FAINT));
            }
            for label in &issue.labels {
                refs = refs.child(key_value("label", label.clone(), SOFT_BLUE));
            }
            for address in &issue.refs {
                refs = refs.child(key_value(
                    match address.marker.as_str() {
                        "project" => "project",
                        "cycle" => "cycle",
                        "milestone" => "milestone",
                        "release" => "release",
                        "parent" => "parent",
                        "child" => "child",
                        "relation" => "relation",
                        "document" => "document",
                        "need" => "need",
                        "session" => "session",
                        _ => "a-ref",
                    },
                    if address.marker.is_empty() {
                        address.coordinate.clone()
                    } else {
                        format!("{} ({})", address.coordinate, address.marker)
                    },
                    SECONDARY,
                ));
            }

            let mut unknown = v_flex().px_2().py_1p5().gap_1();
            if issue.unknown_tags.is_empty() {
                unknown = unknown.child(mono("none — every tag was recognized", FAINT));
            }
            for tag in &issue.unknown_tags {
                unknown = unknown.child(mono(format!("[{}]", tag.join(", ")), AMBER));
            }

            let _ = cx;
            v_flex()
                .gap_2()
                .child(
                    panel_frame(
                        "ISSUE DETAIL",
                        Some(if issue.fixture {
                            "FIXTURE DATA — unsigned spec example".into()
                        } else {
                            "verified authority-signed projection".into()
                        }),
                    )
                    .child(
                        v_flex()
                            .px_2()
                            .py_1p5()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Label::new(
                                            issue
                                                .identifier
                                                .clone()
                                                .unwrap_or_else(|| "(no identifier)".into()),
                                        )
                                        .size(LabelSize::Small)
                                        .weight(FontWeight::BOLD)
                                        .color(Color::Custom(rgb(AMBER).into())),
                                    )
                                    .child(
                                        Label::new(
                                            issue
                                                .title
                                                .clone()
                                                .unwrap_or_else(|| "(no title)".into()),
                                        )
                                        .size(LabelSize::Small)
                                        .color(Color::Custom(rgb(BODY).into())),
                                    ),
                            )
                            .child(fields),
                    ),
                )
                .child(panel_frame("PARTICIPANTS", None).child(people))
                .child(panel_frame("LABELS · PLANNING REFS", None).child(refs))
                .child(
                    panel_frame("UNKNOWN TAGS — PRESERVED, NEVER REINTERPRETED", None)
                        .child(unknown),
                )
                .child(self.render_timeline(&issue))
        }

        fn render_verification(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            let mut body = v_flex()
                .px_2()
                .py_1p5()
                .gap_1()
                .child(key_value(
                    "authority",
                    shorten(&self.config.authority),
                    if authority_is_placeholder(&self.config.authority) {
                        RED
                    } else {
                        BODY
                    },
                ))
                .child(key_value("relay", self.config.relay_url.clone(), SECONDARY))
                .child(key_value(
                    "verified",
                    format!("{} events (id sha256 + BIP-340 schnorr)", self.verified_events),
                    GREEN,
                ))
                .child(key_value(
                    "refused",
                    format!("{} events from non-authority keys", self.refused_foreign),
                    if self.refused_foreign > 0 { RED } else { FAINT },
                ))
                .child(key_value(
                    "invalid",
                    format!("{} malformed or badly signed events", self.invalid_events),
                    if self.invalid_events > 0 { AMBER } else { FAINT },
                ));
            if self.config.fixture {
                body = body.child(key_value(
                    "fixture",
                    "spec examples are unsigned — signature checks do not apply here".into(),
                    AMBER,
                ));
            }
            for line in &self.diagnostics {
                body = body.child(mono(line.clone(), FAINT));
            }
            panel_frame("VERIFICATION", Some("every rendered event verified".into())).child(body)
        }

        fn render_status_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            h_flex()
                .justify_between()
                .items_center()
                .px_3()
                .py_1()
                .bg(rgb(RAISED))
                .border_t_1()
                .border_color(rgb(AMBER_DIM))
                .child(mono(
                    "read-only projection of NIP-WK Work via NIP-PI Issues · this surface \
                     submits nothing and holds no keys",
                    FAINT,
                ))
                .child(mono(
                    "open source: OpenAgentsInc/openagents · immortal · omega",
                    FAINT,
                ))
        }
    }

    impl Render for WorkItems {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let _theme = cx.theme();
            let notices = self.render_notices(cx);
            div()
                .size_full()
                .bg(rgb(VOID))
                .overflow_hidden()
                .child(
                    v_flex()
                        .size_full()
                        .child(self.render_top_bar(cx))
                        .children(notices)
                        .child(
                            h_flex()
                                .flex_1()
                                .items_start()
                                .gap_2()
                                .p_2()
                                .overflow_hidden()
                                .child(
                                    v_flex()
                                        .w(px(460.))
                                        .flex_none()
                                        .h_full()
                                        .gap_2()
                                        .child(self.render_board(cx))
                                        .child(self.render_verification(cx)),
                                )
                                .child(
                                    div()
                                        .id("detail-scroll")
                                        .flex_1()
                                        .h_full()
                                        .overflow_y_scroll()
                                        .child(self.render_detail(cx)),
                                ),
                        )
                        .child(self.render_status_bar(cx)),
                )
        }
    }

    fn apply_terminal_theme(cx: &mut App) {
        let parsed: serde_json::Value = match serde_json::from_str(TERMINAL_JSON) {
            Ok(value) => value,
            Err(_) => return,
        };
        let Some(style) = parsed
            .get("themes")
            .and_then(|themes| themes.get(0))
            .and_then(|theme| theme.get("style"))
            .and_then(|style| style.as_object())
        else {
            return;
        };

        let get = |key: &str| -> Option<gpui::Hsla> {
            style
                .get(key)
                .and_then(|value| value.as_str())
                .and_then(hex_to_hsla)
        };

        let mut next = (*cx.theme().clone()).clone();
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

    fn hex_to_hsla(hex: &str) -> Option<gpui::Hsla> {
        let hex = hex.strip_prefix('#')?;
        let (rgb_part, alpha) = match hex.len() {
            6 => (hex, 0xff),
            8 => (&hex[..6], u8::from_str_radix(&hex[6..8], 16).ok()?),
            _ => return None,
        };
        let value = u32::from_str_radix(rgb_part, 16).ok()?;
        let rgba = gpui::Rgba {
            r: ((value >> 16) & 0xff) as f32 / 255.,
            g: ((value >> 8) & 0xff) as f32 / 255.,
            b: (value & 0xff) as f32 / 255.,
            a: alpha as f32 / 255.,
        };
        Some(rgba.into())
    }

    pub fn run() {
        gpui_platform::web_init();
        let handle = gpui_platform::application().run_embedded(|cx: &mut App| {
            theme::set_theme_settings_provider(
                Box::new(WebThemeSettings {
                    ui_font: gpui::font("Lilex"),
                    buffer_font: gpui::font("Lilex"),
                }),
                cx,
            );
            theme::init(theme::LoadThemes::JustBase, cx);
            apply_terminal_theme(cx);
            let bounds = Bounds::centered(None, size(px(1440.), px(880.)), cx);
            match cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(WorkItems::new),
            ) {
                Ok(_) => cx.activate(true),
                Err(error) => console::error_1(&JsValue::from_str(&format!(
                    "failed to open the work items demo: {error:#}"
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
    eprintln!("work_demo_web builds for wasm32-unknown-unknown")
}
