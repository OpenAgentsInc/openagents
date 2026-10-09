//! Link cards a slide shows in place of a web view: `scene: essays` lays
//! two of our essays side by side as GitHub file previews, and
//! `scene: download` shows openagents.com/download in a browser window.
//!
//! There is no browser engine here, so each card is drawn from the source
//! the page is made from, read at build time: the essays' Markdown from
//! `docs/essays/`, and the download page's Coder release and install
//! commands from the web crate's `pages/download.rs`. A card matches the
//! repository it was built from.
//!
//! An essay card has a header row (the GitHub mark, the repository, and
//! the file's path), the essay's title and date, its opening rendered
//! from the Markdown and fading out at the bottom, and the short URL at
//! the foot. The download card is a browser window with an address bar and
//! the page under it in the site's own monospace: white on near-black, or
//! black on near-white in Coder Light. A click on a card opens its URL in the browser; hovering
//! brightens it. The cards rise and fade in when the slide shows, one
//! after another, and show at once under Reduce motion.

use rust_native::layout::display::Weight;
use rust_native::style::{Color, TextAlign};
use rust_native_desktop::rich::Rich;
use rust_native_desktop::text::{Fonts, font};
use rust_native_desktop::{Frame, PxRect, Theme};
use std::time::Instant;

/// The scene that shows the essays as link cards.
pub const ESSAYS: &str = "essays";
/// The scene that shows openagents.com/download in a browser window.
pub const DOWNLOAD: &str = "download";
/// The download page's address.
pub const DOWNLOAD_URL: &str = "https://openagents.com/download";

/// The repository the essays live in.
const REPO: &str = "OpenAgentsInc/openagents";
/// Where a file in the repository opens on GitHub.
const BLOB: &str = "https://github.com/OpenAgentsInc/openagents/blob/main/";

/// The essays the cards show, by their path in the repository.
const ESSAY_SOURCES: [(&str, &str); 2] = [
    (
        "docs/essays/2026-10-01-the-return-of-the-general-agent.md",
        include_str!("../../../docs/essays/2026-10-01-the-return-of-the-general-agent.md"),
    ),
    (
        "docs/essays/2026-09-29-test-time-capabilities.md",
        include_str!("../../../docs/essays/2026-09-29-test-time-capabilities.md"),
    ),
];

/// The download page's source, for its versions and commands.
const DOWNLOAD_SOURCE: &str = include_str!("../../openagents-web/src/pages/download.rs");

/// The slide height the card sizes are given in; everything scales with
/// the slide.
const DESIGN: f32 = 640.0;
/// How long one card takes to come in, in seconds.
const ENTER: f32 = 0.7;
/// How long after the one before it the next card starts, in seconds.
const STAGGER: f32 = 0.16;
/// How far a card rises as it comes in, in design points.
const RISE: f32 = 26.0;

/// The GitHub mark (Octicons `mark-github`).
const GITHUB_MARK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16"><path fill="#fff" d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z"/></svg>"##;

/// A padlock, for the address bar.
const LOCK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16"><rect x="3" y="7" width="10" height="8" rx="1.6" fill="#fff"/><path d="M5.2 7V5.2a2.8 2.8 0 0 1 5.6 0V7" fill="none" stroke="#fff" stroke-width="1.6"/></svg>"##;

/// An essay as its card shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Essay {
    /// The file's path in the repository.
    pub path: &'static str,
    pub title: String,
    /// The date in the file name, written out ("October 1, 2026").
    pub date: String,
    /// The Markdown before the essay's second section: the opening
    /// paragraphs and its summary.
    pub opening: String,
}

impl Essay {
    /// The essay on GitHub.
    pub fn url(&self) -> String {
        format!("{BLOB}{}", self.path)
    }
}

/// The essays the `essays` scene shows, read from the repository.
pub fn essays() -> Vec<Essay> {
    ESSAY_SOURCES
        .iter()
        .map(|(path, source)| essay(path, source))
        .collect()
}

