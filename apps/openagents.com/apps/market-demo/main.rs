#![cfg_attr(not(target_family = "wasm"), allow(dead_code, unused_imports))]

mod market_live;

#[cfg(target_family = "wasm")]
mod web_app {
    use std::time::Duration;

    use futures::{StreamExt as _, channel::mpsc};
    use gpui::prelude::*;
    use gpui::{
        App, Bounds, Context, Font, FontWeight, Pixels, Task, Window, WindowBounds, WindowOptions,
        div, px, rgb, size,
    };
    use immortal_client::mkt_swp_client::{
        Cancellation, MktSigningRequest, ParticipantRole, RequesterContractLocalInputs,
        RequesterContractSigningInput, RequesterExitPackageCommitment, RequesterOrderInput,
        StatusState, SwapClientConfig, SwapRecordFactory, SwapType,
    };
    use immortal_core::{
        domain::{
            Event as NostrEvent, MKT_CANCEL_KIND, MKT_CLOSE_KIND, MKT_QUOTE_KIND, MKT_STATUS_KIND,
            MKT_SWP_PROFILE_ID, MKT_SWP_PROFILE_VERSION, MKT_SWP_SWAP_CONTRACT_KIND,
            MktProfileSupport, Tag,
        },
        market::{MarketSigner, WrapMaterial, unwrap_mkt_record, wrap_mkt_record},
    };
    use serde_json::{Map, Value, json};
    use theme::{ActiveTheme as _, ThemeSettingsProvider, UiDensity};
    use ui::{
        Button, ButtonCommon as _, ButtonStyle, Clickable as _, Color, Indicator, Label,
        LabelCommon as _, LabelSize, h_flex, v_flex,
    };
    use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        CloseEvent, Event, MessageEvent, Request, RequestInit, Response, UrlSearchParams,
        WebSocket, console,
    };
    use web_time::{SystemTime, UNIX_EPOCH};

    use crate::market_live::{
        DiscoveryBook, DiscoveryFrame, SESSION_SUBSCRIPTION_ID, discovery_subscription,
        session_subscription,
    };

    const DEFAULT_RELAY_WS_URL: &str = "wss://relay.openagents.com";

    // Aiur palette (omega assets/themes/aiur/aiur.json).
    const VOID: u32 = 0x05070d; // editor.background — the page
    const PANEL: u32 = 0x0b1220; // background — panels
    const RAISED: u32 = 0x141f36; // element.background — headers, cards
    const HAIRLINE: u32 = 0x1f2b45; // border
    const RULE: u32 = 0x16203a; // border.variant — table row rules
    const RULE_FAINT: u32 = 0x121a2e; // dimmer rule for the dense event tape
    const ACCENT: u32 = 0x5c96f8; // terminal.ansi.bright_blue — primary accent
    const ACCENT_DIM: u32 = 0x2f6fe0; // terminal.ansi.dim_blue — quiet accent
    const ACCENT_STRONG: u32 = 0x3b82f6; // text.accent — filled emphasis
    const BODY: u32 = 0xeef3ff; // text
    const SECONDARY: u32 = 0xa9b1d6; // text.muted
    const FAINT: u32 = 0x5b6486; // dimmed muted
    const GREEN: u32 = 0x9ece6a; // terminal.ansi.green — pass / active
    const RED: u32 = 0xf7768e; // terminal.ansi.red — failure / refusal
    const ORANGE: u32 = 0xff9e64; // warning / custody / weak-guarantee accent
    const CYAN: u32 = 0x7dcfff; // terminal.ansi.cyan — protocol / transport
    const SOFT_BLUE: u32 = 0x8fb6ff; // claim rung — lighter than the accent
    const WARN_BG: u32 = 0x3a2415; // orange-tinted chip behind the DEMO banner
    const CALLOUT_BG: u32 = 0x0e1525; // accent-tinted closing callout
    const GAP_BG: u32 = 0x1d131b; // red-tinted sequence-gap row
    const GAP_BORDER: u32 = 0x4a2835; // red-tinted sequence-gap border

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

    // Aiur surface values applied over Omega's theme machinery.
    const AIUR_JSON: &str = r##"{
      "themes": [{
        "style": {
          "border": "#1f2b45ff",
          "border.variant": "#16203aff",
          "border.focused": "#5c96f8ff",
          "elevated_surface.background": "#141f36ff",
          "surface.background": "#0b1220ff",
          "background": "#05070dff",
          "panel.background": "#0b1220ff",
          "editor.background": "#05070dff",
          "element.background": "#141f36ff",
          "element.hover": "#5c96f814",
          "element.active": "#5c96f821",
          "element.selected": "#5c96f829",
          "ghost_element.hover": "#5c96f814",
          "ghost_element.selected": "#5c96f829",
          "text": "#eef3ffff",
          "text.muted": "#a9b1d6ff",
          "text.placeholder": "#5b6486ff",
          "text.disabled": "#3c4460ff",
          "text.accent": "#5c96f8ff",
          "icon": "#eef3ffff",
          "icon.muted": "#a9b1d6ff",
          "icon.accent": "#5c96f8ff",
          "status_bar.background": "#0b1220ff",
          "title_bar.background": "#0b1220ff",
          "toolbar.background": "#05070dff",
          "tab_bar.background": "#0b1220ff",
          "tab.active_background": "#05070dff",
          "tab.inactive_background": "#0b1220ff"
        }
      }]
    }"##;

    // ------------------------------------------------------------------
    // Demo data. Every market value is synthetic and labeled DEMO. Shapes
    // follow NIP-MKT v0.1 (kinds 39600-39609) and the MKT-SWP draft.
    // ------------------------------------------------------------------

    struct DemoProvider {
        name: &'static str,
    }

    const PROVIDERS: [DemoProvider; 3] = [
        DemoProvider { name: "aurora-lp" },
        DemoProvider {
            name: "meridian-swaps",
        },
        DemoProvider {
            name: "southpaw-liquidity",
        },
    ];

    struct PriceRow {
        pair: &'static str,
        rail: &'static str,
        base_bps: i64,
        bps: i64,
        delta: i64,
        providers: u32,
    }

    struct DemoQuote {
        provider: usize,
        kind: &'static str,
        reservation: &'static str,
        reservation_proof: &'static str,
        price: &'static str,
        fee: &'static str,
        expiry: &'static str,
        custody: &'static str,
    }

    const QUOTES: [DemoQuote; 2] = [
        DemoQuote {
            provider: 0,
            kind: "firm",
            reservation: "hard",
            reservation_proof: "provider-signed reserve (covenant class reserved)",
            price: "100,000 sat in → 99,610 sat out",
            fee: "39 bps · a fill promise, not a fact",
            expiry: "expires 45 s after issue (NIP-40)",
            custody: "funds: self · execution: script · exit: unilateral",
        },
        DemoQuote {
            provider: 1,
            kind: "indicative",
            reservation: "soft",
            reservation_proof: "no capacity committed — needs provider re-accept",
            price: "100,000 sat in → 99,655 sat out",
            fee: "34 bps · cheaper, nothing reserved",
            expiry: "expires 120 s after issue (NIP-40)",
            custody: "funds: self · execution: script · exit: unilateral",
        },
    ];

    const VERIFY_STEPS: [(&str, &str); 5] = [
        (
            "LOCK SCRIPT / TAPROOT TREE",
            "rebuilt locally; matches committed template",
        ),
        (
            "AMOUNTS",
            "outputs equal accepted Quote terms (decimal-string sats)",
        ),
        (
            "PAYMENT HASH",
            "SHA-256 preimage couples both legs; no substitute",
        ),
        (
            "TIMELOCKS",
            "refund CLTV clears the claim window with margin",
        ),
        (
            "CLAIM + REFUND PATHS",
            "cooperative key path and unilateral exit verified",
        ),
    ];

    struct TimelineRow {
        signer: &'static str,
        seq: Option<u32>,
        state: &'static str,
        rung: &'static str,
        rung_color: u32,
        detail: &'static str,
    }

    const TIMELINE: [TimelineRow; 6] = [
        TimelineRow {
            signer: "provider",
            seq: Some(0),
            state: "accepted",
            rung: "claim",
            rung_color: SOFT_BLUE,
            detail: "firm quote + conforming order",
        },
        TimelineRow {
            signer: "provider",
            seq: Some(1),
            state: "funding_observed",
            rung: "measured",
            rung_color: ORANGE,
            detail: "lock output seen; not verification",
        },
        TimelineRow {
            signer: "provider",
            seq: Some(2),
            state: "executing",
            rung: "claim",
            rung_color: SOFT_BLUE,
            detail: "paying the Lightning leg",
        },
        TimelineRow {
            signer: "provider",
            seq: None,
            state: "sequence gap",
            rung: "gap",
            rung_color: RED,
            detail: "seq 3 never arrived — shown, never papered over",
        },
        TimelineRow {
            signer: "provider",
            seq: Some(4),
            state: "completed",
            rung: "claim",
            rung_color: SOFT_BLUE,
            detail: "a claim until evidence upgrades it",
        },
        TimelineRow {
            signer: "requester",
            seq: Some(0),
            state: "completed",
            rung: "verified",
            rung_color: GREEN,
            detail: "preimage + claim path verified locally",
        },
    ];

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Stage {
        Market,
        Rfq,
        Quotes,
        Verify,
        Timeline,
        Closed,
    }

    impl Stage {
        fn index(self) -> usize {
            match self {
                Stage::Market => 0,
                Stage::Rfq => 1,
                Stage::Quotes => 2,
                Stage::Verify => 3,
                Stage::Timeline => 4,
                Stage::Closed => 5,
            }
        }
    }

    enum RelayProbe {
        Checking,
        Online { name: String, mkt: bool },
        Unreachable(String),
    }

    #[derive(Clone)]
    struct DemoConfig {
        relay_ws_url: String,
        relay_http_url: String,
        problem: Option<String>,
    }

    impl DemoConfig {
        fn from_location() -> Self {
            let mut relay_ws_url = DEFAULT_RELAY_WS_URL.to_owned();
            let mut problem = None;
            if let Some(window) = web_sys::window()
                && let Ok(search) = window.location().search()
                && let Ok(params) = UrlSearchParams::new_with_str(&search)
                && let Some(candidate) = params.get("relay")
            {
                if allowed_relay_url(&candidate) {
                    relay_ws_url = candidate;
                } else {
                    problem = Some(
                        "ignored ?relay override — only the public relay or loopback ws:// URLs are allowed"
                            .to_owned(),
                    );
                }
            }
            let relay_http_url = relay_http_url(&relay_ws_url)
                .unwrap_or_else(|| "https://relay.openagents.com".to_owned());
            Self {
                relay_ws_url,
                relay_http_url,
                problem,
            }
        }

        fn is_loopback(&self) -> bool {
            self.relay_ws_url.starts_with("ws://127.0.0.1:")
                || self.relay_ws_url.starts_with("ws://localhost:")
        }
    }

    fn allowed_relay_url(value: &str) -> bool {
        if value == DEFAULT_RELAY_WS_URL {
            return true;
        }
        if value.len() > 128 {
            return false;
        }
        let Some(authority) = value.strip_prefix("ws://") else {
            return false;
        };
        if authority
            .bytes()
            .any(|byte| matches!(byte, b'/' | b'?' | b'#'))
        {
            return false;
        }
        let Some((host, port)) = authority.rsplit_once(':') else {
            return false;
        };
        matches!(host, "127.0.0.1" | "localhost") && port.parse::<u16>().is_ok_and(|port| port != 0)
    }

    fn relay_http_url(value: &str) -> Option<String> {
        value
            .strip_prefix("wss://")
            .map(|authority| format!("https://{authority}"))
            .or_else(|| {
                value
                    .strip_prefix("ws://")
                    .map(|authority| format!("http://{authority}"))
            })
    }

    #[derive(Clone, PartialEq, Eq)]
    enum DiscoveryConnection {
        Connecting,
        Snapshotting,
        Live,
        Empty,
        Stale(String),
    }

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
                        RelayInput::Error(format!("failed to send discovery REQ: {error:?}")),
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

            let error = Closure::<dyn FnMut(Event)>::new(move |_event| {
                send_relay_input(
                    &sender,
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
                return Err("relay WebSocket is not open".to_owned());
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
                    "failed to close market relay socket: {error:?}"
                )));
            }
        }
    }

    fn send_relay_input(sender: &mpsc::UnboundedSender<RelayInput>, input: RelayInput) {
        if sender.unbounded_send(input).is_err() {
            console::warn_1(&JsValue::from_str(
                "market relay event arrived after the GPUI view closed",
            ));
        }
    }

    /// Where a tape line came from. Rendered as a fixed-width chip so the
    /// disclosure sits left of the free text and can never be truncated by a
    /// long line or a narrow window.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TapeOrigin {
        /// Seeded, locally generated market fiction.
        Demo,
        /// Observed from a real network response.
        Live,
        /// A message about this page itself. Not market data either way.
        Local,
    }

    impl TapeOrigin {
        fn label(self) -> &'static str {
            match self {
                TapeOrigin::Demo => "DEMO",
                TapeOrigin::Live => "LIVE",
                TapeOrigin::Local => "LOCAL",
            }
        }

        fn color(self) -> u32 {
            match self {
                TapeOrigin::Demo => ORANGE,
                TapeOrigin::Live => GREEN,
                TapeOrigin::Local => FAINT,
            }
        }
    }

    struct TapeEvent {
        clock: String,
        kind: &'static str,
        kind_color: u32,
        origin: TapeOrigin,
        text: String,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum LiveSessionPhase {
        AwaitingQuote,
        AwaitingProviderContract,
        AwaitingProviderStatus,
        AwaitingClose,
        Complete,
        Failed,
    }

    struct LiveNoSpendSession {
        signer: MarketSigner,
        factory: SwapRecordFactory,
        rfq: NostrEvent,
        order: Option<NostrEvent>,
        contract: Option<Value>,
        records: Vec<NostrEvent>,
        phase: LiveSessionPhase,
        detail: String,
    }

    pub struct MarketTerminal {
        config: DemoConfig,
        stage: Stage,
        selected_quote: Option<usize>,
        verify_done: usize,
        verifying: bool,
        timeline_shown: usize,
        relay: RelayProbe,
        discovery: DiscoveryBook,
        discovery_connection: DiscoveryConnection,
        discovery_relay: Option<BrowserRelay>,
        auth_challenge: Option<String>,
        reconnect_attempt: u32,
        live_session: Option<LiveNoSpendSession>,
        clock: String,
        prices: Vec<PriceRow>,
        tape: Vec<TapeEvent>,
        wraps_relayed: u64,
        sessions_closed: u64,
        rng: u64,
        _tasks: Vec<Task<()>>,
    }

    fn format_clock(now_secs: u64) -> String {
        let secs = now_secs % 86_400;
        format!(
            "{:02}:{:02}:{:02} UTC",
            secs / 3600,
            (secs % 3600) / 60,
            secs % 60
        )
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or(0)
    }

    fn shorten_text(value: &str, maximum: usize) -> String {
        let count = value.chars().count();
        if count <= maximum {
            return value.to_owned();
        }
        let keep = maximum.saturating_sub(1);
        format!("{}…", value.chars().take(keep).collect::<String>())
    }

    fn kind_label(kind: u16) -> &'static str {
        match kind {
            MKT_QUOTE_KIND => "QUOT",
            MKT_SWP_SWAP_CONTRACT_KIND => "CNTR",
            MKT_STATUS_KIND => "STAT",
            MKT_CANCEL_KIND => "CNCL",
            MKT_CLOSE_KIND => "CLSE",
            _ => "LIVE",
        }
    }

    fn set_page_title(title: &str) {
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            document.set_title(title);
        }
    }

    fn random_32() -> Result<[u8; 32], String> {
        let window = web_sys::window().ok_or_else(|| "browser window is absent".to_owned())?;
        let crypto = window
            .crypto()
            .map_err(|error| format!("WebCrypto is unavailable: {error:?}"))?;
        let js_bytes = js_sys::Uint8Array::new_with_length(32);
        crypto
            .get_random_values_with_js_u8_array(&js_bytes)
            .map_err(|error| format!("WebCrypto entropy failed: {error:?}"))?;
        let mut bytes = [0_u8; 32];
        js_bytes.copy_to(&mut bytes);
        Ok(bytes)
    }

    fn random_hex_32() -> Result<String, String> {
        Ok(random_32()?
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    fn random_market_signer() -> Result<MarketSigner, String> {
        for _ in 0..32 {
            let bytes = random_32()?;
            if let Ok(signer) = MarketSigner::from_secret_bytes(bytes) {
                return Ok(signer);
            }
        }
        Err("WebCrypto did not produce a valid requester key".to_owned())
    }

    fn random_wrap_material() -> Result<WrapMaterial, String> {
        let now = now_secs();
        let seal_nonce = random_32()?;
        let wrap_nonce = random_32()?;
        for _ in 0..32 {
            let wrap_secret = random_32()?;
            if MarketSigner::from_secret_bytes(wrap_secret).is_ok() {
                return Ok(WrapMaterial {
                    seal_created_at: now.saturating_sub(u64::from(seal_nonce[0]) * 10),
                    wrap_created_at: now.saturating_sub(u64::from(wrap_nonce[0]) * 10),
                    seal_nonce,
                    wrap_nonce,
                    wrap_secret,
                });
            }
        }
        Err("WebCrypto did not produce a valid one-time wrap key".to_owned())
    }

    fn swp_profiles() -> [MktProfileSupport<'static>; 1] {
        [MktProfileSupport {
            profile_id: MKT_SWP_PROFILE_ID,
            version: MKT_SWP_PROFILE_VERSION,
            critical_members: &[],
            understood_members: &[],
        }]
    }

    fn sign_request(
        request: MktSigningRequest,
        signer: &MarketSigner,
    ) -> Result<NostrEvent, String> {
        let event = signer.sign(
            request.created_at,
            request.kind,
            request.tags.clone(),
            request.content.clone(),
        );
        request
            .verify_signed(event)
            .map_err(|error| format!("request signature failed: {error}"))
    }

    fn no_spend_rfq_profile() -> Value {
        json!({
            "constraints":{
                "allowed_script_modes":["taproot-musig2-script-exit"],
                "asset_pair":[
                    "swp:1:bip122:00000000000000000000000000000000:btc:chain",
                    "swp:1:bip122:00000000000000000000000000000000:btc:lightning"
                ],
                "confirmation_policy":{
                    "minimum_confirmations":"1",
                    "rbf":"reject",
                    "reorg_safety_blocks":"6",
                    "replacement":"reject",
                    "zero_confirmation":"forbidden"
                },
                "desired_completion_time":2000,
                "firm_quote_required":true,
                "input_amount":"100000",
                "invoice_sha256":"b0a570bb4ee56b4c1a2dfa43e1238af4be827e9bee7b17dd5ab85e27f01fead6",
                "maximum_total_fee":"99000",
                "payment_hash":"96c772a829fb7c780410f1d85cf12a89e8b3c78c0bac5fb47f62758bf961ec30",
                "requester_public_keys":[{
                    "leg_id":"source",
                    "path":"refund",
                    "public_key":"716022efaca232dd8a7927619a9e5f1eb8f1c8b87436a52a03ae7e1239a1662a"
                }],
                "swap_type":"submarine"
            }
        })
    }

    fn record_profile(event: &NostrEvent) -> Result<Map<String, Value>, String> {
        immortal_core::domain::parse_json_without_duplicate_members(
            &event.content,
            "live MKT-SWP record",
        )?
        .get("mkt_swp")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| "live record has no MKT-SWP profile".to_owned())
    }

    fn tag_value<'a>(event: &'a NostrEvent, name: &str) -> Option<&'a str> {
        event
            .tags
            .iter()
            .find(|tag| tag.name() == Some(name))
            .and_then(|tag| tag.value())
    }

    impl MarketTerminal {
        pub fn new(cx: &mut Context<Self>) -> Self {
            let config = DemoConfig::from_location();
            let nip11_url = config.relay_http_url.clone();
            let probe = cx.spawn(async move |this, cx| {
                let probe = probe_relay(&nip11_url).await;
                this.update(cx, |this, cx| {
                    let line = match &probe {
                        RelayProbe::Online { name, mkt } => format!(
                            "NIP-11 OK — {name}{}",
                            if *mkt { " · nip-mkt advertised" } else { "" }
                        ),
                        RelayProbe::Unreachable(reason) => {
                            format!("NIP-11 probe failed — {reason}")
                        }
                        RelayProbe::Checking => "NIP-11 probe pending".to_owned(),
                    };
                    let origin = if matches!(&probe, RelayProbe::Online { .. }) {
                        TapeOrigin::Live
                    } else {
                        TapeOrigin::Local
                    };
                    this.push_tape("NIP11", CYAN, origin, line, cx);
                    this.relay = probe;
                    cx.notify();
                })
                .ok();
            });

            let ticker = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(1000))
                        .await;
                    let alive = this
                        .update(cx, |this, cx| {
                            this.tick(cx);
                        })
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
            });

            let mut terminal = Self {
                config,
                stage: Stage::Market,
                selected_quote: None,
                verify_done: 0,
                verifying: false,
                timeline_shown: 0,
                relay: RelayProbe::Checking,
                discovery: DiscoveryBook::default(),
                discovery_connection: DiscoveryConnection::Connecting,
                discovery_relay: None,
                auth_challenge: None,
                reconnect_attempt: 0,
                live_session: None,
                clock: format_clock(now_secs()),
                prices: vec![
                    PriceRow {
                        pair: "BTC / BTC-LN",
                        rail: "submarine",
                        base_bps: 39,
                        bps: 39,
                        delta: 0,
                        providers: 2,
                    },
                    PriceRow {
                        pair: "BTC-LN / BTC",
                        rail: "reverse",
                        base_bps: 34,
                        bps: 34,
                        delta: 0,
                        providers: 2,
                    },
                    PriceRow {
                        pair: "BTC / L-BTC",
                        rail: "chain",
                        base_bps: 25,
                        bps: 25,
                        delta: 0,
                        providers: 1,
                    },
                    PriceRow {
                        pair: "BTC / USDT*",
                        rail: "evm (reserved)",
                        base_bps: 52,
                        bps: 52,
                        delta: 0,
                        providers: 0,
                    },
                ],
                tape: vec![TapeEvent {
                    clock: format_clock(now_secs()),
                    kind: "BOOT",
                    kind_color: ACCENT,
                    // A local startup message, not market data and not a live feed.
                    origin: TapeOrigin::Local,
                    text: "terminal online — seeded DEMO market loaded".to_owned(),
                }],
                wraps_relayed: 4_182,
                sessions_closed: 137,
                rng: 0x9e37_79b9_7f4a_7c15,
                _tasks: vec![probe, ticker],
            };
            if let Some(problem) = terminal.config.problem.clone() {
                terminal.push_tape("CONFIG", RED, TapeOrigin::Local, problem, cx);
            }
            terminal.open_discovery(cx);
            terminal
        }

        fn open_discovery(&mut self, cx: &mut Context<Self>) {
            self.discovery.begin_snapshot();
            self.discovery_connection = DiscoveryConnection::Connecting;
            let request = discovery_subscription();
            let (sender, mut receiver) = mpsc::unbounded();
            match BrowserRelay::connect(&self.config.relay_ws_url, request, sender) {
                Ok(relay) => self.discovery_relay = Some(relay),
                Err(error) => {
                    self.discovery_connection = DiscoveryConnection::Stale(error.clone());
                    self.push_tape("SOCKET", RED, TapeOrigin::Local, error, cx);
                    self.schedule_reconnect(cx);
                    return;
                }
            }
            let task = cx.spawn(async move |this, cx| {
                while let Some(input) = receiver.next().await {
                    if this
                        .update(cx, |this, cx| this.handle_relay_input(input, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            });
            self._tasks.push(task);
        }

        fn schedule_reconnect(&mut self, cx: &mut Context<Self>) {
            self.reconnect_attempt = self.reconnect_attempt.saturating_add(1).min(8);
            let seconds = 1_u64 << self.reconnect_attempt.min(5);
            let task = cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_secs(seconds))
                    .await;
                this.update(cx, |this, cx| {
                    this.discovery_relay = None;
                    this.open_discovery(cx);
                    cx.notify();
                })
                .ok();
            });
            self._tasks.push(task);
        }

        fn handle_relay_input(&mut self, input: RelayInput, cx: &mut Context<Self>) {
            match input {
                RelayInput::Opened => {
                    self.reconnect_attempt = 0;
                    self.discovery_connection = DiscoveryConnection::Snapshotting;
                    self.push_tape(
                        "SOCKET",
                        CYAN,
                        TapeOrigin::Local,
                        format!("WebSocket opened — {}", self.config.relay_ws_url),
                        cx,
                    );
                }
                RelayInput::Text(text) => {
                    self.handle_session_frame(&text, cx);
                    match self.discovery.ingest_text(&text, now_secs()) {
                        Ok(DiscoveryFrame::Challenge(challenge)) => {
                            self.auth_challenge = Some(challenge);
                        }
                        Ok(DiscoveryFrame::Head {
                            kind,
                            address,
                            replaced,
                        }) => {
                            self.push_tape(
                                "HEAD",
                                ACCENT_DIM,
                                TapeOrigin::Live,
                                format!(
                                    "verified {} {}{}",
                                    kind.label(),
                                    shorten_text(&address, 34),
                                    if replaced { " (replacement)" } else { "" }
                                ),
                                cx,
                            );
                        }
                        Ok(DiscoveryFrame::EndOfStoredEvents) => {
                            let providers = self.discovery.provider_count();
                            let offerings = self.discovery.offering_count();
                            self.discovery_connection = if providers + offerings == 0 {
                                DiscoveryConnection::Empty
                            } else {
                                DiscoveryConnection::Live
                            };
                            self.push_tape(
                                "EOSE",
                                GREEN,
                                TapeOrigin::Live,
                                format!(
                                    "verified snapshot: {providers} provider heads, {offerings} offerings"
                                ),
                                cx,
                            );
                        }
                        Ok(DiscoveryFrame::Closed(reason)) => {
                            self.discovery_connection = DiscoveryConnection::Stale(reason.clone());
                            self.push_tape("CLOSED", RED, TapeOrigin::Local, reason, cx);
                        }
                        Ok(DiscoveryFrame::Notice(notice)) => {
                            self.push_tape(
                                "NOTICE",
                                ORANGE,
                                TapeOrigin::Local,
                                shorten_text(&notice, 120),
                                cx,
                            );
                        }
                        Ok(DiscoveryFrame::Ignored) => {}
                        Err(error) => {
                            self.push_tape(
                                "REFUSE",
                                RED,
                                TapeOrigin::Local,
                                shorten_text(&error, 120),
                                cx,
                            );
                        }
                    }
                }
                RelayInput::Closed { code, reason } => {
                    let detail = if reason.is_empty() {
                        format!("WebSocket closed ({code})")
                    } else {
                        format!("WebSocket closed ({code}): {}", shorten_text(&reason, 96))
                    };
                    self.discovery_connection = DiscoveryConnection::Stale(detail.clone());
                    self.push_tape("SOCKET", RED, TapeOrigin::Local, detail, cx);
                    self.schedule_reconnect(cx);
                }
                RelayInput::Error(error) => {
                    self.push_tape("SOCKET", RED, TapeOrigin::Local, error, cx);
                }
            }
            cx.notify();
        }

        fn start_live_no_spend(&mut self, cx: &mut Context<Self>) {
            if self.live_session.as_ref().is_some_and(|session| {
                !matches!(
                    session.phase,
                    LiveSessionPhase::Complete | LiveSessionPhase::Failed
                )
            }) {
                return;
            }
            let Some(challenge) = self.auth_challenge.clone() else {
                self.push_tape(
                    "LIVE",
                    RED,
                    TapeOrigin::Local,
                    "relay has not supplied a bounded NIP-42 challenge".to_owned(),
                    cx,
                );
                return;
            };
            let Some((provider, offering)) = self.discovery.no_spend_offering() else {
                self.push_tape(
                    "LIVE",
                    RED,
                    TapeOrigin::Local,
                    "no verified active no-spend provider Offering is available".to_owned(),
                    cx,
                );
                return;
            };
            set_page_title("Swap Demo — OpenAgents Markets");
            let provider_pubkey = provider.event.pubkey.clone();
            let offering_address = offering.address.clone();
            let start = (|| -> Result<LiveNoSpendSession, String> {
                let signer = random_market_signer()?;
                let session_id = random_hex_32()?;
                let config = SwapClientConfig {
                    session_id,
                    requester_pubkey: signer.pubkey().to_owned(),
                    provider_pubkey: provider_pubkey.clone(),
                    offering_address,
                    provider_route: None,
                };
                let factory = SwapRecordFactory::new(config)
                    .map_err(|error| format!("could not initialize requester engine: {error}"))?;
                let now = now_secs();
                let rfq = sign_request(
                    factory
                        .rfq(
                            now,
                            &random_hex_32()?,
                            now.saturating_add(300),
                            no_spend_rfq_profile(),
                        )
                        .map_err(|error| format!("could not construct live RFQ: {error}"))?,
                    &signer,
                )?;
                Ok(LiveNoSpendSession {
                    signer,
                    factory,
                    rfq: rfq.clone(),
                    order: None,
                    contract: None,
                    records: vec![rfq],
                    phase: LiveSessionPhase::AwaitingQuote,
                    detail: "authenticated; RFQ published; awaiting provider Quote".to_owned(),
                })
            })();
            let session = match start {
                Ok(session) => session,
                Err(error) => {
                    self.push_tape("LIVE", RED, TapeOrigin::Local, error, cx);
                    return;
                }
            };
            let auth = session.signer.sign(
                now_secs(),
                22_242,
                vec![
                    Tag::new(vec!["relay".into(), self.config.relay_ws_url.clone()]),
                    Tag::new(vec!["challenge".into(), challenge]),
                ],
                String::new(),
            );
            let auth_frame = json!(["AUTH", auth]).to_string();
            let subscription = session_subscription(session.signer.pubkey());
            let send_setup = self
                .discovery_relay
                .as_ref()
                .ok_or_else(|| "relay WebSocket is absent".to_owned())
                .and_then(|relay| relay.send(&auth_frame))
                .and_then(|()| {
                    self.discovery_relay
                        .as_ref()
                        .ok_or_else(|| "relay WebSocket is absent".to_owned())?
                        .send(&subscription)
                });
            if let Err(error) = send_setup {
                self.push_tape("LIVE", RED, TapeOrigin::Local, error, cx);
                return;
            }
            self.live_session = Some(session);
            let rfq = self
                .live_session
                .as_ref()
                .map(|session| session.rfq.clone());
            if let Some(rfq) = rfq {
                if let Err(error) = self.publish_live_record(&rfq) {
                    self.fail_live_session(error, cx);
                    return;
                }
                self.push_tape(
                    "RFQ",
                    CYAN,
                    TapeOrigin::Live,
                    format!(
                        "published verified no-spend RFQ {}",
                        shorten_text(&rfq.id, 18)
                    ),
                    cx,
                );
            }
            cx.notify();
        }

        fn publish_live_record(&self, event: &NostrEvent) -> Result<(), String> {
            let session = self
                .live_session
                .as_ref()
                .ok_or_else(|| "live session is absent".to_owned())?;
            let raw = serde_json::to_vec(event)
                .map_err(|error| format!("could not serialize live record: {error}"))?;
            let wrapped = wrap_mkt_record(
                &raw,
                &session.signer,
                &session.factory.config().provider_pubkey,
                random_wrap_material()?,
            )?;
            let frame = json!(["EVENT", wrapped.event]).to_string();
            self.discovery_relay
                .as_ref()
                .ok_or_else(|| "relay WebSocket is absent".to_owned())?
                .send(&frame)
        }

        fn handle_session_frame(&mut self, text: &str, cx: &mut Context<Self>) {
            if self.live_session.is_none() || text.len() > 512 * 1024 {
                return;
            }
            let value: Value = match immortal_core::domain::parse_json_without_duplicate_members(
                text,
                "live session relay frame",
            ) {
                Ok(value) => value,
                Err(_) => return,
            };
            let Some(fields) = value.as_array() else {
                return;
            };
            if fields.len() != 3
                || fields.first().and_then(Value::as_str) != Some("EVENT")
                || fields.get(1).and_then(Value::as_str) != Some(SESSION_SUBSCRIPTION_ID)
            {
                return;
            }
            let Some(wrap_value) = fields.get(2).cloned() else {
                return;
            };
            let wrap: NostrEvent = match serde_json::from_value(wrap_value) {
                Ok(wrap) => wrap,
                Err(error) => {
                    self.fail_live_session(format!("session wrap shape is invalid: {error}"), cx);
                    return;
                }
            };
            let delivered = if let Some(session) = self.live_session.as_ref() {
                unwrap_mkt_record(&wrap, &session.signer, &swp_profiles())
            } else {
                return;
            };
            let delivered = match delivered {
                Ok(delivered) => delivered,
                Err(error) => {
                    self.fail_live_session(format!("refused provider wrap: {error}"), cx);
                    return;
                }
            };
            let event = delivered.record().event().clone();
            let Some(expected) = self.live_session.as_ref() else {
                return;
            };
            if delivered.record().envelope().session_id != expected.factory.config().session_id {
                return;
            }
            if event.pubkey != expected.factory.config().provider_pubkey {
                self.fail_live_session(
                    "refused live session record from a non-provider signer".to_owned(),
                    cx,
                );
                return;
            }
            self.handle_provider_record(event, cx);
        }

        fn handle_provider_record(&mut self, event: NostrEvent, cx: &mut Context<Self>) {
            let Some(mut session) = self.live_session.take() else {
                return;
            };
            if session.records.iter().any(|record| record.id == event.id) {
                self.live_session = Some(session);
                return;
            }
            if session.records.len() >= 64 {
                session.phase = LiveSessionPhase::Failed;
                session.detail = "signed session record bound reached".to_owned();
                self.live_session = Some(session);
                return;
            }
            let event_kind = event.kind;
            let mut outgoing = Vec::new();
            let result = (|| -> Result<(), String> {
                match event.kind {
                    MKT_QUOTE_KIND if session.phase == LiveSessionPhase::AwaitingQuote => {
                        let now = now_secs().max(event.created_at.saturating_add(1));
                        let order = sign_request(
                            session
                                .factory
                                .requester_order(RequesterOrderInput {
                                    rfq: &session.rfq,
                                    quote: &event,
                                    created_at: now,
                                    observed_at: now_secs(),
                                    distinct: &random_hex_32()?,
                                    selection: None,
                                })
                                .map_err(|error| {
                                    format!("requester engine refused Quote: {error}")
                                })?,
                            &session.signer,
                        )?;
                        let status = sign_request(
                            session
                                .factory
                                .status(
                                    ParticipantRole::Requester,
                                    now.saturating_add(1),
                                    &random_hex_32()?,
                                    &order.id,
                                    StatusState {
                                        sequence: 0,
                                        previous: None,
                                        base_state: "awaiting_input",
                                        swp_state: "requester_verification_passed",
                                    },
                                    Map::new(),
                                )
                                .map_err(|error| {
                                    format!("could not construct requester Status: {error}")
                                })?,
                            &session.signer,
                        )?;
                        let mut local_inputs =
                            RequesterContractLocalInputs::for_swap_type(SwapType::Submarine);
                        local_inputs
                            .exit_package_commitments
                            .push(RequesterExitPackageCommitment {
                            participant_role: "requester".to_owned(),
                            leg_id: "source".to_owned(),
                            path: "refund".to_owned(),
                            package_mode: "presigned".to_owned(),
                            package_sha256:
                                "77abefe30c067cc2f46a9947c38c09a0f6bfd9aedff026fa3760ce1c319adb11"
                                    .to_owned(),
                        });
                        let contract = session
                            .factory
                            .requester_contract_draft(
                                &session.rfq,
                                &event,
                                &order,
                                now_secs(),
                                local_inputs,
                            )
                            .map_err(|error| {
                                format!("could not compose no-spend Contract: {error}")
                            })?;
                        let requester_contract = sign_request(
                            session
                                .factory
                                .requester_contract(RequesterContractSigningInput {
                                    rfq: &session.rfq,
                                    quote: &event,
                                    order: &order,
                                    order_observed_at: now_secs(),
                                    created_at: now.saturating_add(2),
                                    distinct: &random_hex_32()?,
                                    contract: contract.clone(),
                                })
                                .map_err(|error| {
                                    format!("requester engine refused Contract: {error}")
                                })?,
                            &session.signer,
                        )?;
                        session.records.push(event.clone());
                        session.records.push(order.clone());
                        session.records.push(status.clone());
                        session.records.push(requester_contract.clone());
                        session.order = Some(order.clone());
                        session.contract = Some(contract);
                        session.phase = LiveSessionPhase::AwaitingProviderContract;
                        session.detail =
                            "Quote verified; Order, requester Status, and no-spend Contract published"
                                .to_owned();
                        outgoing.extend([order, status, requester_contract]);
                    }
                    MKT_SWP_SWAP_CONTRACT_KIND
                        if session.phase == LiveSessionPhase::AwaitingProviderContract =>
                    {
                        let profile = record_profile(&event)?;
                        if profile.get("contract") != session.contract.as_ref() {
                            return Err(
                                "provider countersigned different Contract terms".to_owned()
                            );
                        }
                        session.records.push(event);
                        session.phase = LiveSessionPhase::AwaitingProviderStatus;
                        session.detail =
                            "provider countersigned the exact Contract; awaiting Status".to_owned();
                    }
                    MKT_STATUS_KIND
                        if session.phase == LiveSessionPhase::AwaitingProviderStatus =>
                    {
                        let profile = record_profile(&event)?;
                        if tag_value(&event, "seq") != Some("0")
                            || profile.get("swp_state").and_then(Value::as_str) != Some("accepted")
                        {
                            return Err(
                                "provider Status is not the expected accepted seq 0".to_owned()
                            );
                        }
                        let order = session
                            .order
                            .as_ref()
                            .ok_or_else(|| "live session has no Order".to_owned())?;
                        let cancel = sign_request(
                            session
                                .factory
                                .cancel(
                                    ParticipantRole::Requester,
                                    now_secs().max(event.created_at.saturating_add(1)),
                                    &random_hex_32()?,
                                    &order.id,
                                    Cancellation {
                                        action: "request",
                                        reason: "browser_no_spend_demo",
                                        request_id: None,
                                        accepted_id: None,
                                    },
                                    json!({"disposition":"no_funding_authorized"}),
                                )
                                .map_err(|error| {
                                    format!("could not construct no-spend Cancel: {error}")
                                })?,
                            &session.signer,
                        )?;
                        session.records.push(event);
                        session.records.push(cancel.clone());
                        session.phase = LiveSessionPhase::AwaitingClose;
                        session.detail =
                            "provider Status verified; cancellation requested before funding"
                                .to_owned();
                        outgoing.push(cancel);
                    }
                    MKT_CANCEL_KIND if session.phase == LiveSessionPhase::AwaitingClose => {
                        let action = tag_value(&event, "action")
                            .ok_or_else(|| "provider Cancel has no action".to_owned())?
                            .to_owned();
                        if !matches!(action.as_str(), "accepted" | "effective") {
                            return Err(format!("provider returned unexpected Cancel {action}"));
                        }
                        session.records.push(event);
                        session.detail = format!("provider cancellation {action}; awaiting Close");
                    }
                    MKT_CLOSE_KIND if session.phase == LiveSessionPhase::AwaitingClose => {
                        let profile = record_profile(&event)?;
                        let zero_spend = tag_value(&event, "outcome") == Some("cancelled")
                            && profile
                                .get("external_spend_effects")
                                .and_then(Value::as_u64)
                                == Some(0)
                            && profile
                                .get("loss_accounting")
                                .and_then(|loss| loss.get("input_committed"))
                                .and_then(Value::as_str)
                                == Some("0");
                        if !zero_spend {
                            return Err(
                                "provider Close does not prove exact zero-spend accounting"
                                    .to_owned(),
                            );
                        }
                        session.records.push(event);
                        session.phase = LiveSessionPhase::Complete;
                        session.detail =
                            "Close verified: cancelled, external_spend_effects=0, input_committed=0"
                                .to_owned();
                        set_page_title("Swap Demo — zero-spend verified");
                    }
                    _ => return Ok(()),
                }
                Ok(())
            })();
            if let Err(error) = result {
                session.phase = LiveSessionPhase::Failed;
                session.detail = error;
            }
            let detail = session.detail.clone();
            let phase = session.phase;
            self.live_session = Some(session);
            if phase == LiveSessionPhase::Failed {
                self.push_tape("LIVE", RED, TapeOrigin::Local, detail, cx);
                return;
            }
            self.push_tape(
                kind_label(event_kind),
                if phase == LiveSessionPhase::Complete {
                    GREEN
                } else {
                    CYAN
                },
                TapeOrigin::Live,
                detail,
                cx,
            );
            for record in outgoing {
                if let Err(error) = self.publish_live_record(&record) {
                    self.fail_live_session(error, cx);
                    return;
                }
            }
            cx.notify();
        }

        fn fail_live_session(&mut self, error: String, cx: &mut Context<Self>) {
            if let Some(session) = &mut self.live_session {
                session.phase = LiveSessionPhase::Failed;
                session.detail = error.clone();
            }
            self.push_tape("LIVE", RED, TapeOrigin::Local, error, cx);
            set_page_title("Swap Demo — no-spend session failed");
            cx.notify();
        }

        fn next_rng(&mut self) -> u64 {
            // Deterministic LCG: the walk is fake and reproducible on purpose.
            self.rng = self
                .rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.rng
        }

        fn tick(&mut self, cx: &mut Context<Self>) {
            self.clock = format_clock(now_secs());
            for index in 0..self.prices.len() {
                let roll = self.next_rng();
                let step = (roll % 5) as i64 - 2;
                let row = &mut self.prices[index];
                if row.providers == 0 {
                    continue;
                }
                let base = row.base_bps;
                let next = (row.bps + step).clamp(base - 6, base + 6);
                row.delta = next - row.bps;
                row.bps = next;
            }
            let roll = self.next_rng();
            if roll % 4 == 0 {
                self.wraps_relayed += 1 + (roll % 3);
                // Short enough to render whole in the tape column, so the DEMO
                // chip is never the part that gets clipped.
                let samples: [(&str, u32, &str); 4] = [
                    ("WRAP", SOFT_BLUE, "1059 wrap relayed"),
                    ("HEAD", ACCENT_DIM, "39601 head: aurora-lp"),
                    ("HEAD", ACCENT_DIM, "39600 beat: meridian"),
                    ("WRAP", SOFT_BLUE, "1059 wrap → npub1mer…"),
                ];
                let pick = &samples[(roll as usize / 7) % samples.len()];
                self.push_tape(pick.0, pick.1, TapeOrigin::Demo, pick.2.to_owned(), cx);
            }
            cx.notify();
        }

        fn push_tape(
            &mut self,
            kind: &'static str,
            kind_color: u32,
            origin: TapeOrigin,
            text: String,
            _cx: &mut Context<Self>,
        ) {
            self.tape.insert(
                0,
                TapeEvent {
                    clock: format_clock(now_secs()),
                    kind,
                    kind_color,
                    origin,
                    text,
                },
            );
            self.tape.truncate(24);
        }

        fn reset(&mut self, cx: &mut Context<Self>) {
            self.stage = Stage::Market;
            self.selected_quote = None;
            self.verify_done = 0;
            self.verifying = false;
            self.timeline_shown = 0;
            self.push_tape(
                "SESS",
                FAINT,
                TapeOrigin::Demo,
                "session reset".to_owned(),
                cx,
            );
            cx.notify();
        }

        fn start_verification(&mut self, cx: &mut Context<Self>) {
            if self.verifying {
                return;
            }
            self.stage = Stage::Verify;
            self.verify_done = 0;
            self.verifying = true;
            self.push_tape(
                "ORDR",
                GREEN,
                TapeOrigin::Demo,
                "order 39606 signed — quote terms committed by event id".to_owned(),
                cx,
            );
            cx.notify();
            let task = cx.spawn(async move |this, cx| {
                for step in 0..VERIFY_STEPS.len() {
                    cx.background_executor()
                        .timer(Duration::from_millis(650))
                        .await;
                    this.update(cx, |this, cx| {
                        this.verify_done = step + 1;
                        this.push_tape(
                            "VRFY",
                            GREEN,
                            TapeOrigin::Demo,
                            format!("{} — ok", VERIFY_STEPS[step].0.to_lowercase()),
                            cx,
                        );
                        cx.notify();
                    })
                    .ok();
                }
                this.update(cx, |this, cx| {
                    this.verifying = false;
                    cx.notify();
                })
                .ok();
            });
            self._tasks.push(task);
        }

        fn advance_timeline(&mut self, cx: &mut Context<Self>) {
            if self.timeline_shown < TIMELINE.len() {
                let row = &TIMELINE[self.timeline_shown];
                let line = match row.seq {
                    Some(seq) => format!("status 39607 seq {seq}: {}", row.state),
                    None => "status seq 3 missing — gap surfaced".to_owned(),
                };
                let color = if row.seq.is_none() { RED } else { SOFT_BLUE };
                self.push_tape("STAT", color, TapeOrigin::Demo, line, cx);
                self.timeline_shown += 1;
            }
            if self.timeline_shown == TIMELINE.len() {
                self.stage = Stage::Closed;
                self.sessions_closed += 1;
                self.push_tape(
                    "CLSE",
                    ACCENT,
                    TapeOrigin::Demo,
                    "close 39609 outcome=completed · public receipt 39603".to_owned(),
                    cx,
                );
            }
            cx.notify();
        }
    }

    // ------------------------------------------------------------------
    // Render helpers
    // ------------------------------------------------------------------

    fn mono(text: impl Into<gpui::SharedString>, color: u32) -> Label {
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
                            .color(Color::Custom(rgb(ACCENT).into())),
                    )
                    .child(mono(right.unwrap_or_default(), FAINT)),
            )
    }

    fn tag(text: &'static str, color: u32) -> impl IntoElement {
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
            .child(div().min_w(px(96.)).child(mono(key, FAINT)))
            .child(mono(value, value_color))
    }

    impl MarketTerminal {
        fn render_top_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            let (relay_dot, relay_text, relay_color) = match &self.discovery_connection {
                DiscoveryConnection::Connecting => {
                    (Color::Muted, "WS: CONNECTING".to_owned(), FAINT)
                }
                DiscoveryConnection::Snapshotting => {
                    (Color::Muted, "WS: VERIFYING SNAPSHOT".to_owned(), CYAN)
                }
                DiscoveryConnection::Live => (
                    Color::Success,
                    format!(
                        "WS: LIVE · {} VERIFIED HEADS",
                        self.discovery.provider_count() + self.discovery.offering_count()
                    ),
                    GREEN,
                ),
                DiscoveryConnection::Empty => (
                    Color::Success,
                    "WS: LIVE · VERIFIED EMPTY".to_owned(),
                    GREEN,
                ),
                DiscoveryConnection::Stale(_) => {
                    (Color::Warning, "WS: STALE / RECONNECTING".to_owned(), RED)
                }
            };
            h_flex()
                .justify_between()
                .items_center()
                .px_3()
                .py_1p5()
                .bg(rgb(RAISED))
                .border_b_1()
                .border_color(rgb(ACCENT_DIM))
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            Label::new("OPENAGENTS MARKETS")
                                .size(LabelSize::Small)
                                .weight(FontWeight::EXTRA_BOLD)
                                .color(Color::Custom(rgb(ACCENT).into())),
                        )
                        .child(mono("<OAMKT>", ACCENT_DIM))
                        .child(mono("NEGOTIATED SWAPS · NIP-MKT v0.1", SECONDARY)),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child(Indicator::dot().color(relay_dot))
                                .child(mono(relay_text, relay_color)),
                        )
                        .child(mono(self.clock.clone(), BODY))
                        .child(
                            div()
                                .px_2()
                                .py_0p5()
                                .bg(rgb(WARN_BG))
                                .border_1()
                                .border_color(rgb(ORANGE))
                                .child(mono("DEMO — NO FUNDS ON THIS SURFACE", ORANGE)),
                        ),
                )
        }

        fn render_stage_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
            const STAGES: [&str; 6] = [
                "1 MARKET",
                "2 RFQ",
                "3 QUOTES",
                "4 VERIFY",
                "5 SESSION",
                "6 CLOSE",
            ];
            let current = self.stage.index();
            let mut strip = h_flex().gap_1().items_center().px_3().py_1().bg(rgb(VOID));
            for (index, label) in STAGES.iter().enumerate() {
                let active = index == current;
                let reached = index <= current;
                strip = strip.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .bg(rgb(if active { ACCENT_STRONG } else { PANEL }))
                        .border_1()
                        .border_color(rgb(if reached { ACCENT_DIM } else { HAIRLINE }))
                        .child(
                            Label::new(*label)
                                .size(LabelSize::XSmall)
                                .weight(FontWeight::BOLD)
                                .color(Color::Custom(
                                    rgb(if active {
                                        VOID
                                    } else if reached {
                                        ACCENT
                                    } else {
                                        FAINT
                                    })
                                    .into(),
                                )),
                        ),
                );
            }
            strip.child(div().flex_1()).child(
                Button::new("reset-top", "RESET")
                    .style(ButtonStyle::Outlined)
                    .on_click(cx.listener(|this, _event, _window, cx| this.reset(cx))),
            )
        }

        fn render_market_watch(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            let mut rows = v_flex();
            rows = rows.child(
                h_flex()
                    .px_2()
                    .py_0p5()
                    .border_b_1()
                    .border_color(rgb(HAIRLINE))
                    .child(div().w(px(104.)).child(mono("PAIR", FAINT)))
                    .child(div().w(px(56.)).child(mono("SPREAD", FAINT)))
                    .child(div().w(px(40.)).child(mono("Δ", FAINT)))
                    .child(div().w(px(34.)).child(mono("LP", FAINT))),
            );
            for row in &self.prices {
                let (delta_text, delta_color) = if row.providers == 0 {
                    ("—".to_owned(), FAINT)
                } else if row.delta > 0 {
                    (format!("+{}", row.delta), RED)
                } else if row.delta < 0 {
                    (format!("{}", row.delta), GREEN)
                } else {
                    ("·".to_owned(), FAINT)
                };
                rows = rows.child(
                    h_flex()
                        .px_2()
                        .py_1()
                        .border_b_1()
                        .border_color(rgb(RULE))
                        .child(
                            div().w(px(104.)).child(
                                v_flex()
                                    .child(mono(row.pair, BODY))
                                    .child(mono(row.rail, FAINT)),
                            ),
                        )
                        .child(div().w(px(56.)).child(mono(
                            if row.providers == 0 {
                                "n/a".to_owned()
                            } else {
                                format!("{} bps", row.bps)
                            },
                            if row.providers == 0 { FAINT } else { ACCENT },
                        )))
                        .child(div().w(px(40.)).child(mono(delta_text, delta_color)))
                        .child(div().w(px(34.)).child(mono(
                            format!("{}", row.providers),
                            if row.providers == 0 { FAINT } else { SECONDARY },
                        ))),
                );
            }
            panel_frame("MARKET WATCH", Some("seeded · DEMO".into()))
                .child(rows)
                .child(div().px_2().py_1().child(mono(
                    "spread = best advertised fee; a fill promise, never a fact",
                    FAINT,
                )))
        }

        fn render_network(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            let relay_line = match &self.relay {
                RelayProbe::Checking => ("probing…".to_owned(), FAINT),
                RelayProbe::Online { name, mkt } => (
                    format!(
                        "{name}{}",
                        if *mkt {
                            " · nip-mkt"
                        } else {
                            " · nip-mkt off"
                        }
                    ),
                    GREEN,
                ),
                RelayProbe::Unreachable(reason) => (reason.clone(), RED),
            };
            let discovery = match &self.discovery_connection {
                DiscoveryConnection::Connecting => "WebSocket connecting".to_owned(),
                DiscoveryConnection::Snapshotting => "validating until EOSE".to_owned(),
                DiscoveryConnection::Live => format!(
                    "EOSE + live · {} profiles / {} offerings",
                    self.discovery.provider_count(),
                    self.discovery.offering_count()
                ),
                DiscoveryConnection::Empty => "EOSE · verified empty snapshot".to_owned(),
                DiscoveryConnection::Stale(reason) => {
                    format!("stale · {}", shorten_text(reason, 44))
                }
            };
            panel_frame("NETWORK", Some("HTTP + browser WebSocket".into())).child(
                v_flex()
                    .px_2()
                    .py_1()
                    .gap_1()
                    .child(key_value("NIP-11", relay_line.0, relay_line.1))
                    .child(key_value(
                        "endpoint",
                        self.config.relay_ws_url.clone(),
                        SECONDARY,
                    ))
                    .child(key_value("discovery", discovery, CYAN))
                    .child(key_value(
                        "wraps",
                        format!("{} relayed (DEMO counter)", self.wraps_relayed),
                        BODY,
                    ))
                    .child(key_value(
                        "sessions",
                        format!("{} closed (DEMO counter)", self.sessions_closed),
                        BODY,
                    ))
                    .child(key_value(
                        "custody",
                        "relay holds zero funds, keys, preimages".to_owned(),
                        SECONDARY,
                    ))
                    .child(key_value(
                        "law",
                        "relay acceptance proves transport only".to_owned(),
                        ACCENT_DIM,
                    )),
            )
        }

        fn render_providers(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            let mut rows = v_flex();
            rows = rows.child(
                h_flex()
                    .px_2()
                    .py_0p5()
                    .border_b_1()
                    .border_color(rgb(HAIRLINE))
                    .child(div().w(px(160.)).child(mono("SIGNED PROVIDER", FAINT)))
                    .child(div().w(px(64.)).child(mono("STATUS", FAINT)))
                    .child(div().w(px(48.)).child(mono("OFFR", FAINT)))
                    .child(div().w(px(72.)).child(mono("PROFILE", FAINT))),
            );
            for provider in self.discovery.providers() {
                let active = provider.status == "active";
                let claimed_name = provider
                    .content
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|name| format!("claimed: {}", shorten_text(name, 28)))
                    .unwrap_or_else(|| "no self-asserted name".to_owned());
                let offerings = self.discovery.offerings_for_provider(&provider.address);
                rows = rows.child(
                    h_flex()
                        .px_2()
                        .py_1()
                        .border_b_1()
                        .border_color(rgb(RULE))
                        .child(
                            div().w(px(160.)).child(
                                v_flex()
                                    .child(mono(provider.distinct.clone(), BODY))
                                    .child(mono(
                                        format!(
                                            "{} · published {}",
                                            provider.short_pubkey(),
                                            provider.published_at
                                        ),
                                        FAINT,
                                    ))
                                    .child(mono(claimed_name, FAINT)),
                            ),
                        )
                        .child(
                            div().w(px(64.)).child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(Indicator::dot().color(if active {
                                        Color::Success
                                    } else {
                                        Color::Warning
                                    }))
                                    .child(mono(
                                        provider.status.clone(),
                                        if active { GREEN } else { ORANGE },
                                    )),
                            ),
                        )
                        .child(
                            div()
                                .w(px(48.))
                                .child(mono(offerings.to_string(), SECONDARY)),
                        )
                        .child(
                            div()
                                .w(px(72.))
                                .child(mono(shorten_text(&provider.profile_label(), 16), CYAN)),
                        ),
                );
            }
            if self.discovery.provider_count() == 0 {
                let message = match &self.discovery_connection {
                    DiscoveryConnection::Connecting | DiscoveryConnection::Snapshotting => {
                        "No snapshot committed — validating signed heads until EOSE"
                    }
                    DiscoveryConnection::Empty => {
                        "EOSE received: the relay returned zero verified provider heads"
                    }
                    DiscoveryConnection::Stale(_) => {
                        "No retained verified heads; relay connection is stale"
                    }
                    DiscoveryConnection::Live => "No verified provider heads",
                };
                rows = rows.child(div().p_2().child(mono(message, FAINT)));
            }
            let right = match &self.discovery_connection {
                DiscoveryConnection::Live => format!(
                    "{} heads · {} offerings",
                    self.discovery.provider_count(),
                    self.discovery.offering_count()
                ),
                DiscoveryConnection::Empty => "EOSE · empty".to_owned(),
                DiscoveryConnection::Stale(_) => "stale".to_owned(),
                DiscoveryConnection::Connecting | DiscoveryConnection::Snapshotting => {
                    "pending EOSE".to_owned()
                }
            };
            panel_frame("VERIFIED LIVE DISCOVERY", Some(right)).child(rows)
        }

        fn render_tape(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            let mut rows = v_flex();
            for event in &self.tape {
                rows = rows.child(
                    h_flex()
                        .px_2()
                        .py_0p5()
                        .gap_2()
                        .items_start()
                        .border_b_1()
                        .border_color(rgb(RULE_FAINT))
                        .child(
                            div()
                                .min_w(px(72.))
                                .flex_none()
                                .child(mono(event.clock.clone(), FAINT)),
                        )
                        .child(
                            div()
                                .min_w(px(38.))
                                .flex_none()
                                .child(mono(event.kind, event.kind_color)),
                        )
                        // Origin chip. Fixed column, left of the free text, so
                        // the disclosure survives any line length or window width.
                        .child(
                            div()
                                .min_w(px(40.))
                                .flex_none()
                                .child(mono(event.origin.label(), event.origin.color())),
                        )
                        .child(mono(event.text.clone(), SECONDARY)),
                );
            }
            panel_frame("EVENT TAPE", Some("newest first".into())).child(
                div()
                    .id("tape-scroll")
                    .h(px(240.))
                    .overflow_y_scroll()
                    .child(rows),
            )
        }

        fn render_live_no_spend(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let ready = matches!(&self.discovery_connection, DiscoveryConnection::Live)
                && self.discovery.no_spend_offering().is_some()
                && self.auth_challenge.is_some();
            let mut body = v_flex().p_2().gap_1();
            body = body.child(mono(
                "LIVE proof: an in-browser throwaway requester exchanges signed NIP-59 records with immortal-provider --no-spend, then cancels before funding. Completion requires the provider Close to state external_spend_effects=0 and input_committed=0.",
                SECONDARY,
            ));
            if let Some(session) = &self.live_session {
                let (phase, color) = match session.phase {
                    LiveSessionPhase::AwaitingQuote => ("awaiting Quote", CYAN),
                    LiveSessionPhase::AwaitingProviderContract => {
                        ("awaiting provider Contract", CYAN)
                    }
                    LiveSessionPhase::AwaitingProviderStatus => ("awaiting provider Status", CYAN),
                    LiveSessionPhase::AwaitingClose => ("awaiting zero-spend Close", ORANGE),
                    LiveSessionPhase::Complete => ("complete · zero-spend verified", GREEN),
                    LiveSessionPhase::Failed => ("failed closed", RED),
                };
                body = body
                    .child(key_value("state", phase.to_owned(), color))
                    .child(key_value(
                        "records",
                        format!("{} signed records", session.records.len()),
                        BODY,
                    ))
                    .child(key_value("evidence", session.detail.clone(), color));
            } else if let Some((provider, offering)) = self.discovery.no_spend_offering() {
                body = body
                    .child(key_value(
                        "provider",
                        format!("{} · {}", provider.distinct, provider.short_pubkey()),
                        GREEN,
                    ))
                    .child(key_value(
                        "offering",
                        format!("{} · provider-signed claim", offering.distinct),
                        CYAN,
                    ));
            } else {
                body = body.child(key_value(
                    "provider",
                    "no verified active mode=no_spend Offering in the committed snapshot"
                        .to_owned(),
                    ORANGE,
                ));
            }
            let can_start = self.live_session.as_ref().is_none_or(|session| {
                matches!(
                    session.phase,
                    LiveSessionPhase::Complete | LiveSessionPhase::Failed
                )
            });
            if ready && can_start {
                body = body.child(
                    h_flex().pt_1().child(
                        Button::new("run-live-no-spend", "RUN LIVE NO-SPEND SESSION")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.start_live_no_spend(cx);
                            })),
                    ),
                );
            } else if self.config.is_loopback() && self.live_session.is_none() {
                body = body.child(mono(
                    "Start scripts/dev-relay.sh and immortal-provider --no-spend, then reload after the signed Offering reaches EOSE.",
                    FAINT,
                ));
            } else if self.live_session.is_none() {
                body = body.child(mono(
                    "The public relay has no verified no-spend provider right now. Use ?relay=ws://127.0.0.1:18080 under the local Trunk preview for the developer proof.",
                    FAINT,
                ));
            }
            panel_frame(
                "LIVE NO-SPEND PROVIDER PROOF",
                Some(
                    if self.config.is_loopback() {
                        "loopback relay"
                    } else {
                        "public relay"
                    }
                    .to_owned(),
                ),
            )
            .child(body)
        }

        fn render_session_market(&self, cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .px_3()
                .py_2()
                .gap_2()
                .child(mono(
                    "The walkthrough below is scripted DEMO data. The separate live proof exchanges signed records with the verified no-spend provider; this walkthrough keeps the gap, fork, and rung laws visible even when the relay snapshot is empty.",
                    SECONDARY,
                ))
                .child(
                    v_flex()
                        .gap_1()
                        .child(key_value(
                            "offering",
                            "BTC (on-chain) → BTC (Lightning) · submarine".into(),
                            BODY,
                        ))
                        .child(key_value(
                            "identity",
                            "id pair btc / btc-ln — tickers are labels, never identity".into(),
                            FAINT,
                        ))
                        .child(key_value(
                            "bounds",
                            "10,000 – 2,000,000 sat (decimal-string atomic units)".into(),
                            SECONDARY,
                        ))
                        .child(key_value("profile", "mkt-swp v1 (draft)".into(), CYAN)),
                )
                .child(
                    h_flex().pt_1().child(
                        Button::new("send-rfq", "SEND DEMO RFQ · 100,000 SAT →")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.stage = Stage::Rfq;
                                this.push_tape(
                                    "RFQ",
                                    CYAN,
                                    TapeOrigin::Demo,
                                    "rfq 39604 sealed in per-recipient 1059 wraps".to_owned(),
                                    cx,
                                );
                                cx.notify();
                            })),
                    ),
                )
        }

        fn render_session_rfq(&self, cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .px_3()
                .py_2()
                .gap_2()
                .child(mono(
                    "PRIVATE RFQ (KIND 39604) — SENT. A fully signed event sealed inside \
                     a NIP-59 gift wrap per recipient. Relays see an opaque wrap: no \
                     amounts, pairs, or counterparties leak, and neither provider learns \
                     who else was asked.",
                    BODY,
                ))
                .child(
                    v_flex()
                        .gap_1()
                        .child(key_value(
                            "session",
                            "d41c…9a03 (random 32-byte id)".into(),
                            FAINT,
                        ))
                        .child(key_value("amount", "100,000 sat (DEMO)".into(), SECONDARY))
                        .child(key_value(
                            "recipients",
                            "aurora-lp · meridian-swaps (separate wraps)".into(),
                            SECONDARY,
                        ))
                        .child(key_value(
                            "expiry",
                            "NIP-40 set — expiry never implies consent".into(),
                            FAINT,
                        )),
                )
                .child(
                    h_flex().pt_1().child(
                        Button::new("recv-quotes", "BOTH PROVIDERS ANSWERED — QUOTES →")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.stage = Stage::Quotes;
                                this.push_tape(
                                    "QUOT",
                                    GREEN,
                                    TapeOrigin::Demo,
                                    "quote 39605 firm/hard from aurora-lp".to_owned(),
                                    cx,
                                );
                                this.push_tape(
                                    "QUOT",
                                    ORANGE,
                                    TapeOrigin::Demo,
                                    "quote 39605 indicative/soft from meridian-swaps".to_owned(),
                                    cx,
                                );
                                cx.notify();
                            })),
                    ),
                )
        }

        fn render_session_quotes(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let mut list = v_flex().gap_2();
            for (index, quote) in QUOTES.iter().enumerate() {
                let provider = &PROVIDERS[quote.provider];
                let firm = quote.kind == "firm";
                list = list.child(
                    v_flex()
                        .gap_1()
                        .p_2()
                        .bg(rgb(RAISED))
                        .border_1()
                        .border_color(rgb(if firm { ACCENT_DIM } else { HAIRLINE }))
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Label::new(provider.name)
                                                .size(LabelSize::XSmall)
                                                .weight(FontWeight::BOLD)
                                                .color(Color::Custom(rgb(BODY).into())),
                                        )
                                        .child(tag(quote.kind, if firm { GREEN } else { ORANGE }))
                                        .child(tag(
                                            quote.reservation,
                                            if firm { GREEN } else { ORANGE },
                                        )),
                                )
                                .child(
                                    Button::new(("accept", index), "ACCEPT → ORDER")
                                        .style(if firm {
                                            ButtonStyle::Filled
                                        } else {
                                            ButtonStyle::Outlined
                                        })
                                        .on_click(cx.listener(move |this, _event, _window, cx| {
                                            this.selected_quote = Some(index);
                                            this.start_verification(cx);
                                        })),
                                ),
                        )
                        .child(key_value("terms", quote.price.into(), BODY))
                        .child(key_value("fee", quote.fee.into(), SECONDARY))
                        .child(key_value("resv", quote.reservation_proof.into(), SECONDARY))
                        .child(key_value("custody", quote.custody.into(), FAINT))
                        .child(key_value("expiry", quote.expiry.into(), FAINT)),
                );
            }
            v_flex()
                .px_3()
                .py_2()
                .gap_2()
                .child(mono(
                    "Signed expiring Quotes (39605). The event id commits exact terms — \
                     an Order references that id and cannot restate a different price and \
                     call it acceptance.",
                    SECONDARY,
                ))
                .child(list)
        }

        fn render_session_verify(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let mut checklist = v_flex().gap_1();
            for (index, (name, detail)) in VERIFY_STEPS.iter().enumerate() {
                let done = index < self.verify_done;
                checklist = checklist.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(if done {
                            Indicator::dot().color(Color::Success)
                        } else {
                            Indicator::dot().color(Color::Muted)
                        })
                        .child(
                            v_flex()
                                .child(mono(*name, if done { GREEN } else { FAINT }))
                                .child(mono(*detail, FAINT)),
                        ),
                );
            }
            let complete = self.verify_done == VERIFY_STEPS.len() && !self.verifying;
            v_flex()
                .px_3()
                .py_2()
                .gap_2()
                .child(mono(
                    "SCRIPTED DEMO CHECKLIST — illustrates verify-before-fund. These rows are not live funding authorization and this surface holds no funds.",
                    BODY,
                ))
                .child(checklist)
                .child(if complete {
                    h_flex().pt_1().child(
                        Button::new("fund", "DEMO CHECKLIST COMPLETE — SIMULATE SESSION →")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.stage = Stage::Timeline;
                                this.timeline_shown = 0;
                                this.advance_timeline(cx);
                            })),
                    )
                } else {
                    h_flex().pt_1().child(mono("verifying…", ACCENT))
                })
        }

        fn render_session_timeline(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let mut rows = v_flex().gap_1();
            for row in TIMELINE.iter().take(self.timeline_shown) {
                let is_gap = row.seq.is_none();
                rows = rows.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .px_2()
                        .py_1()
                        .bg(rgb(if is_gap { GAP_BG } else { RAISED }))
                        .border_1()
                        .border_color(rgb(if is_gap { GAP_BORDER } else { HAIRLINE }))
                        .child(div().min_w(px(70.)).child(mono(row.signer, FAINT)))
                        .child(div().min_w(px(44.)).child(mono(
                            match row.seq {
                                Some(seq) => format!("seq {seq}"),
                                None => "seq 3?".to_owned(),
                            },
                            if is_gap { RED } else { SECONDARY },
                        )))
                        .child(
                            div().min_w(px(122.)).child(
                                Label::new(row.state)
                                    .size(LabelSize::XSmall)
                                    .weight(FontWeight::BOLD)
                                    .color(Color::Custom(
                                        rgb(if is_gap { RED } else { BODY }).into(),
                                    )),
                            ),
                        )
                        .child(tag(row.rung, row.rung_color))
                        .child(mono(row.detail, FAINT)),
                );
            }
            let done = self.timeline_shown >= TIMELINE.len();
            v_flex()
                .px_3()
                .py_2()
                .gap_2()
                .child(mono(
                    "Status records (39607) carry a dense per-signer sequence. A missing \
                     number is a displayed gap; a duplicate would render as a fork. No \
                     state implies the next rung of proof.",
                    SECONDARY,
                ))
                .child(rows)
                .child(if done {
                    h_flex().child(mono("session terminal — close below", ACCENT))
                } else {
                    h_flex().child(
                        Button::new("advance", "NEXT STATUS RECORD →")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.advance_timeline(cx);
                            })),
                    )
                })
        }

        fn render_session_close(&self, cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .gap_2()
                .child(self.render_session_timeline(cx))
                .child(
                    v_flex()
                        .mx_3()
                        .gap_1()
                        .p_2()
                        .bg(rgb(RAISED))
                        .border_1()
                        .border_color(rgb(HAIRLINE))
                        .child(mono("DEMO CLOSE (39609) + PUBLIC RECEIPT (39603)", ACCENT))
                        .child(key_value("outcome", "completed".into(), GREEN))
                        .child(key_value(
                            "close",
                            "each party signs its own terminal reconciliation — \
                             agreement is not inferred, disagreement is not hidden"
                                .into(),
                            SECONDARY,
                        ))
                        .child(key_value(
                            "receipt",
                            "redacted + consented: outcome and close ref only".into(),
                            SECONDARY,
                        ))
                        .child(key_value(
                            "rung",
                            "still a signer's claim — settled needs exact external proof".into(),
                            ORANGE,
                        )),
                )
                .child(
                    div()
                        .mx_3()
                        .p_2()
                        .bg(rgb(CALLOUT_BG))
                        .border_1()
                        .border_color(rgb(ACCENT))
                        .child(mono(
                            "Boltz and Satora went dark because one company's API was the \
                             market. This market is signed events on relays: any provider \
                             can join, any client can verify, and this terminal is just \
                             one window onto it.",
                            ACCENT,
                        )),
                )
                .child(
                    h_flex().px_3().pb_2().child(
                        Button::new("reset", "RUN THE SESSION AGAIN")
                            .style(ButtonStyle::Outlined)
                            .on_click(cx.listener(|this, _event, _window, cx| this.reset(cx))),
                    ),
                )
        }

        fn render_session_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let body: gpui::AnyElement = match self.stage {
                Stage::Market => self.render_session_market(cx).into_any_element(),
                Stage::Rfq => self.render_session_rfq(cx).into_any_element(),
                Stage::Quotes => self.render_session_quotes(cx).into_any_element(),
                Stage::Verify => self.render_session_verify(cx).into_any_element(),
                Stage::Timeline => self.render_session_timeline(cx).into_any_element(),
                Stage::Closed => self.render_session_close(cx).into_any_element(),
            };
            let subtitle = match self.stage {
                Stage::Market => "stage 1 · discovery",
                Stage::Rfq => "stage 2 · private rfq",
                Stage::Quotes => "stage 3 · competing quotes",
                Stage::Verify => "stage 4 · verify before fund",
                Stage::Timeline => "stage 5 · session",
                Stage::Closed => "stage 6 · close",
            };
            panel_frame(
                "SCRIPTED DEMO SESSION · KINDS 39604-39609",
                Some(subtitle.into()),
            )
            .child(body)
        }

        fn render_status_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            h_flex()
                .justify_between()
                .items_center()
                .px_3()
                .py_1()
                .bg(rgb(RAISED))
                .border_t_1()
                .border_color(rgb(ACCENT_DIM))
                .child(mono(
                    "scripted session is DEMO · discovery and no-spend proof are signed live data · relay acceptance is transport only",
                    FAINT,
                ))
                .child(mono(
                    "open source: OpenAgentsInc/openagents · immortal · omega",
                    FAINT,
                ))
        }
    }

    impl Render for MarketTerminal {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let _theme = cx.theme();
            div().size_full().bg(rgb(VOID)).overflow_hidden().child(
                v_flex()
                    .size_full()
                    .child(self.render_top_bar(cx))
                    .child(self.render_stage_strip(cx))
                    .child(
                        div()
                            .id("terminal-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(
                                h_flex()
                                    .items_start()
                                    .gap_2()
                                    .p_2()
                                    .child(
                                        v_flex()
                                            .w(px(268.))
                                            .flex_none()
                                            .gap_2()
                                            .child(self.render_market_watch(cx))
                                            .child(self.render_network(cx)),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w(px(430.))
                                            .gap_2()
                                            .child(self.render_live_no_spend(cx))
                                            .child(self.render_session_panel(cx)),
                                    )
                                    .child(
                                        v_flex()
                                            .w(px(354.))
                                            .flex_none()
                                            .gap_2()
                                            .child(self.render_providers(cx))
                                            .child(self.render_tape(cx)),
                                    ),
                            ),
                    )
                    .child(self.render_status_bar(cx)),
            )
        }
    }

    async fn probe_relay(relay_url: &str) -> RelayProbe {
        let Some(window) = web_sys::window() else {
            return RelayProbe::Unreachable("no browser window".to_owned());
        };
        let options = RequestInit::new();
        options.set_method("GET");
        let request = match Request::new_with_str_and_init(relay_url, &options) {
            Ok(request) => request,
            Err(error) => {
                return RelayProbe::Unreachable(format!("request build failed: {error:?}"));
            }
        };
        if request
            .headers()
            .set("Accept", "application/nostr+json")
            .is_err()
        {
            return RelayProbe::Unreachable("could not set NIP-11 media type".to_owned());
        }
        let response = match JsFuture::from(window.fetch_with_request(&request)).await {
            Ok(response) => response,
            Err(_) => return RelayProbe::Unreachable("fetch failed".to_owned()),
        };
        let Ok(response) = response.dyn_into::<Response>() else {
            return RelayProbe::Unreachable("no HTTP response".to_owned());
        };
        if !response.ok() {
            return RelayProbe::Unreachable(format!("HTTP {}", response.status()));
        }
        let Ok(text_promise) = response.text() else {
            return RelayProbe::Unreachable("unreadable body".to_owned());
        };
        let Ok(text) = JsFuture::from(text_promise).await else {
            return RelayProbe::Unreachable("unreadable body".to_owned());
        };
        let Some(text) = text.as_string() else {
            return RelayProbe::Unreachable("body was not text".to_owned());
        };
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(info) => {
                let name = info
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("relay.openagents.com")
                    .to_owned();
                let mkt = info
                    .get("supported_extensions")
                    .and_then(|value| value.as_array())
                    .is_some_and(|extensions| {
                        extensions
                            .iter()
                            .any(|value| value.as_str() == Some("nip-mkt"))
                    });
                RelayProbe::Online { name, mkt }
            }
            Err(_) => RelayProbe::Unreachable("NIP-11 document was not JSON".to_owned()),
        }
    }

    fn apply_aiur_theme(cx: &mut App) {
        let parsed: serde_json::Value = match serde_json::from_str(AIUR_JSON) {
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
            apply_aiur_theme(cx);
            let bounds = Bounds::centered(None, size(px(1440.), px(880.)), cx);
            match cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(MarketTerminal::new),
            ) {
                Ok(_) => cx.activate(true),
                Err(error) => console::error_1(&JsValue::from_str(&format!(
                    "failed to open the market terminal: {error:#}"
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
    eprintln!("market_demo_web builds for wasm32-unknown-unknown")
}
