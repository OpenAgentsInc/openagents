//! Native Video Lab cards (OpenAgents Labs plan: `lab.card.v1`, view Video).
//!
//! Coder's MCP server shows each Video Lab result as a visual whose script
//! starts with a marker line, `-- coder:video_lab {card}`, carrying the Lab's
//! card as JSON. Clients that only know visuals draw the script (title,
//! picture, links). This one draws the card natively instead:
//!
//! - it follows the render: while the Lab is still rendering, it asks the Lab
//!   for the card again every two seconds and swaps in the video when it lands;
//! - it plays the video inline (macOS: [`player`], AVFoundation frames drawn by
//!   GPUI), with play/pause, time and a seek bar, and plays either side of a
//!   compare;
//! - "Open in the Lab player" opens the Lab's player and timeline in a Browser
//!   tab, as transcript links do.
//!
//! The card's addresses come from the transcript, so only the Lab on this
//! computer is ever fetched or played (`http://127.0.0.1` or `localhost`).

#[cfg(target_os = "macos")]
mod player;

use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Bounds, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    Render, RenderImage, SharedString, StatefulInteractiveElement, Styled, StyledImage as _, Task,
    Window, canvas, div, img, px, relative,
};
use serde_json::Value;

use crate::markdown::render::LinkUi;
use crate::theme::Theme;

/// The marker that turns a visual into a Lab card.
pub const MARKER: &str = "-- coder:video_lab ";
const POLL: Duration = Duration::from_secs(2);
const MAX_FETCH_BYTES: usize = 8 * 1024 * 1024;

/// The card JSON a visual script carries, if it is a Video Lab card.
pub fn card_in_script(script: &str) -> Option<Value> {
    let line = script.lines().next()?.strip_prefix(MARKER)?;
    let card: Value = serde_json::from_str(line).ok()?;
    (card.get("card").and_then(Value::as_str) == Some("video_lab")).then_some(card)
}

/// Only the Lab on this computer: plain HTTP to loopback.
pub fn local_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let host = rest.split(['/', '?']).next().unwrap_or_default();
    let host = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    matches!(host, "127.0.0.1" | "localhost")
}

pub struct LabCard {
    id: SharedString,
    card: Value,
    picture: Option<Arc<RenderImage>>,
    picture_url: Option<String>,
    link_ui: Option<LinkUi>,
    #[cfg(target_os = "macos")]
    player: Option<player::VideoPlayer>,
    /// The video the player has (the card's own, or a compared version's).
    playing_url: Option<String>,
    seek_bounds: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    retired: Vec<Arc<RenderImage>>,
    _poll: Option<Task<()>>,
}

impl LabCard {
    pub fn new(
        id: SharedString,
        card: Value,
        picture: Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.on_release(|this, cx| {
            let mut images = std::mem::take(&mut this.retired);
            images.extend(this.picture.take());
            if !images.is_empty() {
                cx.defer(move |cx| {
                    for image in images {
                        cx.drop_image(image, None);
                    }
                });
            }
        })
        .detach();
        let mut this = Self {
            id,
            picture_url: picture_url(&card),
            card,
            picture,
            link_ui: None,
            #[cfg(target_os = "macos")]
            player: None,
            playing_url: None,
            seek_bounds: Default::default(),
            retired: Vec::new(),
            _poll: None,
        };
        this.follow(cx);
        this
    }

    pub fn set_link_ui(&mut self, link_ui: Option<LinkUi>) {
        self.link_ui = link_ui;
    }

    fn text(&self, key: &str) -> String {
        self.card
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }

    fn rendering(&self) -> bool {
        self.card.get("video").is_none_or(Value::is_null)
            && self
                .card
                .get("job")
                .and_then(|j| j.get("state"))
                .and_then(Value::as_str)
                .is_some_and(|s| matches!(s, "queued" | "running"))
    }

