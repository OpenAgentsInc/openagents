//! The web runner: one zone release in a platform web view (ADR 0010).
//!
//! Rust decides what the web view may load, whether it can run the zone
//! (WebGPU), what the loading and failure cards say, and every bridge frame.
//! The iOS `WKWebView` and Android `WebView` hosts only forward events here
//! and paint what comes back.
use super::*;
use aethyrforge_host::{
    Action, BOOTSTRAP, MAX_BYTES, Message, NATIVE_CHANNEL, NativeEvent, NativeHost, NativePeer,
    zone_navigation_allowed,
};
use rust_native::style::{Space, Style};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use serde::{Deserialize, Serialize};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

/// The page-visible name of the shell's own probe channel (WebGPU report).
/// Separate from [`NATIVE_CHANNEL`] so a probe can never be read as protocol.
pub const PROBE_CHANNEL: &str = "aethyrShell";
const MAX_PROBE_BYTES: usize = 4 * 1024;
const MAX_ADAPTER_TEXT: usize = 256;

/// Which platform's web view hosts the runner; it decides how the page
/// reaches the native channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ZoneWebPlatform {
    /// `WKScriptMessageHandler`s named [`NATIVE_CHANNEL`] and [`PROBE_CHANNEL`].
    Ios,
    /// `WebViewCompat.addWebMessageListener` objects with those names.
    Android,
}

/// The WebGPU adapter the web view reported.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct ZoneGpuAdapter {
    pub vendor: String,
    pub architecture: String,
    pub device: String,
    pub description: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    kind: String,
    available: bool,
    adapter: Option<ZoneGpuAdapter>,
    reason: Option<String>,
    user_agent: Option<String>,
}

/// What the shell shows.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ZoneRunnerPhase {
    /// The page or the WebGPU check is still on its way: show the card.
    Loading,
    /// Show the web view, with no card over it.
    Ready,
    /// The page didn't load: show the card with Retry.
    Failed { message: String },
    /// This web view can't run the zone: show the card with Open in browser.
    Unavailable,
}

/// What a card button asks the shell to do.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ZoneRunnerAction {
    /// Reload the zone's page.
    Retry,
    /// Close the runner.
    Close,
    /// Open this website page in the system browser.
    OpenInBrowser { url: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
enum CardIntent {
    Retry,
    Close,
    Browser,
}

/// The result of a frame from the page.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ZoneBridgeReply {
    /// Frames to deliver to the page's `window.aethyrHost.onmessage`, in order.
    pub deliver: Vec<String>,
    /// The bridge refused the frame (wrong origin, subframe, size, shape).
    pub refused: bool,
}

pub(crate) struct Runner {
    pub zone: Zone,
    origin: String,
    phase: ZoneRunnerPhase,
    page_loaded: bool,
    gpu: Option<Result<ZoneGpuAdapter, String>>,
    user_agent: Option<String>,
    bridge: NativeHost,
    instance: String,
    revision: u64,
    view: Option<ValidatedView<CardIntent>>,
}

fn session() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

