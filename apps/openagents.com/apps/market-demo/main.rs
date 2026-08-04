#![cfg_attr(not(target_family = "wasm"), allow(dead_code, unused_imports))]

#[cfg(target_family = "wasm")]
mod web_app {
    use gpui::prelude::*;
    use gpui::{
        App, Bounds, Context, Font, FontWeight, Pixels, Task, Window, WindowBounds, WindowOptions,
        div, px, rgb, size,
    };
    use theme::{ActiveTheme as _, ThemeSettingsProvider, UiDensity};
    use ui::{
        Button, ButtonCommon as _, ButtonStyle, Chip, Clickable as _, Color, Divider, Indicator,
        Label, LabelCommon as _, LabelSize, h_flex, v_flex,
    };
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, Response, console};

    const RELAY_URL: &str = "https://relay.openagents.com";

    const ENERGY_BLUE: u32 = 0x3a7bff;
    const SOFT_BLUE: u32 = 0x8fb6ff;
    const VOID: u32 = 0x05070d;
    const SURFACE: u32 = 0x0c0f13;
    const RAISED: u32 = 0x11161d;
    const BODY: u32 = 0xc9d2dd;
    const SECONDARY: u32 = 0xaeb9c6;
    const FAINT: u32 = 0x7e8a98;
    const HAIRLINE: u32 = 0x1d2530;
    const AMBER: u32 = 0xd9a52c;
    const GREEN: u32 = 0x3fbf7f;
    const RED: u32 = 0xd96a6a;

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

    // The exact UI-facing values from Omega's Aiur theme at the pinned commit,
    // embedded so the browser build does not pull the native-only
    // theme_settings filesystem stack.
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

    // ------------------------------------------------------------------
    // Demo data. Every value below is synthetic and labeled DEMO in the UI.
    // Shapes follow NIP-MKT v0.1 (kinds 39600-39609) and the drafted
    // MKT-SWP submarine-swap profile vocabulary.
    // ------------------------------------------------------------------

    struct DemoProvider {
        name: &'static str,
        short_key: &'static str,
        status: &'static str,
        note: &'static str,
    }

    const PROVIDERS: [DemoProvider; 3] = [
        DemoProvider {
            name: "aurora-lp",
            short_key: "npub1aur…9f2k",
            status: "active",
            note: "firm quotes · hard reservations · noncustodial atomic path",
        },
        DemoProvider {
            name: "meridian-swaps",
            short_key: "npub1mer…x0q4",
            status: "active",
            note: "indicative quotes · soft reservations",
        },
        DemoProvider {
            name: "southpaw-liquidity",
            short_key: "npub1sth…77vd",
            status: "paused",
            note: "offerings retained; not quoting — status honesty on display",
        },
    ];

    struct DemoOffering {
        provider: usize,
        pair: &'static str,
        asset_ids: &'static str,
        direction: &'static str,
        bounds: &'static str,
        profile: &'static str,
    }

    const OFFERINGS: [DemoOffering; 3] = [
        DemoOffering {
            provider: 0,
            pair: "BTC (on-chain) → BTC (Lightning)",
            asset_ids: "id pair: btc / btc-ln — tickers are labels, never identity",
            direction: "submarine swap",
            bounds: "10,000 – 2,000,000 sat (decimal-string atomic units)",
            profile: "mkt-swp v1 (draft)",
        },
        DemoOffering {
            provider: 1,
            pair: "BTC (Lightning) → BTC (on-chain)",
            asset_ids: "id pair: btc-ln / btc",
            direction: "reverse swap",
            bounds: "25,000 – 900,000 sat",
            profile: "mkt-swp v1 (draft)",
        },
        DemoOffering {
            provider: 2,
            pair: "BTC (on-chain) → USDT (demo)",
            asset_ids: "id pair: btc / usdt-demo",
            direction: "chain swap",
            bounds: "provider paused — not quotable",
            profile: "mkt-swp-evm (reserved)",
        },
    ];

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
            reservation_proof: "provider-signed reserve (covenant proof class reserved)",
            price: "100,000 sat in → 99,610 sat out (DEMO)",
            fee: "39 bps · fee is a fill promise, not a fact",
            expiry: "expires 45 s after issue (NIP-40; frozen for demo)",
            custody: "funds_control: self · execution: script · exit: unilateral",
        },
        DemoQuote {
            provider: 1,
            kind: "indicative",
            reservation: "soft",
            reservation_proof: "no capacity committed — requires provider re-accept",
            price: "100,000 sat in → 99,655 sat out (DEMO)",
            fee: "34 bps · cheaper, but nothing is reserved",
            expiry: "expires 120 s after issue (NIP-40; frozen for demo)",
            custody: "funds_control: self · execution: script · exit: unilateral",
        },
    ];

    const VERIFY_STEPS: [(&str, &str); 5] = [
        (
            "Lock script / Taproot tree",
            "rebuilt locally from quoted terms; matches committed template",
        ),
        (
            "Amounts",
            "output amounts equal accepted Quote terms (decimal-string sats)",
        ),
        (
            "Payment hash binding",
            "SHA-256 preimage hash couples both legs; no substitute hash",
        ),
        (
            "Timelocks",
            "refund CLTV clears the claim window with the profile margin",
        ),
        (
            "Claim and refund paths",
            "cooperative key path plus unilateral script-path exit both verified",
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
            detail: "firm quote + conforming order → protocol-effective",
        },
        TimelineRow {
            signer: "provider",
            seq: Some(1),
            state: "funding_observed",
            rung: "measured",
            rung_color: AMBER,
            detail: "lock output seen; observation, not verification",
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
            detail: "seq 3 never arrived — rendered as a gap, never papered over",
        },
        TimelineRow {
            signer: "provider",
            seq: Some(4),
            state: "completed",
            rung: "claim",
            rung_color: SOFT_BLUE,
            detail: "a claim until evidence upgrades it — never auto-settled",
        },
        TimelineRow {
            signer: "requester",
            seq: Some(0),
            state: "completed",
            rung: "verified",
            rung_color: GREEN,
            detail: "requester verified preimage + claim path locally",
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

    enum RelayProbe {
        Checking,
        Online { name: String, mkt: bool },
        Unreachable(String),
    }

    pub struct MarketDemo {
        stage: Stage,
        selected_quote: Option<usize>,
        verify_done: usize,
        verifying: bool,
        timeline_shown: usize,
        relay: RelayProbe,
        _tasks: Vec<Task<()>>,
    }

    impl MarketDemo {
        pub fn new(cx: &mut Context<Self>) -> Self {
            let task = cx.spawn(async move |this, cx| {
                let probe = probe_relay().await;
                this.update(cx, |this, cx| {
                    this.relay = probe;
                    cx.notify();
                })
                .ok();
            });
            Self {
                stage: Stage::Market,
                selected_quote: None,
                verify_done: 0,
                verifying: false,
                timeline_shown: 0,
                relay: RelayProbe::Checking,
                _tasks: vec![task],
            }
        }

        fn reset(&mut self, cx: &mut Context<Self>) {
            self.stage = Stage::Market;
            self.selected_quote = None;
            self.verify_done = 0;
            self.verifying = false;
            self.timeline_shown = 0;
            cx.notify();
        }

        fn start_verification(&mut self, cx: &mut Context<Self>) {
            if self.verifying {
                return;
            }
            self.stage = Stage::Verify;
            self.verify_done = 0;
            self.verifying = true;
            cx.notify();
            let task = cx.spawn(async move |this, cx| {
                for step in 0..VERIFY_STEPS.len() {
                    // A bounded busy loop on the background executor paces the
                    // checklist so viewers watch each verification land. The
                    // real engine does real script/amount/hash/timelock work
                    // here (immortal M12 #12).
                    let spun = cx
                        .background_spawn(async move {
                            let mut acc: u64 = 0;
                            for i in 0..14_000_000u64 {
                                acc = acc.wrapping_add(i ^ (step as u64));
                            }
                            acc
                        })
                        .await;
                    let _ = spun;
                    this.update(cx, |this, cx| {
                        this.verify_done = step + 1;
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
                self.timeline_shown += 1;
            }
            if self.timeline_shown == TIMELINE.len() {
                self.stage = Stage::Closed;
            }
            cx.notify();
        }
    }

    fn section_title(text: &'static str) -> impl IntoElement {
        Label::new(text)
            .size(LabelSize::XSmall)
            .color(Color::Custom(rgb(FAINT).into()))
    }

    fn demo_chip() -> impl IntoElement {
        div()
            .px_2()
            .py_0p5()
            .rounded_sm()
            .bg(rgb(0x2a1f0a))
            .border_1()
            .border_color(rgb(AMBER))
            .child(
                Label::new("DEMO — no funds exist on this surface")
                    .size(LabelSize::XSmall)
                    .color(Color::Custom(rgb(AMBER).into())),
            )
    }

    fn rung_chip(rung: &'static str, color: u32) -> impl IntoElement {
        div()
            .px_2()
            .py_0p5()
            .rounded_sm()
            .border_1()
            .border_color(rgb(color))
            .child(
                Label::new(rung)
                    .size(LabelSize::XSmall)
                    .color(Color::Custom(rgb(color).into())),
            )
    }

    fn key_value(key: &'static str, value: String, value_color: u32) -> impl IntoElement {
        h_flex()
            .gap_2()
            .items_start()
            .child(
                div().min_w(px(120.)).child(
                    Label::new(key)
                        .size(LabelSize::XSmall)
                        .color(Color::Custom(rgb(FAINT).into())),
                ),
            )
            .child(
                Label::new(value)
                    .size(LabelSize::XSmall)
                    .color(Color::Custom(rgb(value_color).into())),
            )
    }

    fn card() -> gpui::Div {
        v_flex()
            .gap_2()
            .p_4()
            .rounded_md()
            .bg(rgb(SURFACE))
            .border_1()
            .border_color(rgb(HAIRLINE))
    }

    impl MarketDemo {
        fn render_header(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            let relay = match &self.relay {
                RelayProbe::Checking => h_flex().gap_2().child(
                    Label::new("checking relay.openagents.com…")
                        .size(LabelSize::XSmall)
                        .color(Color::Custom(rgb(FAINT).into())),
                ),
                RelayProbe::Online { name, mkt } => h_flex()
                    .gap_2()
                    .child(Indicator::dot().color(Color::Success))
                    .child(
                        Label::new(format!(
                            "{name} — live NIP-11 · {}",
                            if *mkt {
                                "nip-mkt advertised"
                            } else {
                                "nip-mkt not advertised"
                            }
                        ))
                        .size(LabelSize::XSmall)
                        .color(Color::Custom(rgb(SECONDARY).into())),
                    ),
                RelayProbe::Unreachable(reason) => h_flex()
                    .gap_2()
                    .child(Indicator::dot().color(Color::Warning))
                    .child(
                        Label::new(format!("relay unreachable — {reason}"))
                            .size(LabelSize::XSmall)
                            .color(Color::Custom(rgb(FAINT).into())),
                    ),
            };
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    Label::new("OPENAGENTS MARKETS")
                                        .size(LabelSize::Large)
                                        .weight(FontWeight::SEMIBOLD)
                                        .color(Color::Custom(rgb(0xffffff).into())),
                                )
                                .child(
                                    Label::new("swap demo · NIP-MKT v0.1")
                                        .size(LabelSize::Small)
                                        .color(Color::Custom(rgb(SOFT_BLUE).into())),
                                ),
                        )
                        .child(demo_chip()),
                )
                .child(h_flex().justify_between().child(relay).child(
                    Label::new("rendered by Omega's GPUI design system on WebGPU/WASM")
                        .size(LabelSize::XSmall)
                        .color(Color::Custom(rgb(FAINT).into())),
                ))
        }

        fn render_market(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let mut providers = v_flex().gap_2();
            for provider in PROVIDERS.iter() {
                let active = provider.status == "active";
                providers = providers.child(
                    card()
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Indicator::dot().color(if active {
                                            Color::Success
                                        } else {
                                            Color::Warning
                                        }))
                                        .child(
                                            Label::new(provider.name)
                                                .size(LabelSize::Small)
                                                .weight(FontWeight::SEMIBOLD)
                                                .color(Color::Custom(rgb(BODY).into())),
                                        )
                                        .child(
                                            Label::new(provider.short_key)
                                                .size(LabelSize::XSmall)
                                                .color(Color::Custom(rgb(FAINT).into())),
                                        ),
                                )
                                .child(Chip::new(provider.status)),
                        )
                        .child(
                            Label::new(provider.note)
                                .size(LabelSize::XSmall)
                                .color(Color::Custom(rgb(SECONDARY).into())),
                        ),
                );
            }

            let mut offerings = v_flex().gap_2();
            for offering in OFFERINGS.iter() {
                let provider = &PROVIDERS[offering.provider];
                offerings = offerings.child(
                    card()
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    Label::new(offering.pair)
                                        .size(LabelSize::Small)
                                        .weight(FontWeight::SEMIBOLD)
                                        .color(Color::Custom(rgb(BODY).into())),
                                )
                                .child(Chip::new(offering.direction)),
                        )
                        .child(key_value("provider", provider.name.into(), SECONDARY))
                        .child(key_value("identity", offering.asset_ids.into(), FAINT))
                        .child(key_value("bounds", offering.bounds.into(), SECONDARY))
                        .child(key_value("profile", offering.profile.into(), SOFT_BLUE)),
                );
            }

            v_flex()
                .gap_4()
                .child(
                    Label::new(
                        "Public discovery: Provider Profiles (kind 39600) and Offerings \
                         (kind 39601) are signed addressable events any relay can serve \
                         and any client can verify. This demo renders a seeded market.",
                    )
                    .size(LabelSize::Small)
                    .color(Color::Custom(rgb(SECONDARY).into())),
                )
                .child(section_title("PROVIDERS"))
                .child(providers)
                .child(section_title("OFFERINGS"))
                .child(offerings)
                .child(
                    h_flex().pt_2().child(
                        Button::new("send-rfq", "Send demo RFQ for 100,000 sat →")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.stage = Stage::Rfq;
                                cx.notify();
                            })),
                    ),
                )
        }

        fn render_rfq(&self, cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .gap_4()
                .child(
                    card()
                        .child(section_title("PRIVATE RFQ (KIND 39604) — SENT"))
                        .child(
                            Label::new(
                                "The RFQ is a fully signed event sealed inside a NIP-59 gift \
                                 wrap per recipient. Relays see an opaque wrap addressed to \
                                 each provider — no amounts, pairs, or counterparties leak. \
                                 Each provider gets its own wrap and cannot see who else \
                                 was asked.",
                            )
                            .size(LabelSize::Small)
                            .color(Color::Custom(rgb(BODY).into())),
                        )
                        .child(key_value("session", "d41c…9a03 (random 32-byte id)".into(), FAINT))
                        .child(key_value("amount", "100,000 sat (DEMO)".into(), SECONDARY))
                        .child(key_value(
                            "recipients",
                            "aurora-lp, meridian-swaps (separate wraps)".into(),
                            SECONDARY,
                        ))
                        .child(key_value(
                            "expiry",
                            "NIP-40 expiration set — expiry never implies consent".into(),
                            FAINT,
                        )),
                )
                .child(
                    h_flex().child(
                        Button::new("recv-quotes", "Both providers answered — view Quotes →")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.stage = Stage::Quotes;
                                cx.notify();
                            })),
                    ),
                )
        }

        fn render_quotes(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let mut list = v_flex().gap_3();
            for (index, quote) in QUOTES.iter().enumerate() {
                let provider = &PROVIDERS[quote.provider];
                let firm = quote.kind == "firm";
                list = list.child(
                    card()
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
                                                .size(LabelSize::Small)
                                                .weight(FontWeight::SEMIBOLD)
                                                .color(Color::Custom(rgb(BODY).into())),
                                        )
                                        .child(Chip::new(quote.kind))
                                        .child(rung_chip(
                                            if firm { "reservation: hard" } else { "reservation: soft" },
                                            if firm { GREEN } else { AMBER },
                                        )),
                                )
                                .child(
                                    Button::new(("accept", index), "Accept → Order")
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
                        .child(key_value("reservation", quote.reservation_proof.into(), SECONDARY))
                        .child(key_value("custody", quote.custody.into(), FAINT))
                        .child(key_value("expiry", quote.expiry.into(), FAINT)),
                );
            }
            v_flex()
                .gap_4()
                .child(
                    Label::new(
                        "Signed, expiring Quotes (kind 39605). The event ID commits the \
                         exact terms: an Order references that ID and cannot restate a \
                         different price and call it acceptance. Compare the trade-off: \
                         firm+hard commits capacity; indicative+soft is cheaper on paper \
                         with nothing reserved.",
                    )
                    .size(LabelSize::Small)
                    .color(Color::Custom(rgb(SECONDARY).into())),
                )
                .child(list)
        }

        fn render_verify(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let mut checklist = v_flex().gap_2();
            for (index, (name, detail)) in VERIFY_STEPS.iter().enumerate() {
                let done = index < self.verify_done;
                checklist = checklist.child(
                    h_flex()
                        .gap_3()
                        .items_start()
                        .child(if done {
                            Indicator::dot().color(Color::Success)
                        } else {
                            Indicator::dot().color(Color::Muted)
                        })
                        .child(
                            v_flex()
                                .gap_0p5()
                                .child(
                                    Label::new(*name)
                                        .size(LabelSize::Small)
                                        .weight(FontWeight::SEMIBOLD)
                                        .color(Color::Custom(
                                            rgb(if done { BODY } else { FAINT }).into(),
                                        )),
                                )
                                .child(
                                    Label::new(*detail)
                                        .size(LabelSize::XSmall)
                                        .color(Color::Custom(rgb(FAINT).into())),
                                ),
                        ),
                );
            }
            let complete = self.verify_done == VERIFY_STEPS.len() && !self.verifying;
            v_flex()
                .gap_4()
                .child(
                    card()
                        .child(section_title("VERIFY BEFORE FUND — THE LAW BOLTZ TAUGHT"))
                        .child(
                            Label::new(
                                "Nothing is funded until the client itself has verified \
                                 every term below. This is structural: the engine refuses \
                                 to signal funding readiness until all checks pass.",
                            )
                            .size(LabelSize::Small)
                            .color(Color::Custom(rgb(BODY).into())),
                        )
                        .child(checklist),
                )
                .child(if complete {
                    h_flex().gap_3().items_center().child(
                        Button::new("fund", "All checks green — fund and watch the session →")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.stage = Stage::Timeline;
                                this.timeline_shown = 1;
                                cx.notify();
                            })),
                    )
                } else {
                    h_flex().child(
                        Label::new("verifying…")
                            .size(LabelSize::Small)
                            .color(Color::Custom(rgb(SOFT_BLUE).into())),
                    )
                })
        }

        fn render_timeline(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let mut rows = v_flex().gap_2();
            for row in TIMELINE.iter().take(self.timeline_shown) {
                let is_gap = row.seq.is_none();
                rows = rows.child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .p_2()
                        .rounded_sm()
                        .bg(rgb(if is_gap { 0x1a0d0d } else { RAISED }))
                        .border_1()
                        .border_color(rgb(if is_gap { 0x3a1d1d } else { HAIRLINE }))
                        .child(
                            div().min_w(px(90.)).child(
                                Label::new(row.signer)
                                    .size(LabelSize::XSmall)
                                    .color(Color::Custom(rgb(FAINT).into())),
                            ),
                        )
                        .child(
                            div().min_w(px(52.)).child(
                                Label::new(match row.seq {
                                    Some(seq) => format!("seq {seq}"),
                                    None => "seq 3?".to_owned(),
                                })
                                .size(LabelSize::XSmall)
                                .color(Color::Custom(rgb(if is_gap { RED } else { SECONDARY }).into())),
                            ),
                        )
                        .child(
                            div().min_w(px(150.)).child(
                                Label::new(row.state)
                                    .size(LabelSize::Small)
                                    .weight(FontWeight::SEMIBOLD)
                                    .color(Color::Custom(rgb(if is_gap { RED } else { BODY }).into())),
                            ),
                        )
                        .child(rung_chip(row.rung, row.rung_color))
                        .child(
                            Label::new(row.detail)
                                .size(LabelSize::XSmall)
                                .color(Color::Custom(rgb(FAINT).into())),
                        ),
                );
            }
            let done = self.timeline_shown >= TIMELINE.len();
            v_flex()
                .gap_4()
                .child(
                    Label::new(
                        "Status records (kind 39607) carry a dense per-signer sequence. \
                         A missing number is displayed as a gap; two records at one \
                         sequence would render as a fork. Nothing is silently resolved, \
                         and no state implies the next rung of proof.",
                    )
                    .size(LabelSize::Small)
                    .color(Color::Custom(rgb(SECONDARY).into())),
                )
                .child(rows)
                .child(if done {
                    h_flex().child(
                        Label::new("session terminal — see Close below")
                            .size(LabelSize::Small)
                            .color(Color::Custom(rgb(SOFT_BLUE).into())),
                    )
                } else {
                    h_flex().child(
                        Button::new("advance", "Next status record →")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.advance_timeline(cx);
                            })),
                    )
                })
        }

        fn render_close(&self, cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .gap_4()
                .child(
                    card()
                        .child(section_title("CLOSE (KIND 39609) + PUBLIC RECEIPT (KIND 39603)"))
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
                            "optional, redacted, consented: outcome + close ref only. \
                             No session, counterparty, amount, or route disclosed."
                                .into(),
                            SECONDARY,
                        ))
                        .child(key_value(
                            "rung",
                            "still a signer's claim — 'settled' requires exact external \
                             proof this demo does not manufacture"
                                .into(),
                            AMBER,
                        )),
                )
                .child(
                    card()
                        .bg(rgb(0x0a1220))
                        .border_color(rgb(ENERGY_BLUE))
                        .child(
                            Label::new(
                                "Boltz and Satora went dark because one company's API was \
                                 the market. This market is signed events on relays: any \
                                 provider can join, any client can verify, and this page \
                                 is just one window onto it.",
                            )
                            .size(LabelSize::Small)
                            .color(Color::Custom(rgb(SOFT_BLUE).into())),
                        ),
                )
                .child(
                    h_flex().child(
                        Button::new("reset", "Run the demo again")
                            .style(ButtonStyle::Outlined)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.reset(cx);
                            })),
                    ),
                )
        }

        fn render_stage_rail(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            const STAGES: [(&str, Stage); 6] = [
                ("1 · Market", Stage::Market),
                ("2 · RFQ", Stage::Rfq),
                ("3 · Quotes", Stage::Quotes),
                ("4 · Verify", Stage::Verify),
                ("5 · Session", Stage::Timeline),
                ("6 · Close", Stage::Closed),
            ];
            let mut rail = h_flex().gap_2().flex_wrap();
            for (label, stage) in STAGES {
                let current = self.stage == stage;
                rail = rail.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(if current { RAISED } else { SURFACE }))
                        .border_1()
                        .border_color(rgb(if current { ENERGY_BLUE } else { HAIRLINE }))
                        .child(
                            Label::new(label)
                                .size(LabelSize::XSmall)
                                .color(Color::Custom(
                                    rgb(if current { SOFT_BLUE } else { FAINT }).into(),
                                )),
                        ),
                );
            }
            rail
        }
    }

    impl Render for MarketDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let _aiur = cx.theme();
            let body: gpui::AnyElement = match self.stage {
                Stage::Market => self.render_market(cx).into_any_element(),
                Stage::Rfq => self.render_rfq(cx).into_any_element(),
                Stage::Quotes => self.render_quotes(cx).into_any_element(),
                Stage::Verify => self.render_verify(cx).into_any_element(),
                Stage::Timeline => self.render_timeline(cx).into_any_element(),
                Stage::Closed => v_flex()
                    .gap_4()
                    .child(self.render_timeline(cx))
                    .child(self.render_close(cx))
                    .into_any_element(),
            };
            div()
                .size_full()
                .bg(rgb(VOID))
                .overflow_hidden()
                .child(
                    div()
                        .id("market-demo-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .child(
                            v_flex()
                                .max_w(px(880.))
                                .mx_auto()
                                .px_6()
                                .py_6()
                                .gap_5()
                                .child(self.render_header(cx))
                                .child(Divider::horizontal())
                                .child(self.render_stage_rail(cx))
                                .child(body)
                                .child(Divider::horizontal())
                                .child(
                                    v_flex().gap_1().pb_10().child(
                                        Label::new(
                                            "Every value on this page is synthetic demo data \
                                             shaped by NIP-MKT v0.1. No keys, funds, or custody \
                                             exist in this browser. The protocol, the relay \
                                             implementation, and this design system are open \
                                             source: OpenAgentsInc/openagents · \
                                             OpenAgentsInc/immortal · OpenAgentsInc/omega.",
                                        )
                                        .size(LabelSize::XSmall)
                                        .color(Color::Custom(rgb(FAINT).into())),
                                    ),
                                ),
                        ),
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
                |_, cx| cx.new(MarketDemo::new),
            ) {
                Ok(_) => cx.activate(true),
                Err(error) => console::error_1(&JsValue::from_str(&format!(
                    "failed to open the market demo: {error:#}"
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
    eprintln!("market_demo_web builds for wasm32-unknown-unknown");
}