    /// While the Lab renders, ask it for the card again until it's done.
    fn follow(&mut self, cx: &mut Context<Self>) {
        if !self.rendering() {
            self._poll = None;
            return;
        }
        let Some(url) = self.card_url() else {
            return;
        };
        self._poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let url = url.clone();
                let fetched = cx
                    .background_executor()
                    .spawn(async move {
                        get(&url).and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    })
                    .await;
                let Some(mut card) = fetched else { continue };
                absolute(&mut card);
                let still = this
                    .update(cx, |this, cx| {
                        this.card = card;
                        this.refresh_picture(cx);
                        cx.notify();
                        this.rendering()
                    })
                    .unwrap_or(false);
                if !still {
                    break;
                }
            }
        }));
    }

    /// `GET /api/card` for this card's document, version and view.
    fn card_url(&self) -> Option<String> {
        let lab = self.text("lab");
        if !local_url(&lab) {
            return None;
        }
        let mut url = format!(
            "{lab}/api/card?doc={}&v={}&q={}",
            encode(&self.text("doc")),
            self.card.get("version").and_then(Value::as_u64)?,
            encode(&self.text("quality")),
        );
        for key in ["from", "to"] {
            if let Some(t) = self.card.get(key).and_then(Value::as_f64) {
                url.push_str(&format!("&{key}={t}"));
            }
        }
        if let Some(compare) = self.card.get("compare") {
            if let Some(a) = compare["a"]["version"].as_u64() {
                url.push_str(&format!(
                    "&a={a}&at={}",
                    compare["at"].as_f64().unwrap_or_default()
                ));
            }
        }
        Some(url)
    }

    /// A finished render brings a poster; show it in place of the old picture
    /// when the card had none of its own (a frame or compare stays).
    fn refresh_picture(&mut self, cx: &mut Context<Self>) {
        let Some(url) = picture_url(&self.card) else {
            return;
        };
        if self.picture.is_some() && self.picture_url.as_deref() == Some(url.as_str()) {
            return;
        }
        if self.picture.is_some() && self.card.get("image").is_some_and(|v| !v.is_null()) {
            return;
        }
        self.picture_url = Some(url.clone());
        cx.spawn(async move |this, cx| {
            let decoded = cx
                .background_executor()
                .spawn(async move { get(&url).and_then(decode) })
                .await;
            if let Some(image) = decoded {
                let _ = this.update(cx, |this, cx| {
                    if let Some(old) = this.picture.replace(image) {
                        this.retired.push(old);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    #[cfg(target_os = "macos")]
    fn toggle(&mut self, url: Option<String>, cx: &mut Context<Self>) {
        let Some(url) = url
            .or_else(|| self.playing_url.clone())
            .filter(|u| local_url(u))
        else {
            return;
        };
        if self.playing_url.as_deref() != Some(url.as_str()) {
            self.player = player::VideoPlayer::new(&url);
            self.playing_url = Some(url);
            if let (Some(player), Some(start)) =
                (&self.player, self.card.get("from").and_then(Value::as_f64))
            {
                player.seek(start);
            }
            if let Some(player) = &self.player {
                player.play();
            }
        } else if let Some(player) = &self.player {
            if player.playing() {
                player.pause();
            } else {
                player.play();
            }
        }
        cx.notify();
    }

    #[cfg(not(target_os = "macos"))]
    fn toggle(&mut self, url: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let _ = (url, window, cx);
    }

    fn open(&self, url: &str, window: &mut Window, cx: &mut App) {
        crate::markdown::render::activate_link(
            crate::markdown::links::LinkTarget::new(url, url),
            crate::markdown::links::LinkAction::Primary,
            self.link_ui.as_ref(),
            window,
            cx,
        );
    }

    fn state_line(&self) -> Option<String> {
        if self.card.get("video").is_some_and(|v| !v.is_null()) {
            return Some(format!("{} ready", capital(&self.text("quality"))));
        }
        let job = self.card.get("job").filter(|j| !j.is_null())?;
        let quality = self.text("quality");
        Some(match job["state"].as_str()? {
            "failed" => format!(
                "The {quality} render failed: {}",
                job["error"].as_str().unwrap_or("see the Lab log")
            ),
            "done" => format!("{} ready", capital(&quality)),
            state => {
                let what = if state == "queued" {
                    "Waiting to render"
                } else {
                    "Rendering"
                };
                match job["estimate"].as_f64().filter(|e| *e > 0.0) {
                    Some(e) => format!(
                        "{what} the {quality}, about {} min…",
                        (e / 60.0).ceil() as u64
                    ),
                    None => format!("{what} the {quality}…"),
                }
            }
        })
    }
}

impl Render for LabCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let small = crate::typography::ui_rems(12.0);
        let body = crate::typography::ui_rems(14.0);
        let id = self.id.clone();
        let eid = move |part: &str| SharedString::from(format!("{id}:lab:{part}"));

        let mut meta = String::new();
        if let Some(v) = self.card.get("version").and_then(Value::as_u64) {
            meta.push_str(&format!("v{v}"));
        }
        let summary = self.text("summary");
        if !summary.is_empty() {
            meta.push_str(&format!(
                "{}{summary}",
                if meta.is_empty() { "" } else { " · " }
            ));
        }
        let video = self
            .card
            .get("video")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let compare = self.card.get("compare").cloned().filter(|c| !c.is_null());

        // The picture area: the playing video, else the picture.
        #[cfg(target_os = "macos")]
        let (frame, playing, position, duration) = match self.player.as_mut() {
            Some(player) => {
                let frame = player.frame();
                let playing = player.playing();
                if playing {
                    window.request_animation_frame();
                }
                (frame, playing, player.position(), player.duration())
            }
            None => (None, false, 0.0, 0.0),
        };
        #[cfg(not(target_os = "macos"))]
        let (playing, position, duration) = (false, 0.0f64, 0.0f64);

        let mut screen = div()
            .id(eid("screen"))
            .relative()
            .w_full()
            .aspect_ratio(16.0 / 9.0)
            .rounded(px(Theme::CONTROL_RADIUS))
            .overflow_hidden()
            .bg(gpui::black().opacity(0.35));
        #[cfg(target_os = "macos")]
        let showing_video = frame.is_some();
        #[cfg(not(target_os = "macos"))]
        let showing_video = false;
        #[cfg(target_os = "macos")]
        if let Some(frame) = frame {
            screen = screen.child(
                gpui::surface(frame)
                    .object_fit(gpui::ObjectFit::Contain)
                    .size_full(),
            );
        }
        if !showing_video {
            screen = match &self.picture {
                Some(picture) => screen.child(
                    img(picture.clone())
                        .object_fit(gpui::ObjectFit::Contain)
                        .size_full(),
                ),
                None => screen.child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(small)
                        .text_color(theme.text_faint)
                        .child(if self.rendering() {
                            "Rendering…"
                        } else {
                            "No picture yet"
                        }),
                ),
            };
        }
        let can_play = cfg!(target_os = "macos") && video.as_deref().is_some_and(local_url);
        if can_play && !playing {
            let url = if self.playing_url.is_some() {
                None
            } else {
                video.clone()
            };
            screen = screen.cursor_pointer().child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .px(px(14.0))
                            .py(px(8.0))
                            .rounded_full()
                            .bg(gpui::black().opacity(0.6))
                            .text_color(gpui::white())
                            .text_size(body)
                            .child("▶  Play"),
                    ),
            );
            #[cfg(target_os = "macos")]
            {
                screen = screen
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(url.clone(), cx)));
            }
        } else if playing {
            #[cfg(target_os = "macos")]
            {
                screen = screen
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.toggle(None, cx)));
            }
        }

        // Transport: play/pause, time, and a seek bar, once a video is loaded.
        let transport = (self.playing_url.is_some() && duration > 0.0).then(|| {
            let fraction = (position / duration).clamp(0.0, 1.0) as f32;
            let bounds = self.seek_bounds.clone();
            let bar_bounds = self.seek_bounds.clone();
            let mut bar = div()
                .id(eid("seek"))
                .flex_1()
                .h(px(14.0))
                .flex()
                .items_center()
                .cursor_pointer()
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(px(4.0))
                        .rounded_full()
                        .bg(theme.text_faint.opacity(0.35))
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .h_full()
                                .w(relative(fraction))
                                .rounded_full()
                                .bg(theme.accent),
                        ),
                )
                .child(
                    canvas(move |b, _, _| bar_bounds.set(Some(b)), |_, _, _, _| {})
                        .absolute()
                        .inset_0(),
                );
            #[cfg(target_os = "macos")]
            {
                bar = bar.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                        if let (Some(b), Some(player)) = (bounds.get(), &this.player) {
                            let x = (event.position.x - b.origin.x) / b.size.width;
                            player.seek(f64::from(x.clamp(0.0, 1.0)) * player.duration());
                            cx.notify();
                        }
                    }),
                );
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (bounds, MouseButton::Left);
            let mut button = div()
                .id(eid("play"))
                .cursor_pointer()
                .text_size(body)
                .text_color(theme.text)
                .child(if playing { "Pause" } else { "Play" });
            #[cfg(target_os = "macos")]
            {
                button = button.on_click(cx.listener(|this, _, _, cx| this.toggle(None, cx)));
            }
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(button)
                .child(bar)
                .child(
                    div()
                        .text_size(small)
                        .text_color(theme.text_muted)
                        .child(format!("{} / {}", clock(position), clock(duration))),
                )
        });

        // Links: inline plays, then the player.
        let mut links = div().flex().flex_wrap().gap(px(16.0));
        let link = |label: String, key: &str| {
            div()
                .id(eid(key))
                .cursor_pointer()
                .text_size(body)
                .text_color(theme.accent)
                .child(label)
        };
        if let Some(compare) = &compare {
            for side in ["a", "b"] {
                if let (Some(url), Some(v)) = (
                    compare[side]["video"].as_str(),
                    compare[side]["version"].as_u64(),
                ) {
                    if !(cfg!(target_os = "macos") && local_url(url)) {
                        continue;
                    }
                    let url = url.to_owned();
                    let mut el = link(format!("Play v{v}"), side);
                    #[cfg(target_os = "macos")]
                    {
                        el =
                            el.on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle(Some(url.clone()), cx)
                            }));
                    }
                    #[cfg(not(target_os = "macos"))]
                    let _ = url;
                    links = links.child(el);
                }
            }
        }
        let player_url = self.text("player");
        if !player_url.is_empty() {
            links = links.child(
                link("Open in the Lab player and timeline".into(), "player").on_click(
                    cx.listener(move |this, _, window, cx| this.open(&player_url, window, cx)),
                ),
            );
        }
        if !cfg!(target_os = "macos")
            && let Some(video) = video.clone()
        {
            links = links.child(
                link(format!("Play the {}", self.text("quality")), "video").on_click(
                    cx.listener(move |this, _, window, cx| this.open(&video, window, cx)),
                ),
            );
        }

        let private: Vec<String> = self
            .card
            .get("private")
            .and_then(Value::as_array)
            .map(|p| {
                p.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();

        div()
            .id(eid("card"))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(crate::typography::ui_rems(16.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(if self.text("title").is_empty() {
                        self.text("doc")
                    } else {
                        self.text("title")
                    }),
            )
            .when(!meta.is_empty(), |el| {
                el.child(
                    div()
                        .text_size(small)
                        .text_color(theme.text_muted)
                        .child(meta),
                )
            })
            .when_some(compare.as_ref(), |el, c| {
                let side = |s: &str| {
                    format!(
                        "v{} {}",
                        c[s]["version"].as_u64().unwrap_or_default(),
                        c[s]["summary"].as_str().unwrap_or_default()
                    )
                };
                el.child(div().text_size(small).text_color(theme.text).child(format!(
                    "Compare at {}: {}  |  {}",
                    clock(c["at"].as_f64().unwrap_or_default()),
                    side("a"),
                    side("b")
                )))
            })
            .child(screen)
            .when_some(transport, |el, t| el.child(t))
            .when_some(self.state_line(), |el, line| {
                el.child(
                    div()
                        .text_size(small)
                        .text_color(theme.text_muted)
                        .child(line),
                )
            })
            .when(!private.is_empty(), |el| {
                el.child(
                    div()
                        .text_size(crate::typography::ui_rems(11.0))
                        .text_color(theme.text_faint)
                        .child(format!(
                            "Stays on this Mac, not for training: {}",
                            private.join("; ")
                        )),
                )
            })
            .child(links)
            .into_any_element()
    }
}

fn picture_url(card: &Value) -> Option<String> {
    ["image", "poster"]
        .into_iter()
        .find_map(|k| card.get(k).and_then(Value::as_str).filter(|u| local_url(u)))
        .map(str::to_owned)
}

/// Media addresses in a fresh `/api/card` answer are Lab paths.
fn absolute(card: &mut Value) {
    let Some(lab) = card.get("lab").and_then(Value::as_str).map(str::to_owned) else {
        return;
    };
    let fix = |v: &mut Value| {
        if let Some(path) = v.as_str().filter(|p| p.starts_with('/')) {
            *v = Value::String(format!("{lab}{path}"));
        }
    };
    for key in ["video", "poster", "image"] {
        if let Some(v) = card.get_mut(key) {
            fix(v);
        }
    }
    for side in ["a", "b"] {
        if let Some(v) = card
            .get_mut("compare")
            .and_then(|c| c.get_mut(side))
            .and_then(|s| s.get_mut("video"))
        {
            fix(v);
        }
    }
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Render".into(),
    }
}

