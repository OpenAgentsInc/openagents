#![cfg_attr(not(target_family = "wasm"), allow(dead_code, unused_imports))]

#[cfg(target_family = "wasm")]
mod web_app {
    use std::time::Duration;

    use gpui::prelude::*;
    use gpui::{
        App, Bounds, Context, Font, FontWeight, Pixels, Task, Window, WindowBounds, WindowOptions,
        div, px, rgb, size,
    };
    use theme::{ActiveTheme as _, ThemeSettingsProvider, UiDensity};
    use ui::{
        Button, ButtonCommon as _, ButtonStyle, Clickable as _, Color, Indicator, Label,
        LabelCommon as _, LabelSize, h_flex, v_flex,
    };
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, Response, console};
    use web_time::{SystemTime, UNIX_EPOCH};

    const RELAY_URL: &str = "https://relay.openagents.com";

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
        short_key: &'static str,
        status: &'static str,
        class: &'static str,
        reservation: &'static str,
    }

    const PROVIDERS: [DemoProvider; 3] = [
        DemoProvider {
            name: "aurora-lp",
            short_key: "npub1aur…9f2k",
            status: "active",
            class: "firm",
            reservation: "hard",
        },
        DemoProvider {
            name: "meridian-swaps",
            short_key: "npub1mer…x0q4",
            status: "active",
            class: "indicative",
            reservation: "soft",
        },
        DemoProvider {
            name: "southpaw-liquidity",
            short_key: "npub1sth…77vd",
            status: "paused",
            class: "—",
            reservation: "—",
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

    struct TapeEvent {
        clock: String,
        kind: &'static str,
        kind_color: u32,
        text: String,
    }

    pub struct MarketTerminal {
        stage: Stage,
        selected_quote: Option<usize>,
        verify_done: usize,
        verifying: bool,
        timeline_shown: usize,
        relay: RelayProbe,
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

    impl MarketTerminal {
        pub fn new(cx: &mut Context<Self>) -> Self {
            let probe = cx.spawn(async move |this, cx| {
                let probe = probe_relay().await;
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
                    this.push_tape("RELAY", CYAN, line, cx);
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

            Self {
                stage: Stage::Market,
                selected_quote: None,
                verify_done: 0,
                verifying: false,
                timeline_shown: 0,
                relay: RelayProbe::Checking,
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
                    text: "terminal online — seeded DEMO market loaded".to_owned(),
                }],
                wraps_relayed: 4_182,
                sessions_closed: 137,
                rng: 0x9e37_79b9_7f4a_7c15,
                _tasks: vec![probe, ticker],
            }
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
                let samples: [(&str, u32, &str); 4] = [
                    (
                        "WRAP",
                        SOFT_BLUE,
                        "kind 1059 gift wrap relayed (contents opaque)",
                    ),
                    (
                        "HEAD",
                        ACCENT_DIM,
                        "offering 39601 head refreshed: aurora-lp btc/btc-ln",
                    ),
                    (
                        "HEAD",
                        ACCENT_DIM,
                        "provider 39600 heartbeat: meridian-swaps active",
                    ),
                    (
                        "WRAP",
                        SOFT_BLUE,
                        "kind 1059 gift wrap relayed to npub1mer…x0q4",
                    ),
                ];
                let pick = &samples[(roll as usize / 7) % samples.len()];
                self.push_tape(pick.0, pick.1, format!("{} [DEMO]", pick.2), cx);
            }
            cx.notify();
        }

        fn push_tape(
            &mut self,
            kind: &'static str,
            kind_color: u32,
            text: String,
            _cx: &mut Context<Self>,
        ) {
            self.tape.insert(
                0,
                TapeEvent {
                    clock: format_clock(now_secs()),
                    kind,
                    kind_color,
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
            self.push_tape("SESS", FAINT, "session reset".to_owned(), cx);
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
                "order 39606 signed — quote terms committed by event id [DEMO]".to_owned(),
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
                    Some(seq) => format!("status 39607 seq {seq}: {} [DEMO]", row.state),
                    None => "status seq 3 missing — gap surfaced [DEMO]".to_owned(),
                };
                let color = if row.seq.is_none() { RED } else { SOFT_BLUE };
                self.push_tape("STAT", color, line, cx);
                self.timeline_shown += 1;
            }
            if self.timeline_shown == TIMELINE.len() {
                self.stage = Stage::Closed;
                self.sessions_closed += 1;
                self.push_tape(
                    "CLSE",
                    ACCENT,
                    "close 39609 outcome=completed · public receipt 39603 [DEMO]".to_owned(),
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
            let (relay_dot, relay_text, relay_color) = match &self.relay {
                RelayProbe::Checking => (Color::Muted, "RELAY: PROBING".to_owned(), FAINT),
                RelayProbe::Online { mkt, .. } => (
                    Color::Success,
                    format!("RELAY: LIVE NIP-11{}", if *mkt { " · NIP-MKT" } else { "" }),
                    GREEN,
                ),
                RelayProbe::Unreachable(_) => {
                    (Color::Warning, "RELAY: UNREACHABLE".to_owned(), RED)
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
            panel_frame("NETWORK", Some("1 relay probed live".into())).child(
                v_flex()
                    .px_2()
                    .py_1()
                    .gap_1()
                    .child(key_value("relay", relay_line.0, relay_line.1))
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
                    .child(div().w(px(128.)).child(mono("PROVIDER", FAINT)))
                    .child(div().w(px(64.)).child(mono("STATUS", FAINT)))
                    .child(div().w(px(70.)).child(mono("QUOTES", FAINT)))
                    .child(div().w(px(56.)).child(mono("RESV", FAINT))),
            );
            for provider in PROVIDERS.iter() {
                let active = provider.status == "active";
                rows = rows.child(
                    h_flex()
                        .px_2()
                        .py_1()
                        .border_b_1()
                        .border_color(rgb(RULE))
                        .child(
                            div().w(px(128.)).child(
                                v_flex()
                                    .child(mono(provider.name, BODY))
                                    .child(mono(provider.short_key, FAINT)),
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
                                        provider.status,
                                        if active { GREEN } else { ORANGE },
                                    )),
                            ),
                        )
                        .child(div().w(px(70.)).child(mono(provider.class, SECONDARY)))
                        .child(div().w(px(56.)).child(mono(
                            provider.reservation,
                            if provider.reservation == "hard" {
                                GREEN
                            } else if provider.reservation == "soft" {
                                ORANGE
                            } else {
                                FAINT
                            },
                        ))),
                );
            }
            panel_frame("PROVIDERS · KIND 39600", Some("3 heads".into())).child(rows)
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
                        .child(div().min_w(px(72.)).child(mono(event.clock.clone(), FAINT)))
                        .child(
                            div()
                                .min_w(px(38.))
                                .child(mono(event.kind, event.kind_color)),
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

        fn render_session_market(&self, cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .px_3()
                .py_2()
                .gap_2()
                .child(mono(
                    "Public discovery is live on relays: Provider Profiles (39600) and \
                     Offerings (39601) are signed addressable events any client verifies \
                     itself. Run the flow below — one negotiated swap session, every \
                     protocol law visible.",
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
                                    "rfq 39604 sealed in per-recipient 1059 wraps [DEMO]"
                                        .to_owned(),
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
                                    "quote 39605 firm/hard from aurora-lp [DEMO]".to_owned(),
                                    cx,
                                );
                                this.push_tape(
                                    "QUOT",
                                    ORANGE,
                                    "quote 39605 indicative/soft from meridian-swaps [DEMO]"
                                        .to_owned(),
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
                    "VERIFY BEFORE FUND — the law Boltz taught. Nothing is funded until \
                     the client itself verified every term; the engine refuses funding \
                     readiness until all checks pass.",
                    BODY,
                ))
                .child(checklist)
                .child(if complete {
                    h_flex().pt_1().child(
                        Button::new("fund", "ALL CHECKS GREEN — FUND & WATCH SESSION →")
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
                        .child(mono("CLOSE (39609) + PUBLIC RECEIPT (39603)", ACCENT))
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
            panel_frame("SWAP SESSION · KINDS 39604-39609", Some(subtitle.into())).child(body)
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
                    "synthetic DEMO market · protocol NIP-MKT v0.1 · relay probe is the \
                     only live data",
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

    async fn probe_relay() -> RelayProbe {
        let Some(window) = web_sys::window() else {
            return RelayProbe::Unreachable("no browser window".to_owned());
        };
        let options = RequestInit::new();
        options.set_method("GET");
        let request = match Request::new_with_str_and_init(RELAY_URL, &options) {
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
