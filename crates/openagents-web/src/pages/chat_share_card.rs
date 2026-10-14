//! A shared chat's preview picture (`og:image`), 1200×630 PNG, in the
//! launch film's look: the noir canvas, the faint perspective Grid floor
//! receding to a horizon with a soft glow, and on top the sharer (picture
//! or initials, name, the start and end of their npub, the date), the
//! OpenAgents mark, the chat's title, and the first question and the start
//! of the answer.
//!
//! Drawn from an SVG built here and rendered with `resvg`, in Geist (OFL,
//! `static/fonts/geist`), with lines measured from the font so they wrap
//! and end in "…" instead of running off. Only the chat's own words are
//! drawn: never a file sent with it, never a project's files.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, OnceLock};

use resvg::{tiny_skia, usvg};
use sha2::{Digest, Sha256};

pub(crate) const WIDTH: u32 = 1200;
pub(crate) const HEIGHT: u32 = 630;
/// Changes when the drawing changes, so every card's address does too.
const VERSION: &str = "card.v1";

const REGULAR: &[u8] = include_bytes!("../../static/fonts/geist/Geist-Regular.ttf");
const MEDIUM: &[u8] = include_bytes!("../../static/fonts/geist/Geist-Medium.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../../static/fonts/geist/Geist-SemiBold.ttf");
const MONO: &[u8] = include_bytes!("../../static/fonts/geist/GeistMono-Regular.ttf");

/// What a card shows.
#[derive(Clone, Debug, Default)]
pub(crate) struct Card {
    pub title: String,
    /// The first question, plain.
    pub ask: String,
    /// The start of the first answer, plain.
    pub reply: String,
    /// The sharer's name, when it isn't an email address.
    pub name: Option<String>,
    pub npub: Option<String>,
    /// The sharer's GitHub picture's address (it goes in the digest).
    pub avatar_url: Option<String>,
    /// "Oct 13, 2026".
    pub date: String,
}