/// Decode a picture for GPUI (BGRA).
pub fn decode(bytes: Vec<u8>) -> Option<Arc<RenderImage>> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.limits(crate::image_media::raster_limits());
    let decoded = reader.decode().ok()?;
    let decoded = if decoded.width() > 1920 || decoded.height() > 1080 {
        decoded.thumbnail(1920, 1080)
    } else {
        decoded
    };
    let mut pixels = decoded.into_rgba8();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
}

/// A bounded GET from the Lab on this computer (HTTP/1.0, no redirects).
fn get(url: &str) -> Option<Vec<u8>> {
    use std::io::{Read as _, Write as _};
    if !local_url(url) {
        return None;
    }
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let address = if authority.contains(':') {
        authority.to_owned()
    } else {
        format!("{authority}:80")
    };
    let address = address.replacen("localhost", "127.0.0.1", 1);
    let mut stream =
        std::net::TcpStream::connect_timeout(&address.parse().ok()?, Duration::from_secs(2))
            .ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = Vec::new();
    stream
        .take(MAX_FETCH_BYTES as u64 + 64 * 1024)
        .read_to_end(&mut response)
        .ok()?;
    let split = response.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&response[..split]).ok()?;
    if !head
        .split_whitespace()
        .nth(1)
        .is_some_and(|code| code == "200")
    {
        return None;
    }
    let body = response[split + 4..].to_vec();
    (body.len() <= MAX_FETCH_BYTES).then_some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_labs_own_marker_makes_a_card() {
        let script =
            format!("{MARKER}{{\"card\":\"video_lab\",\"doc\":\"d\"}}\nreturn function() end");
        assert_eq!(card_in_script(&script).unwrap()["doc"], "d");
        assert!(card_in_script("return function() end").is_none());
        assert!(card_in_script(&format!("{MARKER}{{\"card\":\"other\"}}\n")).is_none());
        assert!(card_in_script(&format!("x\n{MARKER}{{\"card\":\"video_lab\"}}")).is_none());
    }

    #[test]
    fn only_this_computers_lab_is_reached() {
        assert!(local_url("http://127.0.0.1:8792/media/a.mp4"));
        assert!(local_url("http://localhost:8792/api/card?doc=x"));
        assert!(!local_url("https://127.0.0.1:8792/a"));
        assert!(!local_url("http://example.com/a.mp4"));
        assert!(!local_url("http://127.0.0.1.example.com/a"));
        assert!(!local_url("http://10.0.0.2:8792/a"));
    }

    #[test]
    fn fresh_cards_get_absolute_media() {
        let mut card = serde_json::json!({"lab": "http://127.0.0.1:8792", "video": "/media/v.mp4", "poster": null,
            "compare": {"a": {"video": "/media/a.mp4"}, "b": {"video": null}}});
        absolute(&mut card);
        assert_eq!(card["video"], "http://127.0.0.1:8792/media/v.mp4");
        assert_eq!(
            card["compare"]["a"]["video"],
            "http://127.0.0.1:8792/media/a.mp4"
        );
        assert!(card["poster"].is_null());
    }

    #[test]
    fn names_are_encoded_for_the_query() {
        assert_eq!(encode("episode 290/ü"), "episode%20290%2F%C3%BC");
        assert_eq!(clock(274.4), "4:34");
    }
}