impl Runner {
    pub fn new(zone: Zone) -> Result<Self, ZoneError> {
        let release = zone
            .release
            .clone()
            .filter(|_| zone.manifest.runner == ZoneRunnerKind::Web)
            .ok_or_else(|| invalid("That zone doesn't open in a web view."))?;
        let bridge = NativeHost::new(
            &release.engine_url,
            session(),
            release.release.clone(),
            release.engine_abi.clone(),
            release.project_format,
            release.project.clone(),
        )
        .map_err(|_| invalid("That zone's release can't be opened."))?;
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Ok(Self {
            origin: release.origin(),
            zone,
            phase: ZoneRunnerPhase::Loading,
            page_loaded: false,
            gpu: None,
            user_agent: None,
            bridge,
            instance: format!("zone-runner-{}", NEXT.fetch_add(1, Ordering::Relaxed)),
            revision: 0,
            view: None,
        })
    }
    pub fn url(&self) -> &str {
        self.zone.manifest.url.as_deref().unwrap_or_default()
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn allows(&self, url: &str) -> bool {
        zone_navigation_allowed(&self.origin, url)
    }
    pub fn phase(&self) -> ZoneRunnerPhase {
        self.phase.clone()
    }
    fn settle(&mut self) {
        if matches!(
            self.phase,
            ZoneRunnerPhase::Failed { .. } | ZoneRunnerPhase::Unavailable
        ) {
            return;
        }
        self.phase = match &self.gpu {
            Some(Err(_)) => ZoneRunnerPhase::Unavailable,
            Some(Ok(_)) if self.page_loaded => ZoneRunnerPhase::Ready,
            _ => ZoneRunnerPhase::Loading,
        };
    }
    /// A new document started loading: a new bridge session, and the card.
    pub fn page_started(&mut self) {
        let _ = self.bridge.reload(session());
        self.page_loaded = false;
        self.gpu = None;
        if !matches!(self.phase, ZoneRunnerPhase::Unavailable) {
            self.phase = ZoneRunnerPhase::Loading;
        }
    }
    pub fn page_finished(&mut self) {
        self.page_loaded = true;
        self.settle();
    }
    pub fn page_failed(&mut self, offline: bool) {
        self.bridge.shutdown();
        self.phase = ZoneRunnerPhase::Failed {
            message: if offline {
                "You're offline. Connect and try again.".into()
            } else {
                format!(
                    "{} didn't open. Try again in a moment.",
                    self.zone.manifest.title
                )
            },
        };
    }
    pub fn retry(&mut self) {
        self.phase = ZoneRunnerPhase::Loading;
        self.page_loaded = false;
        self.gpu = None;
    }
    pub fn gpu(&self) -> Option<ZoneGpuAdapter> {
        self.gpu.as_ref().and_then(|g| g.as_ref().ok().cloned())
    }
    pub fn gpu_reason(&self) -> Option<String> {
        self.gpu.as_ref().and_then(|g| g.as_ref().err().cloned())
    }
    pub fn user_agent(&self) -> Option<String> {
        self.user_agent.clone()
    }
    /// The probe's report, from the zone's main frame only.
    pub fn probe(&mut self, peer: NativePeer<'_>, text: &str) -> Result<(), ZoneError> {
        let bad = || invalid("The page sent an invalid report.");
        if text.len() > MAX_PROBE_BYTES || !peer.main_frame || peer.origin != self.origin {
            return Err(bad());
        }
        let p: Probe = serde_json::from_str(text).map_err(|_| bad())?;
        let short = |s: &str| s.len() <= MAX_ADAPTER_TEXT && !s.contains('\0');
        if p.kind != "webgpu"
            || p.reason.as_deref().is_some_and(|r| !short(r))
            || p.user_agent.as_deref().is_some_and(|u| u.len() > 1024)
            || p.adapter.as_ref().is_some_and(|a| {
                !short(&a.vendor)
                    || !short(&a.architecture)
                    || !short(&a.device)
                    || !short(&a.description)
            })
        {
            return Err(bad());
        }
        self.user_agent = p.user_agent;
        self.gpu = Some(match (p.available, p.adapter) {
            (true, Some(adapter)) => Ok(adapter),
            (true, None) => Ok(ZoneGpuAdapter {
                vendor: String::new(),
                architecture: String::new(),
                device: String::new(),
                description: String::new(),
            }),
            (false, _) => Err(p.reason.unwrap_or_else(|| "no-adapter".into())),
        });
        self.settle();
        Ok(())
    }
    /// A frame on [`NATIVE_CHANNEL`]. Answers the bootstrap, and opens the
    /// zone as a guest once the engine is ready.
    pub fn bridge(&mut self, peer: NativePeer<'_>, text: &str, now: u64) -> ZoneBridgeReply {
        let refused = ZoneBridgeReply {
            deliver: vec![],
            refused: true,
        };
        self.bridge.expire(now);
        match self.bridge.receive(peer, text) {
            Ok(NativeEvent::Hello(hello)) => ZoneBridgeReply {
                deliver: vec![hello],
                refused: false,
            },
            Ok(NativeEvent::Message(m)) if matches!(*m, Message::Ready { .. }) => {
                match self.bridge.request(Action::Open { checkpoint: None }, now) {
                    Ok(open) => ZoneBridgeReply {
                        deliver: vec![open],
                        refused: false,
                    },
                    Err(_) => refused,
                }
            }
            Ok(NativeEvent::Message(_)) => ZoneBridgeReply {
                deliver: vec![],
                refused: false,
            },
            Err(_) => refused,
        }
    }
    pub fn bridge_phase(&self) -> aethyrforge_host::Phase {
        self.bridge.host().phase
    }
    /// The card over the web view, or `None` when the zone is showing.
    pub fn card(&mut self) -> Result<Option<String>, ZoneError> {
        let title = self.zone.manifest.title.clone();
        let (heading, body, buttons): (String, String, Vec<(&str, String, CardIntent)>) =
            match &self.phase {
                ZoneRunnerPhase::Ready => {
                    self.view = None;
                    return Ok(None);
                }
                ZoneRunnerPhase::Loading => (
                    format!("Opening {title}"),
                    "Getting the world ready.".into(),
                    vec![("close", "Close".into(), CardIntent::Close)],
                ),
                ZoneRunnerPhase::Failed { message } => (
                    format!("{title} didn't open"),
                    message.clone(),
                    vec![
                        ("retry", "Try again".into(), CardIntent::Retry),
                        ("close", "Close".into(), CardIntent::Close),
                    ],
                ),
                ZoneRunnerPhase::Unavailable => (
                    format!("{title} can't run here yet"),
                    "This phone's web view can't draw the world yet. You can open it in your browser instead.".into(),
                    vec![
                        ("browser", "Open in browser".into(), CardIntent::Browser),
                        ("close", "Close".into(), CardIntent::Close),
                    ],
                ),
            };
        self.revision += 1;
        let mut children = vec![
            text_node("heading", heading, TextRole::Heading),
            text_node("body", body, TextRole::Body),
        ];
        children.extend(buttons.into_iter().map(|(key, label, intent)| Node {
            key: key.into(),
            style: Style::default(),
            element: Element::Button {
                label,
                enabled: true,
                icon: None,
                shortcut: None,
                intent,
            },
        }));
        let view = View::new(
            &self.instance,
            self.revision,
            Node {
                key: "zone-card".into(),
                style: Style {
                    gap: Some(Space::Md),
                    padding_top: Some(Space::Lg),
                    padding_bottom: Some(Space::Lg),
                    padding_start: Some(Space::Lg),
                    padding_end: Some(Space::Lg),
                    radius: Some(16),
                    ..Style::default()
                },
                element: Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            },
        )
        .validate()
        .map_err(|_| invalid("The card could not be drawn."))?;
        let bytes = view
            .to_json()
            .map_err(|_| invalid("The card could not be drawn."))?;
        self.view = Some(view);
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| invalid("The card could not be drawn."))
    }
    pub fn activate(&mut self, activation: &Activation) -> Result<ZoneRunnerAction, ZoneError> {
        let intent = *self
            .view
            .as_ref()
            .ok_or_else(|| invalid("This card changed. Try again."))?
            .activate(activation)
            .map_err(|_| invalid("This card changed. Try again."))?;
        Ok(match intent {
            CardIntent::Retry => {
                self.retry();
                ZoneRunnerAction::Retry
            }
            CardIntent::Close => {
                self.bridge.shutdown();
                ZoneRunnerAction::Close
            }
            CardIntent::Browser => ZoneRunnerAction::OpenInBrowser {
                url: self
                    .zone
                    .manifest
                    .page_url
                    .clone()
                    .ok_or_else(|| invalid("This zone has no web page."))?,
            },
        })
    }
}

