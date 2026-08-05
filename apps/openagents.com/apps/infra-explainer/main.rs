#![cfg_attr(not(target_family = "wasm"), allow(dead_code, unused_imports))]

#[cfg(target_family = "wasm")]
mod web_app {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::{Arc, OnceLock};

    use gpui::prelude::*;
    use gpui::{
        App, Bounds, Context, Font, FontWeight, Image, ImageFormat, ImageSource, Pixels, Point,
        ScrollHandle, Task, Window, WindowBounds, WindowOptions, div, img, point, px, rgb, size,
    };
    use theme::{ActiveTheme as _, ThemeSettingsProvider, UiDensity};
    use ui::{Color, Indicator, Label, LabelCommon as _, LabelSize, h_flex, v_flex};
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, Response, console};

    const RELAY_URL: &str = "https://relay.openagents.com";

    // Aiur palette (omega assets/themes/aiur/aiur.json).
    const VOID: u32 = 0x05070d; // editor.background — the page
    const PANEL: u32 = 0x0b1220; // background — panels
    const HAIRLINE: u32 = 0x1f2b45; // border
    const TEXT: u32 = 0xeef3ff; // text
    const MUTED: u32 = 0xa9b1d6; // text.muted
    const FAINT: u32 = 0x5b6486; // dimmed muted
    const LINK: u32 = 0x5c96f8; // link accent
    const RED: u32 = 0xf7768e; // failure accent
    const ORANGE: u32 = 0xff9e64; // custody accent

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
            UiDensity::Comfortable
        }
    }

    // Aiur surface values applied over Omega's theme machinery.
    const AIUR_JSON: &str = r##"{
      "themes": [{
        "style": {
          "border": "#1f2b45ff",
          "border.variant": "#141d33ff",
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
    // Diagrams: rendered at build time by diagrams-gen (merman + usvg,
    // Aiur-themed, text flattened to paths) and embedded as SVG bytes.
    // ------------------------------------------------------------------

    const DIAGRAM_SOURCES: [&str; 6] = [
        include_str!("assets/diagrams/01-what-went-down.svg"),
        include_str!("assets/diagrams/02-decentralized-shape.svg"),
        include_str!("assets/diagrams/03-monorepo.svg"),
        include_str!("assets/diagrams/04-swap-end-to-end.svg"),
        include_str!("assets/diagrams/05-inside-a-provider.svg"),
        include_str!("assets/diagrams/06-die-safely.svg"),
    ];

    /// Content column width in layout pixels.
    const COLUMN_WIDTH: f32 = 940.;
    /// Horizontal room a diagram gets inside the column, minus frame padding.
    const DIAGRAM_VIEWPORT_WIDTH: f32 = COLUMN_WIDTH - 36.;
    /// Diagrams never shrink below this scale; wider ones pan sideways.
    const MIN_DIAGRAM_SCALE: f32 = 0.72;
    /// A diagram box never grows past this height; taller ones pan vertically.
    const MAX_DIAGRAM_HEIGHT: f32 = 820.;

    /// Per-diagram pan state.
    ///
    /// `handle` is the scroll state GPUI itself maintains for the diagram's
    /// viewport; `settled` is the last in-range offset this document observed,
    /// which is what makes the wheel-propagation decision exact (see
    /// [`InfraDocument::diagram_panel`]).
    #[derive(Clone)]
    struct DiagramPan {
        handle: ScrollHandle,
        settled: Rc<Cell<Point<Pixels>>>,
    }

    impl DiagramPan {
        fn new() -> Self {
            Self {
                handle: ScrollHandle::new(),
                settled: Rc::new(Cell::new(Point::default())),
            }
        }
    }

    fn diagram_images() -> &'static [Arc<Image>; 6] {
        static IMAGES: OnceLock<[Arc<Image>; 6]> = OnceLock::new();
        IMAGES.get_or_init(|| {
            DIAGRAM_SOURCES.map(|source| {
                Arc::new(Image::from_bytes(
                    ImageFormat::Svg,
                    source.as_bytes().to_vec(),
                ))
            })
        })
    }

    /// Reads the root `width="…" height="…"` attributes usvg writes.
    fn svg_dimensions(source: &str) -> (f32, f32) {
        fn attr(source: &str, name: &str) -> Option<f32> {
            let start = source.find(&format!("{name}=\""))? + name.len() + 2;
            let rest = &source[start..];
            let end = rest.find('"')?;
            rest[..end].parse().ok()
        }
        let head = &source[..source.len().min(512)];
        (
            attr(head, "width").unwrap_or(800.),
            attr(head, "height").unwrap_or(600.),
        )
    }

    struct Section {
        number: &'static str,
        title: &'static str,
        prose: &'static [&'static str],
        diagram: usize,
        caption: &'static str,
    }

    const SECTIONS: [Section; 6] = [
        Section {
            number: "01",
            title: "WHAT WENT DOWN",
            prose: &[
                "Boltz — the dominant non-custodial swap provider — suspended swaps under a \
                 sustained rise of AI-assisted probes and exploits, and a wave of wallets and \
                 services that treated one company's API as the market went down with it.",
                "The shape was the vulnerability: one operator was simultaneously the REST API, \
                 the backend, the liquidity, and the web UI. Everything downstream inherited \
                 that single point of failure by construction.",
            ],
            diagram: 0,
            caption: "the centralized shape: one operator, everyone downstream",
        },
        Section {
            number: "02",
            title: "THE DECENTRALIZED SHAPE",
            prose: &[
                "The replacement splits the one operator into three roles that fail \
                 independently. Clients hold their own keys. Relays carry public discovery — \
                 Provider Profiles (kind 39600) and Offerings (kind 39601) — plus gift-wrapped \
                 private negotiation the relay cannot read. Independent providers hold their own \
                 funds and drive their own rails.",
                "The relay never holds funds, spend keys, or unreleased preimages. That is a \
                 structural rule of the software, not an operator promise.",
            ],
            diagram: 1,
            caption: "clients ⇄ N relays ⇄ M providers, each with their own rails",
        },
        Section {
            number: "03",
            title: "ONE REPO, HARDENED BINARIES",
            prose: &[
                "Immortal grew from one hardened Nostr relay into a Cargo workspace of small, \
                 severe, independently deployable binaries sharing one discipline: \
                 immortal-core (events, NIP-44, MKT grammar, taproot and bolt11 verification \
                 primitives — written in-repo, fixture-tested), immortal-relay (the deployed \
                 binary, still named immortal), immortal-client (the wasm verify-before-fund \
                 swap engine behind the TypeScript SDK), and immortal-provider, the liquidity \
                 daemon that holds the money.",
                "The custody boundary is a crate boundary: cargo tree on the relay binary shows \
                 no wallet, signing, or spend-capable code. Same seven-crate dependency \
                 allowlist per product, owner-approved (workspace conversion: immortal #24).",
            ],
            diagram: 2,
            caption: "the workspace: shared audited core, custody as a crate boundary",
        },
        Section {
            number: "04",
            title: "A SWAP, END TO END",
            prose: &[
                "Discovery is public; everything else travels sealed. The wallet sends an RFQ \
                 (39604) in per-recipient gift wraps, providers answer with signed expiring \
                 Quotes (39605) whose event id pins the exact terms, and the Order (39606) \
                 commits that id — nobody can restate a price and call it acceptance.",
                "Nothing is funded until the client itself re-derives the lock script, amounts, \
                 payment hash, and timelocks. Then: lockup, hold invoice, claim with preimage, \
                 sequenced Status records (39607) where a missing number is a displayed gap, \
                 and a terminal Close (39609) with a redacted public receipt (39603).",
            ],
            diagram: 3,
            caption: "one negotiated swap session over relays and rails",
        },
        Section {
            number: "05",
            title: "INSIDE A PROVIDER",
            prose: &[
                "immortal-provider runs against operator-owned rails: bitcoind over polled \
                 JSON-RPC, CLN plus the hold plugin over a unix socket, and its own Postgres \
                 for state — never keys. The seed lives in an operator-owned file, mode 0600, \
                 outside the database.",
                "Around the session logic: a reservation ledger so a hard reservation is never \
                 emitted without confirmed capacity, a watchtower loop for the timeout ladder, \
                 and an alert webhook that pages a human — a stuck swap is money sitting on a \
                 timelock. All of it on the relay's same minimal dependency discipline \
                 (provider rails: immortal #25).",
            ],
            diagram: 4,
            caption: "the daemon and the rails it drives; keys never touch Postgres",
        },
        Section {
            number: "06",
            title: "DESIGNED TO DIE SAFELY",
            prose: &[
                "Kill any role. If the relay dies, in-flight swaps complete or refund from the \
                 client's persisted session records — that is the doomsday drill. If a provider \
                 dies, the client's unilateral script-path exit needs no cooperation. If a \
                 client dies, the provider's refund ladder returns its own funds at expiry.",
                "Every route quotes a timeout ladder before funding, so every party always has \
                 a path out that it can execute alone. No role's failure strands another \
                 role's money (adversarial regtest lab: immortal #18; network cutover runbook: \
                 immortal #19).",
            ],
            diagram: 5,
            caption: "three deaths, three survivals, one law",
        },
    ];

    enum RelayProbe {
        Checking,
        Online { name: String, mkt: bool },
        Unreachable(String),
    }

    pub struct InfraDocument {
        relay: RelayProbe,
        pans: [DiagramPan; 6],
        _tasks: Vec<Task<()>>,
    }

    impl InfraDocument {
        pub fn new(cx: &mut Context<Self>) -> Self {
            let probe = cx.spawn(async move |this, cx| {
                let probe = probe_relay().await;
                this.update(cx, |this, cx| {
                    this.relay = probe;
                    cx.notify();
                })
                .ok();
            });
            Self {
                relay: RelayProbe::Checking,
                pans: std::array::from_fn(|_| DiagramPan::new()),
                _tasks: vec![probe],
            }
        }
    }

    // ------------------------------------------------------------------
    // Render helpers
    // ------------------------------------------------------------------

    fn mono(text: impl Into<gpui::SharedString>, color: u32) -> Label {
        Label::new(text)
            .size(LabelSize::Small)
            .color(Color::Custom(rgb(color).into()))
    }

    fn small(text: impl Into<gpui::SharedString>, color: u32) -> Label {
        Label::new(text)
            .size(LabelSize::XSmall)
            .color(Color::Custom(rgb(color).into()))
    }

    fn paragraph(text: &'static str) -> impl IntoElement {
        div().max_w(px(COLUMN_WIDTH)).child(
            Label::new(text)
                .size(LabelSize::Small)
                .color(Color::Custom(rgb(MUTED).into())),
        )
    }

    impl InfraDocument {
        /// One diagram in its own pannable viewport.
        ///
        /// Wheel ownership is decided per axis, and never traps the reader.
        /// GPUI dispatches wheel events to the innermost element first, and its
        /// built-in scroll listener has already applied this tick's delta to
        /// the viewport by the time the listener below runs. So the listener
        /// only has to answer one question: did the diagram actually move?
        /// It compares the viewport's current in-range offset against the last
        /// in-range offset it recorded. If they differ the diagram consumed the
        /// wheel and propagation stops, so the page stays put. If they match —
        /// because that axis has no overflow, or because the diagram is already
        /// against its limit — propagation continues and the page scrolls
        /// normally. `restrict_scroll_to_axis` keeps a vertical wheel from
        /// being remapped onto the horizontal axis, which is what makes
        /// ordinary reading over a wide diagram behave like ordinary reading.
        fn diagram_panel(&self, index: usize, caption: &'static str) -> gpui::AnyElement {
            let (natural_w, natural_h) = svg_dimensions(DIAGRAM_SOURCES[index]);
            let fit = DIAGRAM_VIEWPORT_WIDTH / natural_w;
            let scale = if fit >= 1.0 {
                1.0
            } else {
                fit.max(MIN_DIAGRAM_SCALE)
            };
            let display_w = natural_w * scale;
            let display_h = natural_h * scale;
            let viewport_h = display_h.min(MAX_DIAGRAM_HEIGHT);
            // At the default column width these are the only diagrams wider
            // than their box. A narrower window pans more of them, which the
            // same viewport handles because GPUI measures the real layout.
            let pans_horizontally = display_w > DIAGRAM_VIEWPORT_WIDTH;
            let pans_vertically = display_h > viewport_h;

            let pan = self.pans[index].clone();
            let handle = pan.handle.clone();
            let settled = pan.settled.clone();

            let image = img(ImageSource::Image(diagram_images()[index].clone()))
                .flex_none()
                .w(px(display_w))
                .h(px(display_h));

            // A diagram that fits its box never becomes a scroll target at all,
            // so the wheel over it is always the page's.
            if !pans_horizontally && !pans_vertically {
                return Self::diagram_frame(
                    div().w_full().flex().justify_center().child(image),
                    caption,
                    "",
                );
            }

            let mut viewport = div()
                .id(("diagram-viewport", index))
                .w_full()
                .h(px(viewport_h))
                .track_scroll(&pan.handle)
                .child(image);
            // Enable exactly the axes that overflow. That is what lets GPUI
            // treat a plain vertical wheel as a sideways pan on a diagram that
            // only overflows horizontally (its rule: remap y onto x only when
            // the y axis is not itself scrollable), so a mouse without a
            // horizontal wheel can still read the wide diagrams.
            if pans_horizontally {
                viewport = viewport.overflow_x_scroll();
            }
            if pans_vertically {
                viewport = viewport.overflow_y_scroll();
                // When both axes move, keep them honest: vertical reading must
                // not be hijacked into sideways motion.
                viewport.style().restrict_scroll_to_axis = Some(true);
            }
            let viewport = viewport.on_scroll_wheel(move |_event, _window, cx| {
                let max = handle.max_offset();
                let offset = handle.offset();
                // GPUI keeps scroll offsets in [-max, 0] and clamps on the next
                // prepaint; clamp here so an overshoot at a limit still reads
                // as "did not move".
                let in_range = point(
                    offset.x.clamp(-max.x, px(0.)),
                    offset.y.clamp(-max.y, px(0.)),
                );
                if in_range != settled.get() {
                    settled.set(in_range);
                    cx.stop_propagation();
                }
            });

            let hint = if pans_horizontally && pans_vertically {
                "scroll inside the diagram to pan it"
            } else if pans_horizontally {
                "scroll inside the diagram to pan right · page resumes at the edge"
            } else if pans_vertically {
                "scroll inside the diagram to pan it"
            } else {
                ""
            };

            Self::diagram_frame(viewport, caption, hint)
        }

        fn diagram_frame(
            body: impl IntoElement,
            caption: &'static str,
            hint: &'static str,
        ) -> gpui::AnyElement {
            v_flex()
                .w_full()
                .bg(rgb(VOID))
                .border_1()
                .border_color(rgb(HAIRLINE))
                .child(div().p_2().w_full().child(body))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .px_2()
                        .py_1()
                        .bg(rgb(PANEL))
                        .border_t_1()
                        .border_color(rgb(HAIRLINE))
                        .child(small(caption, FAINT))
                        .child(small(hint, FAINT)),
                )
                .into_any_element()
        }
    }

    impl InfraDocument {
        fn render_top_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
            let (relay_dot, relay_text, relay_color) = match &self.relay {
                RelayProbe::Checking => (Color::Muted, "RELAY: PROBING".to_owned(), FAINT),
                RelayProbe::Online { mkt, .. } => (
                    Color::Success,
                    format!("RELAY: LIVE NIP-11{}", if *mkt { " · NIP-MKT" } else { "" }),
                    LINK,
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
                .bg(rgb(PANEL))
                .border_b_1()
                .border_color(rgb(HAIRLINE))
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            Label::new("IMMORTAL INFRASTRUCTURE")
                                .size(LabelSize::Small)
                                .weight(FontWeight::EXTRA_BOLD)
                                .color(Color::Custom(rgb(TEXT).into())),
                        )
                        .child(small("EP 267", LINK))
                        .child(small("THE DECENTRALIZED BOLTZ REPLACEMENT", FAINT)),
                )
                .child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .child(Indicator::dot().color(relay_dot))
                        .child(small(relay_text, relay_color)),
                )
        }

        fn render_intro(&self) -> impl IntoElement {
            v_flex()
                .gap_2()
                .pt_6()
                .pb_2()
                .child(
                    Label::new("Immortal Infrastructure")
                        .size(LabelSize::Large)
                        .weight(FontWeight::EXTRA_BOLD)
                        .color(Color::Custom(rgb(TEXT).into())),
                )
                .child(paragraph(
                    "Episode 267 · the architecture behind the decentralized Boltz \
                     replacement. Six views of the swap network that github.com/OpenAgentsInc/\
                     immortal ships: what failed, the shape that replaces it, the hardened \
                     binaries, one full swap, the inside of a liquidity provider, and why every \
                     role can die without stranding anyone's money.",
                ))
                .child(small(
                    "diagrams rendered from committed mermaid sources at build time · \
                     the relay status above is the only live data on this page",
                    FAINT,
                ))
        }

        fn render_section(&self, section: &Section) -> impl IntoElement {
            let mut prose = v_flex().gap_2();
            for text in section.prose {
                prose = prose.child(paragraph(text));
            }
            v_flex()
                .gap_3()
                .pt_6()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .px_1p5()
                                .border_1()
                                .border_color(rgb(LINK))
                                .child(small(section.number, LINK)),
                        )
                        .child(
                            Label::new(section.title)
                                .size(LabelSize::Default)
                                .weight(FontWeight::BOLD)
                                .color(Color::Custom(rgb(TEXT).into())),
                        ),
                )
                .child(prose)
                .child(self.diagram_panel(section.diagram, section.caption))
        }

        fn render_footer(&self) -> impl IntoElement {
            let relay_line = match &self.relay {
                RelayProbe::Checking => ("NIP-11 probe pending…".to_owned(), FAINT),
                RelayProbe::Online { name, mkt } => (
                    format!(
                        "relay.openagents.com · {name}{}",
                        if *mkt {
                            " · nip-mkt advertised"
                        } else {
                            " · nip-mkt not yet advertised"
                        }
                    ),
                    LINK,
                ),
                RelayProbe::Unreachable(reason) => {
                    (format!("relay.openagents.com · {reason}"), RED)
                }
            };
            v_flex()
                .gap_1()
                .mt_8()
                .mb_10()
                .p_3()
                .bg(rgb(PANEL))
                .border_1()
                .border_color(rgb(HAIRLINE))
                .child(small("KEEP PULLING THE THREAD", ORANGE))
                .child(mono("source: github.com/OpenAgentsInc/immortal (CC0)", MUTED))
                .child(mono(
                    "live swap-session walkthrough: openagents.com/demo",
                    MUTED,
                ))
                .child(mono(relay_line.0, relay_line.1))
        }
    }

    impl Render for InfraDocument {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let _theme = cx.theme();
            let mut column = v_flex()
                .w(px(COLUMN_WIDTH))
                .max_w_full()
                .child(self.render_intro());
            for section in SECTIONS.iter() {
                column = column.child(self.render_section(section));
            }
            column = column.child(self.render_footer());

            div()
                .size_full()
                .bg(rgb(VOID))
                .overflow_hidden()
                .child(
                    v_flex()
                        .size_full()
                        .child(self.render_top_bar(cx))
                        .child(
                            div()
                                .id("document-scroll")
                                .flex_1()
                                .overflow_y_scroll()
                                .child(
                                    div()
                                        .w_full()
                                        .flex()
                                        .justify_center()
                                        .px_4()
                                        .child(column),
                                ),
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .px_3()
                                .py_1()
                                .bg(rgb(PANEL))
                                .border_t_1()
                                .border_color(rgb(HAIRLINE))
                                .child(small(
                                    "static document · diagrams pre-rendered · relay probe is \
                                     the only live data",
                                    FAINT,
                                ))
                                .child(small(
                                    "open source: OpenAgentsInc/immortal · openagents · omega",
                                    FAINT,
                                )),
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
                    ui_font: gpui::font("Lilex"),
                    buffer_font: gpui::font("Lilex"),
                }),
                cx,
            );
            theme::init(theme::LoadThemes::JustBase, cx);
            apply_aiur_theme(cx);
            let bounds = Bounds::centered(None, size(px(1280.), px(900.)), cx);
            match cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(InfraDocument::new),
            ) {
                Ok(_) => cx.activate(true),
                Err(error) => console::error_1(&JsValue::from_str(&format!(
                    "failed to open the infrastructure document: {error:#}"
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
    eprintln!("infra_explainer_web builds for wasm32-unknown-unknown")
}