impl Card {
    /// A digest of everything drawn, so a card's address changes with it.
    pub(crate) fn digest(&self) -> String {
        let mut hash = Sha256::new();
        for part in [
            VERSION,
            &self.title,
            &self.ask,
            &self.reply,
            self.name.as_deref().unwrap_or(""),
            self.npub.as_deref().unwrap_or(""),
            self.avatar_url.as_deref().unwrap_or(""),
            &self.date,
        ] {
            hash.update(part.as_bytes());
            hash.update([0]);
        }
        hash.finalize()[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// Plain words from a reply's Markdown: paragraphs, headings and list
/// items, without code blocks or drawn components.
pub(crate) fn plain(markdown: &str) -> String {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut out = String::new();
    let mut code = 0usize;
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code += 1,
            Event::End(TagEnd::CodeBlock) => code = code.saturating_sub(1),
            Event::Text(text) | Event::Code(text) if code == 0 => out.push_str(&text),
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            Event::End(TagEnd::Heading(_)) => {
                // A heading reads as its own sentence.
                if !out.trim_end().ends_with(|c: char| c.is_ascii_punctuation()) {
                    out.push('.');
                }
                out.push(' ');
            }
            Event::End(TagEnd::Paragraph | TagEnd::Item) => out.push(' '),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Oct 13, 2026" for a Unix time.
pub(crate) fn date(unix: u64) -> String {
    // Days to a civil date (Howard Hinnant's algorithm).
    let days = (unix / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{} {day}, {year}", MONTHS[(month - 1) as usize])
}

/// "npub1abcd…wxyz".
pub(crate) fn short_npub(npub: &str) -> String {
    if npub.len() <= 18 {
        return npub.to_owned();
    }
    format!("{}…{}", &npub[..9], &npub[npub.len() - 4..])
}

fn initials(name: Option<&str>) -> String {
    let letters: String = name
        .unwrap_or("")
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_' || c == '.')
        .filter_map(|word| word.chars().find(|c| c.is_alphanumeric()))
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    if letters.is_empty() {
        "OA".to_owned()
    } else {
        letters
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Text width in pixels at `size`, from the font's advances.
fn width(font: &[u8], size: f32, text: &str) -> f32 {
    let Ok(face) = ttf_parser::Face::parse(font, 0) else {
        return text.chars().count() as f32 * size * 0.55;
    };
    let scale = size / f32::from(face.units_per_em());
    text.chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|glyph| face.glyph_hor_advance(glyph))
                .map_or(size * 0.55, |advance| f32::from(advance) * scale)
        })
        .sum()
}

/// `text` in at most `lines` lines of at most `max` pixels; the last line
/// ends in "…" when the text goes on.
fn wrap(font: &[u8], size: f32, text: &str, max: f32, lines: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        let next = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if width(font, size, &next) <= max {
            line = next;
            index += 1;
            continue;
        }
        if line.is_empty() {
            // One word wider than the line: cut it.
            let mut cut = String::new();
            for c in word.chars() {
                if width(font, size, &format!("{cut}{c}")) > max {
                    break;
                }
                cut.push(c);
            }
            line = cut;
            index += 1;
        }
        out.push(std::mem::take(&mut line));
        if out.len() == lines {
            break;
        }
    }
    if !line.is_empty() && out.len() < lines {
        out.push(line);
    }
    if index < words.len()
        && let Some(last) = out.last_mut()
    {
        let whole = last.len();
        while !last.is_empty() && width(font, size, &format!("{last}…")) > max {
            last.pop();
        }
        // Cut back to a whole word when the ellipsis took part of one.
        if last.len() < whole
            && let Some(at) = last.rfind(' ')
            && at > last.len() / 2
        {
            last.truncate(at);
        }
        *last = format!(
            "{}…",
            last.trim_end_matches(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        );
    }
    out
}

/// The Grid floor: rows and rails in perspective to a horizon at `horizon`,
/// fading out toward it.
fn grid(horizon: f32) -> String {
    let vx = WIDTH as f32 / 2.0 - 10.0;
    let depth = HEIGHT as f32 - horizon;
    let mut path = String::new();
    // Rows: equal steps on the floor, closer together toward the horizon.
    let mut z = 0.62_f32;
    while z < 24.0 {
        let y = horizon + depth / z;
        if y < HEIGHT as f32 + 1.0 {
            let _ = write!(path, "M0 {y:.2}H{WIDTH}");
        }
        z += 0.5;
    }
    // Rails: lines from below the frame to the vanishing point.
    let near = 0.45_f32;
    for i in -40..=40 {
        let x = vx + depth * (i as f32 * 1.9) / near;
        let y = horizon + depth / near;
        let _ = write!(path, "M{x:.2} {y:.2}L{vx:.2} {horizon:.2}");
    }
    format!(
        r##"<defs>
  <linearGradient id="fade" x1="0" y1="{horizon}" x2="0" y2="{HEIGHT}" gradientUnits="userSpaceOnUse">
    <stop offset="0" stop-color="#fff" stop-opacity="0"/>
    <stop offset="0.12" stop-color="#fff" stop-opacity="0.10"/>
    <stop offset="1" stop-color="#fff" stop-opacity="0.42"/>
  </linearGradient>
  <mask id="floor" maskUnits="userSpaceOnUse" x="0" y="0" width="{WIDTH}" height="{HEIGHT}">
    <rect x="0" y="{horizon}" width="{WIDTH}" height="{depth}" fill="url(#fade)"/>
  </mask>
  <radialGradient id="glow" cx="{vx}" cy="{horizon}" r="560" gradientUnits="userSpaceOnUse">
    <stop offset="0" stop-color="#fff" stop-opacity="0.05"/>
    <stop offset="0.5" stop-color="#fff" stop-opacity="0.015"/>
    <stop offset="1" stop-color="#fff" stop-opacity="0"/>
  </radialGradient>
  <radialGradient id="haze" cx="0.5" cy="0.5" r="0.5">
    <stop offset="0" stop-color="#fff" stop-opacity="0.12"/>
    <stop offset="1" stop-color="#fff" stop-opacity="0"/>
  </radialGradient>
</defs>
<rect width="{WIDTH}" height="{HEIGHT}" fill="url(#glow)"/>
<ellipse cx="{vx}" cy="{horizon}" rx="320" ry="26" fill="url(#haze)"/>
<path d="{path}" stroke="#fff" stroke-width="1.1" fill="none" mask="url(#floor)"/>"##
    )
}

/// Where the sharer's picture sits: centre and radius.
const AVATAR: (f32, f32, f32) = (98.0, 84.0, 26.0);

/// The card as SVG. `avatar` is the sharer's picture as PNG, if any.
pub(crate) fn svg(card: &Card, avatar: Option<&[u8]>) -> String {
    const LEFT: f32 = 72.0;
    let max = WIDTH as f32 - 2.0 * LEFT;
    let horizon = 468.0;
    let mut out = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">
<rect width="{WIDTH}" height="{HEIGHT}" fill="#0a0a0a"/>
{}
"##,
        grid(horizon)
    );
    // The sharer: picture (or initials), name, npub and date.
    let (cx, cy, r) = AVATAR;
    match avatar {
        // The picture itself is drawn over the rendered card ([`render`]).
        Some(_) => {
            let _ = write!(
                out,
                r##"<circle cx="{cx}" cy="{cy}" r="{}" fill="none" stroke="#fff" stroke-opacity="0.18"/>
"##,
                r + 0.5
            );
        }
        None => {
            let _ = write!(
                out,
                r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="#1f1f1f" stroke="#fff" stroke-opacity="0.18"/>
<text x="{cx}" y="{}" text-anchor="middle" font-family="Geist" font-weight="500" font-size="20" fill="#d4d4d4">{}</text>
"##,
                cy + 7.0,
                escape(&initials(card.name.as_deref()))
            );
        }
    }
    let text_x = cx + r + 18.0;
    let mut detail = Vec::new();
    if let Some(npub) = &card.npub {
        detail.push(short_npub(npub));
    }
    detail.push(card.date.clone());
    let detail = escape(&detail.join("  ·  "));
    match &card.name {
        Some(name) => {
            let name = wrap(MEDIUM, 24.0, name, 560.0, 1).join("");
            let _ = write!(
                out,
                r##"<text x="{text_x}" y="78" font-family="Geist" font-weight="500" font-size="24" fill="#ededed">{}</text>
<text x="{text_x}" y="106" font-family="Geist Mono" font-size="17" fill="#8a8a8a">{detail}</text>
"##,
                escape(&name)
            );
        }
        // No name to show: the npub and date alone, beside the picture.
        None => {
            let _ = write!(
                out,
                r##"<text x="{text_x}" y="{}" font-family="Geist Mono" font-size="18" fill="#a3a3a3">{detail}</text>
"##,
                cy + 6.0
            );
        }
    }
    // The mark, top right.
    let mark = "OpenAgents";
    let mark_x = WIDTH as f32 - LEFT - width(SEMIBOLD, 26.0, mark);
    let _ = write!(
        out,
        r##"<text x="{mark_x:.1}" y="93" font-family="Geist" font-weight="600" font-size="26" letter-spacing="-0.3" fill="#ededed">{mark}</text>
"##
    );
    // The title, up to two lines.
    let title = wrap(SEMIBOLD, 58.0, &card.title, max, 2);
    let mut y = 216.0;
    for line in &title {
        let _ = write!(
            out,
            r##"<text x="{LEFT}" y="{y}" font-family="Geist" font-weight="600" font-size="58" letter-spacing="-1.2" fill="#fafafa">{}</text>
"##,
            escape(line)
        );
        y += 68.0;
    }
    // The question, then the start of the answer.
    y += 6.0;
    if !card.ask.is_empty() {
        for line in wrap(REGULAR, 25.0, &format!("“{}”", card.ask), max, 1) {
            let _ = write!(
                out,
                r##"<text x="{LEFT}" y="{y}" font-family="Geist" font-size="25" fill="#8f8f8f">{}</text>
"##,
                escape(&line)
            );
            y += 38.0;
        }
    }
    let room = if title.len() > 1 { 2 } else { 3 };
    for line in wrap(REGULAR, 25.0, &card.reply, max, room) {
        let _ = write!(
            out,
            r##"<text x="{LEFT}" y="{y}" font-family="Geist" font-size="25" fill="#cfcfcf">{}</text>
"##,
            escape(&line)
        );
        y += 36.0;
    }
    out.push_str("</svg>\n");
    out
}

/// A page's preview picture (`og:image`) for a page that has no picture of
/// its own ([`crate::link_preview`]): the same canvas and Grid floor, the
/// OpenAgents mark, an optional small `label` above, the page's `heading`,
/// and one or two lines of `line` under it.
pub(crate) fn page_svg(label: Option<&str>, heading: &str, line: &str) -> String {
    const LEFT: f32 = 72.0;
    let max = WIDTH as f32 - 2.0 * LEFT;
    let mut out = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">
<rect width="{WIDTH}" height="{HEIGHT}" fill="#0a0a0a"/>
{}
<text x="{LEFT}" y="100" font-family="Geist" font-weight="600" font-size="34" letter-spacing="-0.4" fill="#ededed">OpenAgents</text>
"##,
        grid(486.0)
    );
    let site = "openagents.com";
    let site_x = WIDTH as f32 - LEFT - width(MONO, 20.0, site);
    let _ = write!(
        out,
        r##"<text x="{site_x:.1}" y="98" font-family="Geist Mono" font-size="20" fill="#7a7a7a">{site}</text>
"##
    );
    let heading = wrap(SEMIBOLD, 72.0, heading, max, 2);
    let mut y = if heading.len() > 1 { 228.0 } else { 270.0 };
    if let Some(label) = label {
        let _ = write!(
            out,
            r##"<text x="{LEFT}" y="{}" font-family="Geist Mono" font-size="22" letter-spacing="1.5" fill="#8a8a8a">{}</text>
"##,
            y - 92.0,
            escape(&label.to_uppercase())
        );
    }
    for text in &heading {
        let _ = write!(
            out,
            r##"<text x="{LEFT}" y="{y}" font-family="Geist" font-weight="600" font-size="72" letter-spacing="-1.6" fill="#fafafa">{}</text>
"##,
            escape(text)
        );
        y += 82.0;
    }
    y += 4.0;
    for text in wrap(REGULAR, 30.0, line, max, 2) {
        let _ = write!(
            out,
            r##"<text x="{LEFT}" y="{y}" font-family="Geist" font-size="30" fill="#a3a3a3">{}</text>
"##,
            escape(&text)
        );
        y += 42.0;
    }
    out.push_str("</svg>\n");
    out
}

/// [`page_svg`] as PNG.
pub(crate) fn render_page(
    label: Option<&str>,
    heading: &str,
    line: &str,
) -> Result<Vec<u8>, String> {
    let options = usvg::Options {
        fontdb: fonts(),
        font_family: "Geist".into(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&page_svg(label, heading, line), &options)
        .map_err(|e| e.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH, HEIGHT).ok_or_else(|| "no pixmap".to_owned())?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| e.to_string())
}

fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            for font in [REGULAR, MEDIUM, SEMIBOLD, MONO] {
                db.load_font_data(font.to_vec());
            }
            db.set_sans_serif_family("Geist");
            db.set_monospace_family("Geist Mono");
            Arc::new(db)
        })
        .clone()
}