fn text_node(key: &str, value: String, role: TextRole) -> Node<CardIntent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text { value, role },
    }
}

/// The WebGPU probe, injected at document end into the zone's main frame.
/// It reports `'gpu' in navigator` and the `requestAdapter()` result over
/// [`PROBE_CHANNEL`]. It reads nothing else and sends no credentials.
pub fn probe_script(platform: ZoneWebPlatform) -> String {
    let post = match platform {
        ZoneWebPlatform::Ios => {
            format!("window.webkit.messageHandlers.{PROBE_CHANNEL}.postMessage")
        }
        ZoneWebPlatform::Android => format!("window.{PROBE_CHANNEL}.postMessage"),
    };
    format!(
        r#"(async () => {{
  const send = (r) => {{ try {{ {post}(JSON.stringify(r)); }} catch (_) {{}} }};
  const s = (v) => String(v == null ? "" : v).slice(0, 256);
  const r = {{ kind: "webgpu", available: false, adapter: null, reason: null, user_agent: s(navigator.userAgent) }};
  try {{
    if (!("gpu" in navigator) || !navigator.gpu) {{ r.reason = "no-webgpu"; return send(r); }}
    const a = await navigator.gpu.requestAdapter();
    if (!a) {{ r.reason = "no-adapter"; return send(r); }}
    let i = a.info;
    if (!i && typeof a.requestAdapterInfo === "function") {{ try {{ i = await a.requestAdapterInfo(); }} catch (_) {{}} }}
    i = i || {{}};
    r.available = true;
    r.adapter = {{ vendor: s(i.vendor), architecture: s(i.architecture), device: s(i.device), description: s(i.description) }};
  }} catch (_) {{ r.reason = "error"; }}
  send(r);
}})();"#
    )
}

/// iOS only: give the page the same `window.aethyrHost` object Android's
/// `addWebMessageListener` injects, backed by the script message handler.
/// Installed at document start in the main frame's page world.
pub fn ios_channel_script() -> String {
    format!(
        r#"(() => {{
  if (window.top !== window || window.{NATIVE_CHANNEL}) return;
  const handler = window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.{NATIVE_CHANNEL};
  if (!handler) return;
  const channel = {{ onmessage: null, postMessage(text) {{ if (typeof text === "string") handler.postMessage(text); }} }};
  Object.defineProperty(window, "{NATIVE_CHANNEL}", {{ value: channel, writable: false, configurable: false }});
}})();"#
    )
}