fn essay(path: &'static str, source: &str) -> Essay {
    let mut lines = source.lines();
    let title = lines
        .by_ref()
        .find_map(|line| line.strip_prefix("# "))
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut opening = Vec::new();
    let mut sections = 0;
    for line in lines {
        if line.starts_with("## ") {
            sections += 1;
            if sections == 2 {
                break;
            }
        }
        opening.push(line);
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    Essay {
        path,
        title,
        date: date(name.get(..10).unwrap_or_default()),
        opening: opening.join("\n").trim().to_string(),
    }
}

/// `2026-10-01` written out as "October 1, 2026".
fn date(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let mut parts = iso.split('-').map(|part| part.parse::<usize>().ok());
    match (parts.next(), parts.next(), parts.next()) {
        (Some(Some(year)), Some(Some(month)), Some(Some(day))) if (1..=12).contains(&month) => {
            format!("{} {day}, {year}", MONTHS[month - 1])
        }
        _ => iso.to_string(),
    }
}

/// What the download page offers, read from its source: Coder's release
/// candidate and its installers (the page is Coder only since #11030).
#[derive(Clone, Debug, PartialEq)]
pub struct Download {
    /// Coder's release candidate (`CODER_VERSION`).
    pub version: String,
    /// The install command for macOS and Linux (`CODER_SH`).
    pub sh: String,
    /// The install command for Windows, in PowerShell (`CODER_PS1`).
    pub ps1: String,
}

/// The download page's version and commands, from the web crate's
/// `pages/download.rs` constants.
pub fn download() -> Download {
    let constant = |name: &str| constant(DOWNLOAD_SOURCE, name).unwrap_or_default();
    Download {
        version: constant("CODER_VERSION"),
        sh: constant("CODER_SH"),
        ps1: constant("CODER_PS1"),
    }
}

/// The string a `const NAME: &str = "…";` in `source` holds.
fn constant(source: &str, name: &str) -> Option<String> {
    let start = source.find(&format!("const {name}: &str ="))?;
    let rest = &source[start..];
    let open = rest.find('"')? + 1;
    let close = rest[open..].find('"')?;
    Some(rest[open..open + close].to_string())
}

/// The cards' places in `slide`, in the slide's units.
pub fn cards(scene: &str, slide: PxRect) -> Vec<PxRect> {
    let k = slide.h / DESIGN;
    match scene {
        ESSAYS => {
            let (pad, top, gap) = (60.0 * k, 52.0 * k, 36.0 * k);
            let w = (slide.w - 2.0 * pad - gap) / 2.0;
            (0..2)
                .map(|index| PxRect {
                    x: slide.x + pad + index as f32 * (w + gap),
                    y: slide.y + top,
                    w,
                    h: slide.h - 2.0 * top,
                })
                .collect()
        }
        DOWNLOAD => {
            let (pad, top) = (56.0 * k, 36.0 * k);
            vec![PxRect {
                x: slide.x + pad,
                y: slide.y + top,
                w: slide.w - 2.0 * pad,
                h: slide.h - 2.0 * top,
            }]
        }
        _ => Vec::new(),
    }
}

/// Where the card at `index` on `scene` links.
pub fn url(scene: &str, index: usize) -> Option<String> {
    match scene {
        ESSAYS => essays().get(index).map(Essay::url),
        DOWNLOAD if index == 0 => Some(DOWNLOAD_URL.to_string()),
        _ => None,
    }
}

/// A gray at `level` of white on black in the dark look; in Coder Light the
/// same step of black on white (#11028).
fn gray(level: u8) -> Color {
    let level = match openagents_chat_app::visual::scheme() {
        openagents_chat_app::visual::Scheme::Dark => level,
        openagents_chat_app::visual::Scheme::Light => 255 - level,
    };
    Color::rgb(level, level, level)
}

fn alpha(color: Color, alpha: f32) -> Color {
    Color {
        alpha: (f32::from(color.alpha) * alpha.clamp(0.0, 1.0)).round() as u8,
        ..color
    }
}

fn eased(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The link cards of the slide showing, with their entrance and hover.
pub struct Embeds {
    scene: Option<&'static str>,
    shown_at: Option<Instant>,
    now: Option<Instant>,
    reduce_motion: bool,
    hover: Option<usize>,
    fonts: Fonts,
    essays: Vec<Essay>,
    download: Download,
    /// Each essay's opening laid out, with the width it was laid out at.
    openings: Vec<Option<(u32, Rich)>>,
    version: u64,
}

impl Embeds {
    pub fn new(reduce_motion: bool) -> Embeds {
        let essays = essays();
        Embeds {
            scene: None,
            shown_at: None,
            now: None,
            reduce_motion,
            hover: None,
            fonts: Fonts::new(),
            openings: essays.iter().map(|_| None).collect(),
            essays,
            download: download(),
            version: 0,
        }
    }

    /// Shows `scene` as of `now`, the frame clock's time: a scene newly
    /// shown starts its entrance. Returns whether the painting changed.
    pub fn show(&mut self, scene: &'static str, now: Instant) -> bool {
        let was = self.entering();
        if self.scene != Some(scene) || self.shown_at.is_none() {
            self.scene = Some(scene);
            self.shown_at = Some(now);
            self.hover = None;
            self.now = Some(now);
            self.version = self.version.wrapping_add(1);
            return true;
        }
        self.now = Some(now);
        if was {
            self.version = self.version.wrapping_add(1);
        }
        was
    }

    /// Off the slide: the next visit comes in again.
    pub fn reset(&mut self) {
        self.scene = None;
        self.shown_at = None;
        self.hover = None;
    }

    /// Whether the cards are still coming in.
    pub fn entering(&self) -> bool {
        let count = match self.scene {
            Some(ESSAYS) => 2,
            Some(_) => 1,
            None => return false,
        };
        !self.reduce_motion && self.elapsed() < ENTER + STAGGER * (count - 1) as f32
    }

    fn elapsed(&self) -> f32 {
        match (self.shown_at, self.now) {
            (Some(at), Some(now)) => now.saturating_duration_since(at).as_secs_f32(),
            _ => 0.0,
        }
    }

    /// How far the card at `index` has come in, 0 to 1.
    pub fn reveal(&self, index: usize) -> f32 {
        if self.reduce_motion {
            return 1.0;
        }
        eased((self.elapsed() - STAGGER * index as f32) / ENTER)
    }

    /// Marks the card at `index` as under the pointer. Returns whether
    /// that changed.
    pub fn set_hover(&mut self, index: Option<usize>) -> bool {
        if self.hover == index {
            return false;
        }
        self.hover = index;
        self.version = self.version.wrapping_add(1);
        true
    }

    pub fn hovered(&self) -> Option<usize> {
        self.hover
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// Whether `scene` still needs frames: it has yet to start, or its
    /// cards are coming in.
    pub fn pending(&self, scene: &str) -> bool {
        self.scene != Some(scene) || self.entering()
    }

    /// Paints `scene`'s cards over `slide`, in pixels. Before the scene's
    /// first tick only the slide's background shows, so the cards come in
    /// from nothing (at once under Reduce motion).
    pub fn paint(&mut self, frame: &mut Frame, slide: PxRect, scene: &'static str) {
        let background = openagents_chat_app::visual::current().canvas;
        frame.fill(slide, 0.0, background);
        if self.scene != Some(scene) && !self.reduce_motion {
            return;
        }
        let k = slide.h / DESIGN;
        let clip = frame.clip_to(slide);
        for (index, card) in cards(scene, slide).into_iter().enumerate() {
            let shown = if self.scene == Some(scene) {
                self.reveal(index)
            } else {
                1.0
            };
            if shown <= 0.0 {
                continue;
            }
            let card = PxRect {
                y: card.y + (1.0 - shown) * RISE * k,
                ..card
            };
            let hover = self.hover == Some(index);
            match scene {
                ESSAYS => self.essay_card(frame, card, k, index, hover),
                _ => self.download_card(frame, card, k, hover),
            }
            if shown < 1.0 {
                // Fade in: the slide's background over the card, lifting.
                let margin = 4.0 * k;
                frame.fill(
                    PxRect {
                        x: card.x - margin,
                        y: card.y - margin,
                        w: card.w + 2.0 * margin,
                        h: card.h + 2.0 * margin,
                    },
                    0.0,
                    alpha(background, 1.0 - shown),
                );
            }
        }
        frame.restore_clip(clip);
    }

    /// Text at `x`, `y` pixels; wrapped to `wrap` pixels when given.
    /// Returns its size.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        frame: &mut Frame,
        value: &str,
        x: f32,
        y: f32,
        size: f32,
        weight: Weight,
        mono: bool,
        color: Color,
        wrap: Option<f32>,
    ) -> (f32, f32) {
        let paragraph = self.fonts.paragraph(value, font(size, weight, mono), wrap);
        self.fonts.draw(
            frame,
            &paragraph,
            x,
            y,
            wrap.unwrap_or(paragraph.width + 1.0),
            TextAlign::Start,
            1.0,
            color,
        );
        (paragraph.width, paragraph.height)
    }

    fn essay_card(&mut self, frame: &mut Frame, card: PxRect, k: f32, index: usize, hover: bool) {
        let Some(essay) = self.essays.get(index).cloned() else {
            return;
        };
        let radius = 14.0 * k;
        let fill = gray(if hover { 22 } else { 16 });
        let rule = gray(if hover { 78 } else { 46 });
        frame.fill(card, radius, fill);
        // The header: the GitHub mark, the repository, and the path.
        let header = 64.0 * k;
        frame.fill_corners(
            PxRect { h: header, ..card },
            [radius, radius, 0.0, 0.0],
            gray(if hover { 28 } else { 22 }),
        );
        frame.fill(
            PxRect {
                y: card.y + header,
                h: k.max(1.0),
                ..card
            },
            0.0,
            rule,
        );
        let mark = 26.0 * k;
        let left = card.x + 22.0 * k;
        svg(
            frame,
            PxRect {
                x: left,
                y: card.y + (header - mark) / 2.0,
                w: mark,
                h: mark,
            },
            GITHUB_MARK,
            gray(236),
        );
        let text_x = left + mark + 14.0 * k;
        let width = card.x + card.w - 22.0 * k - text_x;
        let (owner, repo) = REPO.split_once('/').unwrap_or((REPO, ""));
        let size = 15.0 * k;
        let y = card.y + 13.0 * k;
        let w = self
            .fonts
            .advance(owner, font(size, Weight::Regular, false));
        let slash = self
            .fonts
            .advance(" / ", font(size, Weight::Regular, false));
        self.text(
            frame,
            owner,
            text_x,
            y,
            size,
            Weight::Regular,
            false,
            gray(190),
            None,
        );
        self.text(
            frame,
            " / ",
            text_x + w,
            y,
            size,
            Weight::Regular,
            false,
            gray(110),
            None,
        );
        self.text(
            frame,
            repo,
            text_x + w + slash,
            y,
            size,
            Weight::Semibold,
            false,
            gray(245),
            None,
        );
        let path_font = font(12.0 * k, Weight::Regular, true);
        let path = self.fonts.ellipsized(essay.path, path_font, width);
        self.text(
            frame,
            &path,
            text_x,
            card.y + 36.0 * k,
            12.0 * k,
            Weight::Regular,
            true,
            gray(140),
            None,
        );
        // The foot: the short URL, and where a click goes.
        let foot = 52.0 * k;
        let foot_y = card.y + card.h - foot;
        frame.fill(
            PxRect {
                y: foot_y,
                h: k.max(1.0),
                ..card
            },
            0.0,
            rule,
        );
        let foot_text = foot_y + (foot - 18.0 * k) / 2.0;
        self.text(
            frame,
            "github.com/OpenAgentsInc/openagents",
            card.x + 28.0 * k,
            foot_text,
            12.5 * k,
            Weight::Regular,
            true,
            gray(150),
            None,
        );
        let label = "Read on GitHub";
        let label_font = font(13.0 * k, Weight::Medium, false);
        let label_w = self.fonts.advance(label, label_font);
        let arrow = 9.0 * k;
        let label_x = card.x + card.w - 28.0 * k - arrow - 8.0 * k - label_w;
        let ink = gray(if hover { 250 } else { 175 });
        self.text(
            frame,
            label,
            label_x,
            foot_text - 0.5 * k,
            13.0 * k,
            Weight::Medium,
            false,
            ink,
            None,
        );
        // A small arrow out, up and to the right.
        let (ax, ay) = (card.x + card.w - 28.0 * k - arrow, foot_y + foot / 2.0);
        let stroke = 1.5 * k;
        frame.line(
            (ax, ay + arrow / 2.0),
            (ax + arrow, ay - arrow / 2.0),
            stroke,
            ink,
        );
        frame.line(
            (ax + arrow * 0.3, ay - arrow / 2.0),
            (ax + arrow, ay - arrow / 2.0),
            stroke,
            ink,
        );
        frame.line(
            (ax + arrow, ay - arrow / 2.0),
            (ax + arrow, ay + arrow * 0.2),
            stroke,
            ink,
        );
        // The body: the date, the title, and the opening, fading out.
        let pad = 30.0 * k;
        let inner = card.w - 2.0 * pad;
        let mut y = card.y + header + 26.0 * k;
        let (_, h) = self.text(
            frame,
            &format!("Essay \u{b7} {}", essay.date),
            card.x + pad,
            y,
            13.0 * k,
            Weight::Medium,
            false,
            gray(150),
            None,
        );
        y += h + 10.0 * k;
        let (_, h) = self.text(
            frame,
            &essay.title,
            card.x + pad,
            y,
            32.0 * k,
            Weight::Bold,
            false,
            gray(250),
            Some(inner),
        );
        y += h + 16.0 * k;
        let bottom = foot_y - 6.0 * k;
        if bottom > y {
            let magnification = 0.92;
            let width = inner / k / magnification;
            // Laid out again when the width or the scheme changes.
            let light =
                openagents_chat_app::visual::scheme() == openagents_chat_app::visual::Scheme::Light;
            let key = ((width.round() as u32) << 1) | u32::from(light);
            let stale = self.openings[index]
                .as_ref()
                .is_none_or(|(laid, _)| *laid != key);
            if stale {
                let node = rust_native::Node {
                    key: format!("essay-{index}"),
                    style: rust_native::style::Style::default(),
                    element: rust_native::Element::Markdown {
                        blocks: rust_native::markdown::parse(&essay.opening),
                    },
                };
                self.openings[index] =
                    Rich::in_family(node, width, Theme::openagents().font_family)
                        .ok()
                        .map(|mut rich| {
                            if light {
                                let look = &openagents_chat_app::visual::Visual::LIGHT;
                                rich.set_palette(&look.colors);
                                rich.set_syntax_palette(look.syntax);
                            }
                            (key, rich)
                        });
            }
            if let Some((_, rich)) = &self.openings[index] {
                let clip = frame.clip_to(PxRect {
                    x: card.x,
                    y,
                    w: card.w,
                    h: bottom - y,
                });
                rich.paint(
                    frame,
                    &mut self.fonts,
                    card.x + pad,
                    y,
                    k,
                    magnification,
                    Some(bottom),
                );
                frame.restore_clip(clip);
            }
            fade(
                frame,
                card.x,
                card.w,
                bottom,
                (bottom - y).min(150.0 * k),
                fill,
            );
        }
        frame.stroke(card, radius, k.max(1.0), rule);
    }

    fn download_card(&mut self, frame: &mut Frame, card: PxRect, k: f32, hover: bool) {
        let radius = 12.0 * k;
        let page_color = gray(10);
        let rule = gray(if hover { 96 } else { 58 });
        frame.fill(card, radius, page_color);
        // The window's bar: three dots and the address.
        let bar = 46.0 * k;
        frame.fill_corners(
            PxRect { h: bar, ..card },
            [radius, radius, 0.0, 0.0],
            gray(if hover { 32 } else { 26 }),
        );
        frame.fill(
            PxRect {
                y: card.y + bar,
                h: k.max(1.0),
                ..card
            },
            0.0,
            gray(44),
        );
        for dot in 0..3 {
            let d = 12.0 * k;
            frame.fill(
                PxRect {
                    x: card.x + 18.0 * k + dot as f32 * 20.0 * k,
                    y: card.y + (bar - d) / 2.0,
                    w: d,
                    h: d,
                },
                d / 2.0,
                gray(72),
            );
        }
        let address = PxRect {
            x: card.x + card.w * 0.28,
            y: card.y + 9.0 * k,
            w: card.w * 0.44,
            h: bar - 18.0 * k,
        };
        frame.fill(address, address.h / 2.0, gray(14));
        let (host, path) = ("openagents.com", "/download");
        let size = 14.0 * k;
        let host_w = self.fonts.advance(host, font(size, Weight::Medium, false));
        let path_w = self.fonts.advance(path, font(size, Weight::Regular, false));
        let lock = 12.0 * k;
        let gap = 8.0 * k;
        let start = address.x + (address.w - lock - gap - host_w - path_w) / 2.0;
        let line = size * rust_native_desktop::text::LINE_EM;
        svg(
            frame,
            PxRect {
                x: start,
                y: address.y + (address.h - lock) / 2.0,
                w: lock,
                h: lock,
            },
            LOCK,
            gray(150),
        );
        let text_y = address.y + (address.h - line) / 2.0;
        self.text(
            frame,
            host,
            start + lock + gap,
            text_y,
            size,
            Weight::Medium,
            false,
            gray(240),
            None,
        );
        self.text(
            frame,
            path,
            start + lock + gap + host_w,
            text_y,
            size,
            Weight::Regular,
            false,
            gray(150),
            None,
        );
        // The page, scaled so all of it fits.
        let page = PxRect {
            x: card.x + k,
            y: card.y + bar + k,
            w: card.w - 2.0 * k,
            h: card.h - bar - 2.0 * k,
        };
        let natural = self.site(None, page, 1.0);
        let scale = (page.h / natural).min(1.25 * k);
        let clip = frame.clip_to(page);
        self.site(Some(frame), page, scale);
        frame.restore_clip(clip);
        frame.stroke(card, radius, k.max(1.0), rule);
    }

    /// The download page in `area`, at `s` pixels a CSS pixel, after the
    /// site's stylesheet: a 14px monospace base, 1.5 line height, and
    /// four intensities of white. Without a frame it only measures.
    /// Returns its height in pixels.
    fn site(&mut self, mut frame: Option<&mut Frame>, area: PxRect, s: f32) -> f32 {
        const W25: u8 = 0x4a;
        const W50: u8 = 0x8a;
        const W75: u8 = 0xc8;
        const W100: u8 = 0xff;
        let download = self.download.clone();
        let base = 14.0 * s;
        let line = 21.0 * s;
        let ch = self.fonts.advance("0", font(base, Weight::Regular, true));
        // The page's column is 96ch; it widens to keep the longest
        // install command on its line when the window has room.
        let longest = download
            .sh
            .chars()
            .count()
            .max(download.ps1.chars().count()) as f32;
        let column = (96.0 * ch)
            .max((longest + 2.0) * ch + 2.0 * s)
            .min(area.w - 32.0 * s);
        let x0 = area.x + (area.w - column) / 2.0;
        let mut y = area.y;
        // A run of styled pieces on one line, left to right.
        let runs = |frame: &mut Option<&mut Frame>,
                    fonts: &mut Fonts,
                    x: f32,
                    y: f32,
                    size: f32,
                    pieces: &[(&str, Weight, u8, bool)]|
         -> f32 {
            let mut x = x;
            for (text, weight, level, underline) in pieces {
                let font = font(size, *weight, true);
                let width = fonts.advance(text, font);
                if let Some(frame) = frame.as_deref_mut() {
                    let paragraph = fonts.paragraph(text, font, None);
                    let top = y + (line - paragraph.line_height()) / 2.0;
                    fonts.draw(
                        frame,
                        &paragraph,
                        x,
                        top,
                        width + 1.0,
                        TextAlign::Start,
                        1.0,
                        gray(*level),
                    );
                    if *underline {
                        frame.fill(
                            PxRect {
                                x,
                                y: top + size * 1.22,
                                w: width,
                                h: s.max(1.0),
                            },
                            0.0,
                            gray(*level),
                        );
                    }
                }
                x += width;
            }
            x
        };
        let rule = |frame: &mut Option<&mut Frame>, rect: PxRect| {
            if let Some(frame) = frame.as_deref_mut() {
                frame.stroke(rect, 0.0, s.max(1.0), gray(W25));
            }
        };
        // The header: the wordmark, then the sections, and a rule under.
        let header = 10.5 * s * 2.0 + line;
        runs(
            &mut frame,
            &mut self.fonts,
            x0,
            y + 10.5 * s,
            base,
            &[("OpenAgents", Weight::Bold, W100, false)],
        );
        let docs = self
            .fonts
            .advance("Docs", font(base, Weight::Regular, true));
        let download_w = self
            .fonts
            .advance("Download", font(base, Weight::Regular, true));
        runs(
            &mut frame,
            &mut self.fonts,
            x0 + column - docs - 2.0 * ch - download_w,
            y + 10.5 * s,
            base,
            &[
                ("Download", Weight::Regular, W100, true),
                ("  ", Weight::Regular, W75, false),
                ("Docs", Weight::Regular, W75, false),
            ],
        );
        if let Some(frame) = frame.as_deref_mut() {
            frame.fill(
                PxRect {
                    x: area.x,
                    y: y + header,
                    w: area.w,
                    h: s.max(1.0),
                },
                0.0,
                gray(W25),
            );
        }
        y += header + 28.0 * s;
        // <h1>Download OpenAgents</h1>
        let h1 = 19.6 * s;
        runs(
            &mut frame,
            &mut self.fonts,
            x0,
            y + (h1 * 1.3 - line) / 2.0,
            h1,
            &[("Download OpenAgents", Weight::Bold, W100, false)],
        );
        y += h1 * 1.3 + 14.0 * s;
        // <h2>Coder</h2> and the release candidate.
        let h2 = 16.1 * s;
        runs(
            &mut frame,
            &mut self.fonts,
            x0,
            y + (h2 * 1.3 - line) / 2.0,
            h2,
            &[("Coder", Weight::Bold, W100, false)],
        );
        y += h2 * 1.3 + 3.5 * s;
        let lead = format!("Release candidate {}.", download.version);
        runs(
            &mut frame,
            &mut self.fonts,
            x0,
            y,
            base,
            &[(lead.as_str(), Weight::Regular, W50, false)],
        );
        y += line + 14.0 * s;
        for (hint, command) in [
            ("macOS and Linux:", download.sh.as_str()),
            ("Windows, in PowerShell:", download.ps1.as_str()),
        ] {
            runs(
                &mut frame,
                &mut self.fonts,
                x0,
                y,
                base,
                &[(hint, Weight::Regular, W75, false)],
            );
            y += line + 14.0 * s;
            let pre = PxRect {
                x: x0,
                y,
                w: column,
                h: 10.5 * s * 2.0 + line,
            };
            rule(&mut frame, pre);
            // A <pre> keeps its line and scrolls; here it is cut at the box.
            let clip = frame.as_deref_mut().map(|frame| {
                frame.clip_to(PxRect {
                    x: pre.x + s,
                    w: pre.w - 2.0 * s,
                    ..pre
                })
            });
            runs(
                &mut frame,
                &mut self.fonts,
                x0 + ch,
                y + 10.5 * s,
                base,
                &[(command, Weight::Regular, W100, false)],
            );
            if let (Some(frame), Some(clip)) = (frame.as_deref_mut(), clip) {
                frame.restore_clip(clip);
            }
            y += pre.h + 14.0 * s;
        }
        // How to open what the installer put on the computer.
        runs(
            &mut frame,
            &mut self.fonts,
            x0,
            y,
            base,
            &[
                ("Run ", Weight::Regular, W75, false),
                ("coder", Weight::Regular, W100, false),
                (" to open the new terminal.", Weight::Regular, W75, false),
            ],
        );
        y += line + 28.0 * s;
        // The closed disclosure with the release files.
        runs(
            &mut frame,
            &mut self.fonts,
            x0,
            y,
            base,
            &[("Download Coder manually", Weight::Regular, W100, true)],
        );
        y += line + 28.0 * s;
        y - area.y
    }
}

/// The card's own color rising over the bottom `height` pixels above
/// `bottom`, so the text there fades out.
fn fade(frame: &mut Frame, x: f32, w: f32, bottom: f32, height: f32, color: Color) {
    let rows = height.max(0.0).ceil() as usize;
    for row in 0..rows {
        let t = (row as f32 + 0.5) / height;
        frame.fill(
            PxRect {
                x,
                y: bottom - height + row as f32,
                w,
                h: 1.0,
            },
            0.0,
            alpha(color, t * t * (3.0 - 2.0 * t)),
        );
    }
    frame.fill(
        PxRect {
            x,
            y: bottom,
            w,
            h: 8.0,
        },
        0.0,
        color,
    );
}

/// An SVG's shape in `color`, rasterized to `rect`.
fn svg(frame: &mut Frame, rect: PxRect, source: &str, color: Color) {
    let (width, height) = (rect.w.round() as u32, rect.h.round() as u32);
    if !(1..=512).contains(&width) || !(1..=512).contains(&height) {
        return;
    }
    let Ok(tree) = resvg::usvg::Tree::from_str(source, &resvg::usvg::Options::default()) else {
        return;
    };
    let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(width, height) else {
        return;
    };
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(
            width as f32 / tree.size().width(),
            height as f32 / tree.size().height(),
        ),
        &mut pixmap.as_mut(),
    );
    for (index, pixel) in pixmap.data().chunks_exact(4).enumerate() {
        if pixel[3] != 0 {
            frame.blend(
                rect.x.round() as i64 + (index % width as usize) as i64,
                rect.y.round() as i64 + (index / width as usize) as i64,
                color,
                f32::from(pixel[3]) / 255.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cards read the essays from the repository: each has its title,
    /// its date, and an opening without its title or later sections.
    #[test]
    fn the_essays_come_from_the_repository() {
        let essays = essays();
        assert_eq!(essays[0].title, "The Return of the General Agent");
        assert_eq!(essays[0].date, "October 1, 2026");
        assert_eq!(essays[1].title, "Test-Time Capabilities");
        assert_eq!(essays[1].date, "September 29, 2026");
        for essay in &essays {
            assert!(essay.opening.starts_with("Essay, "), "{}", essay.opening);
            assert!(!essay.opening.contains(&format!("# {}", essay.title)));
            assert_eq!(essay.opening.matches("\n## ").count(), 1);
            assert!(essay.url().starts_with(BLOB) && essay.url().ends_with(".md"));
        }
        assert_eq!(
            url(ESSAYS, 1).as_deref(),
            Some(
                "https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-09-29-test-time-capabilities.md"
            )
        );
        assert_eq!(url(DOWNLOAD, 0).as_deref(), Some(DOWNLOAD_URL));
        assert_eq!(url(DOWNLOAD, 1), None);
    }

    /// The download card shows the page's own Coder release and install
    /// commands, and every fixed phrase it draws is still in the page's
    /// source (#11030).
    #[test]
    fn the_download_card_matches_the_page() {
        let download = download();
        assert!(
            download.version.starts_with("1.0.0"),
            "{}",
            download.version
        );
        assert!(
            download.sh.starts_with("curl -fsSL https://") && download.sh.ends_with("| bash"),
            "{}",
            download.sh
        );
        assert!(
            download.ps1.starts_with("irm https://") && download.ps1.ends_with("| iex"),
            "{}",
            download.ps1
        );
        for phrase in [
            "\"Download OpenAgents\"",
            "\"Coder\"",
            "Release candidate ",
            "macOS and Linux:",
            "Windows, in PowerShell:",
            "\"Run \" code { \"coder\" } \" to open the new terminal.",
            "Download Coder manually",
        ] {
            assert!(DOWNLOAD_SOURCE.contains(phrase), "{phrase}");
        }
    }

    /// Two cards side by side fill the essays slide; one window fills the
    /// download slide.
    #[test]
    fn the_cards_fill_the_slide() {
        let slide = PxRect {
            x: 10.0,
            y: 20.0,
            w: 1280.0,
            h: 720.0,
        };
        let two = cards(ESSAYS, slide);
        assert_eq!(two.len(), 2);
        assert!(two[0].x + two[0].w < two[1].x);
        assert!(two[0].w + two[1].w > 0.8 * slide.w);
        let one = cards(DOWNLOAD, slide);
        assert_eq!(one.len(), 1);
        assert!(one[0].w > 0.85 * slide.w);
        assert!(cards("grid", slide).is_empty());
    }

    /// The cards come in one after another, and under Reduce motion they
    /// show at once.
    #[test]
    fn the_cards_come_in_and_reduce_motion_shows_them() {
        let start = Instant::now();
        let mut embeds = Embeds::new(false);
        embeds.show(ESSAYS, start);
        assert!(embeds.entering());
        assert_eq!(embeds.reveal(0), 0.0);
        embeds.show(ESSAYS, start + std::time::Duration::from_millis(300));
        assert!(embeds.reveal(0) > embeds.reveal(1));
        embeds.show(ESSAYS, start + std::time::Duration::from_secs(2));
        assert!(!embeds.entering());
        assert_eq!((embeds.reveal(0), embeds.reveal(1)), (1.0, 1.0));
        let mut still = Embeds::new(true);
        still.show(DOWNLOAD, start);
        assert!(!still.entering());
        assert_eq!(still.reveal(0), 1.0);
    }
}