/// A picture as PNG at most 128 pixels square, from PNG, JPEG, GIF or
/// WebP bytes the `image` crate reads.
pub(crate) fn avatar_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let picture = image::load_from_memory(bytes).ok()?;
    let picture = picture.thumbnail(128, 128);
    let mut out = std::io::Cursor::new(Vec::new());
    picture.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// The card as PNG.
pub(crate) fn render(card: &Card, avatar: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let svg = svg(card, avatar);
    let options = usvg::Options {
        fontdb: fonts(),
        font_family: "Geist".into(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|e| e.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH, HEIGHT).ok_or_else(|| "no pixmap".to_owned())?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    if let Some(picture) = avatar.and_then(|png| tiny_skia::Pixmap::decode_png(png).ok()) {
        let (cx, cy, r) = AVATAR;
        let scale = 2.0 * r / picture.width().max(picture.height()) as f32;
        let shader = tiny_skia::Pattern::new(
            picture.as_ref(),
            tiny_skia::SpreadMode::Pad,
            tiny_skia::FilterQuality::Bicubic,
            1.0,
            tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, cx - r, cy - r),
        );
        let paint = tiny_skia::Paint {
            shader,
            anti_alias: true,
            ..tiny_skia::Paint::default()
        };
        if let Some(circle) = tiny_skia::PathBuilder::from_circle(cx, cy, r) {
            pixmap.fill_path(
                &circle,
                &paint,
                tiny_skia::FillRule::Winding,
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    pixmap.encode_png().map_err(|e| e.to_string())
}

/// Cards already drawn, by share token and digest.
fn drawn() -> &'static Mutex<HashMap<String, Arc<Vec<u8>>>> {
    static DRAWN: OnceLock<Mutex<HashMap<String, Arc<Vec<u8>>>>> = OnceLock::new();
    DRAWN.get_or_init(Default::default)
}

/// The card drawn once per token and digest, then kept.
pub(crate) fn cached(key: &str, draw: impl FnOnce() -> Option<Vec<u8>>) -> Option<Arc<Vec<u8>>> {
    if let Some(found) = drawn().lock().ok().and_then(|d| d.get(key).cloned()) {
        return Some(found);
    }
    let png = Arc::new(draw()?);
    if let Ok(mut kept) = drawn().lock() {
        if kept.len() >= 64 {
            kept.clear();
        }
        kept.insert(key.to_owned(), png.clone());
    }
    Some(png)
}

/// Forget a token's cards (its share stopped).
pub(crate) fn forget(token: &str) {
    if let Ok(mut kept) = drawn().lock() {
        kept.retain(|key, _| !key.starts_with(token));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_npubs_and_plain_words() {
        assert_eq!(date(0), "Jan 1, 1970");
        assert_eq!(date(1_791_849_600), "Oct 13, 2026");
        assert_eq!(
            short_npub("npub1abcdefghijklmnopqrstuvwxyz0123456789wxyz"),
            "npub1abcd…wxyz"
        );
        assert_eq!(
            plain("## Plan\n\nShip **Friday**.\n\n```sh\nrm -rf /\n```\n\n- one\n- two"),
            "Plan. Ship Friday. one two"
        );
        assert_eq!(initials(Some("Octo Local")), "OL");
        assert_eq!(initials(None), "OA");
    }

    #[test]
    fn long_text_wraps_and_ends_in_an_ellipsis() {
        let lines = wrap(REGULAR, 25.0, &"word ".repeat(200), 400.0, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'));
        assert!(lines.iter().all(|l| width(REGULAR, 25.0, l) <= 400.0));
        assert_eq!(wrap(REGULAR, 25.0, "short", 400.0, 2), vec!["short"]);
    }

    #[test]
    fn a_card_renders_as_a_1200_by_630_png() {
        let card = Card {
            title: "Plan the launch email <& “quotes”>".into(),
            ask: "Draft the launch email for Friday.".into(),
            reply: "Here is a draft. Subject: Coder is here.".into(),
            name: Some("Octo Local".into()),
            npub: Some("npub1abcdefghijklmnopqrstuvwxyz0123456789wxyz".into()),
            avatar_url: None,
            date: "Oct 13, 2026".into(),
        };
        let png = render(&card, None).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let picture = image::load_from_memory(&png).unwrap();
        assert_eq!((picture.width(), picture.height()), (WIDTH, HEIGHT));
        // A picture is drawn in its circle.
        let red = image::RgbaImage::from_pixel(40, 40, image::Rgba([220, 30, 30, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(red)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let avatar = avatar_png(&bytes.into_inner()).unwrap();
        let with = image::load_from_memory(&render(&card, Some(&avatar)).unwrap())
            .unwrap()
            .to_rgba8();
        let (cx, cy, _) = AVATAR;
        let centre = with.get_pixel(cx as u32, cy as u32);
        assert!(centre[0] > 180 && centre[1] < 80, "{centre:?}");
        assert_ne!(card.digest(), Card::default().digest());
        if let Ok(dir) = std::env::var("OA_CARD_OUT") {
            std::fs::write(format!("{dir}/card.png"), &png).unwrap();
            let long = Card {
                title: "Why does my Rust build take so long after every small change in the workspace?".into(),
                ask: "Why does my Rust build take so long after every small change? It rebuilds half the workspace each time and I don't know why.".into(),
                reply: plain("Most of the time goes to **linking** and to crates that rebuild because a feature changed.\n\n## What to try\n\n1. Keep one target folder per checkout so nothing rebuilds from scratch.\n2. Split the big crate."),
                name: Some("Christopher David".into()),
                npub: None,
                avatar_url: Some("x".into()),
                date: "Oct 13, 2026".into(),
            };
            std::fs::write(
                format!("{dir}/card-long.png"),
                render(&long, Some(&avatar)).unwrap(),
            )
            .unwrap();
            let bare = Card {
                name: None,
                npub: None,
                avatar_url: None,
                ..long
            };
            std::fs::write(format!("{dir}/card-bare.png"), render(&bare, None).unwrap()).unwrap();
            std::fs::write(format!("{dir}/card.svg"), svg(&card, None)).unwrap();
        }
    }
}