/// iOS only: the body `callAsyncJavaScript` runs, with `text` passed as an
/// argument (never spliced into source), to deliver one frame to the page.
pub fn ios_deliver_script() -> String {
    format!(
        r#"const c = window.{NATIVE_CHANNEL}; if (c && typeof c.onmessage === "function") c.onmessage({{ data: text }});"#
    )
}

/// One open zone in one web view.
#[derive(uniffi::Object)]
pub struct CoreZoneRunner {
    manifest: ZoneManifest,
    state: Mutex<Runner>,
}
impl CoreZoneRunner {
    pub(crate) fn open(zone: Zone) -> Result<Self, ZoneError> {
        Ok(Self {
            manifest: zone.manifest.clone(),
            state: Mutex::new(Runner::new(zone)?),
        })
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Runner>, ZoneError> {
        self.state.lock().map_err(|_| invalid("This zone closed."))
    }
}

#[uniffi::export]
impl CoreZoneRunner {
    pub fn manifest(&self) -> ZoneManifest {
        self.manifest.clone()
    }
    /// The page the web view loads first.
    pub fn url(&self) -> String {
        self.lock().map(|s| s.url().to_owned()).unwrap_or_default()
    }
    /// The exact origin the web view may show and accept frames from.
    pub fn origin(&self) -> String {
        self.lock()
            .map(|s| s.origin().to_owned())
            .unwrap_or_default()
    }
    pub fn channel_name(&self) -> String {
        NATIVE_CHANNEL.into()
    }
    pub fn probe_channel_name(&self) -> String {
        PROBE_CHANNEL.into()
    }
    pub fn probe_script(&self, platform: ZoneWebPlatform) -> String {
        probe_script(platform)
    }
    pub fn ios_channel_script(&self) -> String {
        ios_channel_script()
    }
    pub fn ios_deliver_script(&self) -> String {
        ios_deliver_script()
    }
    /// The largest frame either channel carries.
    pub fn max_frame_bytes(&self) -> u64 {
        MAX_BYTES as u64
    }
    /// May the web view navigate (or a subresource frame load) to `url`?
    /// Anything else is cancelled; it never opens elsewhere silently.
    pub fn allows_navigation(&self, url: String) -> bool {
        self.lock().is_ok_and(|s| s.allows(&url))
    }
    pub fn phase(&self) -> ZoneRunnerPhase {
        self.lock()
            .map(|s| s.phase())
            .unwrap_or(ZoneRunnerPhase::Failed {
                message: "This zone closed.".into(),
            })
    }
    pub fn page_started(&self) {
        if let Ok(mut s) = self.lock() {
            s.page_started()
        }
    }
    pub fn page_finished(&self) {
        if let Ok(mut s) = self.lock() {
            s.page_finished()
        }
    }
    pub fn page_failed(&self, offline: bool) {
        if let Ok(mut s) = self.lock() {
            s.page_failed(offline)
        }
    }
    /// A frame on the probe channel.
    pub fn probe(&self, origin: String, main_frame: bool, text: String) -> Result<(), ZoneError> {
        self.lock()?.probe(
            NativePeer {
                origin: &origin,
                main_frame,
            },
            &text,
        )
    }
    /// A frame on the bridge channel; deliver what comes back, in order.
    pub fn bridge(
        &self,
        origin: String,
        main_frame: bool,
        text: String,
        now_ms: u64,
    ) -> ZoneBridgeReply {
        match self.lock() {
            Ok(mut s) => s.bridge(
                NativePeer {
                    origin: &origin,
                    main_frame,
                },
                &text,
                now_ms,
            ),
            Err(_) => ZoneBridgeReply {
                deliver: vec![],
                refused: true,
            },
        }
    }
    /// The WebGPU adapter the page reported, once it has.
    pub fn gpu_adapter(&self) -> Option<ZoneGpuAdapter> {
        self.lock().ok().and_then(|s| s.gpu())
    }
    /// Why WebGPU is missing (`no-webgpu`, `no-adapter`, `error`), if it is.
    pub fn gpu_missing_reason(&self) -> Option<String> {
        self.lock().ok().and_then(|s| s.gpu_reason())
    }
    pub fn user_agent(&self) -> Option<String> {
        self.lock().ok().and_then(|s| s.user_agent())
    }
    /// The card to show over the web view (Rust Native JSON), or none.
    pub fn card(&self) -> Result<Option<String>, ZoneError> {
        self.lock()?.card()
    }
    pub fn activate(
        &self,
        instance: String,
        revision: u64,
        node: String,
    ) -> Result<ZoneRunnerAction, ZoneError> {
        self.lock()?.activate(&Activation {
            instance,
            revision,
            node,
        })
    }
    /// The bootstrap string an engine page sends first (for diagnostics).
    pub fn bootstrap(&self) -> String {
        BOOTSTRAP.into()
    }
}
