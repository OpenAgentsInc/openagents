//! Link cards (#11126): under a reply, a card for each web link the reply
//! itself contains, with the page's title, its site, and its preview
//! picture (`og:image`) when the page names one.
//!
//! This module is the pure half: which links a reply carries, the cards'
//! shared state, reading a page's preview tags, and making a fetched
//! picture safe to show. The app that owns the network
//! (`openagents-mobile`) fetches each page the cards ask for, within its
//! own size and time limits, and reports back with [`LinkPreviews::finish`].
//! A card whose page has no picture, or whose fetch fails, is a plain link
//! card: its title (or its site) and its site.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Cursor;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;

/// The most cards under one reply.
pub const MAX_PER_REPLY: usize = 3;
/// The longest link that gets a card.
const MAX_URL: usize = 2048;
/// The most previews kept; the oldest go first.
const MAX_KEPT: usize = 128;
/// The longest title or site name shown.
const MAX_TITLE: usize = 120;
/// The most page bytes the preview tags are read from.
pub const MAX_PAGE_BYTES: usize = 512 * 1024;
/// The most picture bytes fetched.
pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
/// The largest picture decoded, a side.
const MAX_DIMENSION: u32 = 4096;
/// The widest picture a card keeps, in pixels.
const CARD_WIDTH: u32 = 720;
/// A card's height with a picture, and without one, in points.
pub const IMAGE_CARD_HEIGHT: u16 = 228;
pub const PLAIN_CARD_HEIGHT: u16 = 72;

/// The `https` links in a reply's text, in order, each once, at most
/// [`MAX_PER_REPLY`]. Links inside code are left out.
#[must_use]
pub fn answer_links(text: &str) -> Vec<String> {
    let mut found: Vec<String> = vec![];
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        // Inline code spans: text between backticks on this line.
        let mut code = false;
        let mut plain = String::with_capacity(line.len());
        for ch in line.chars() {
            if ch == '`' {
                code = !code;
                plain.push(' ');
            } else if code {
                plain.push(' ');
            } else {
                plain.push(ch);
            }
        }
        let mut rest = plain.as_str();
        while let Some(at) = rest.find("https://") {
            let tail = &rest[at..];
            let link = take_link(tail);
            rest = &tail[link.len().max(8)..];
            if link.len() <= "https://".len() || link.len() > MAX_URL {
                continue;
            }
            if !found.iter().any(|seen| seen == link) {
                found.push(link.to_owned());
                if found.len() == MAX_PER_REPLY {
                    return found;
                }
            }
        }
    }
    found
}

/// The link at the start of `text`: up to whitespace or a closing mark,
/// keeping balanced parentheses, less trailing punctuation.
fn take_link(text: &str) -> &str {
    let mut depth = 0usize;
    let mut end = 0;
    for (at, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => break,
            ')' => depth -= 1,
            c if c.is_whitespace()
                || matches!(
                    c,
                    '<' | '>' | '"' | '\'' | ']' | '[' | '{' | '}' | '|' | '\\' | '^' | '`'
                ) =>
            {
                break;
            }
            c if c.is_control() => break,
            _ => {}
        }
        end = at + ch.len_utf8();
    }
    let mut link = &text[..end];
    while let Some(last) = link.chars().last() {
        if matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | '*' | '_' | '~') {
            link = &link[..link.len() - 1];
        } else {
            break;
        }
    }
    link
}

/// The surface resource for a link's card: `link:` and a hash of the link
/// (a resource is a short identifier, never the link itself).
#[must_use]
pub fn resource(url: &str) -> String {
    // FNV-1a, 64 bits.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("link:{hash:016x}")
}

/// The site a link names: its host, less `www.`.
#[must_use]
pub fn site(url: &str) -> String {
    let rest = url.strip_prefix("https://").unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    host.strip_prefix("www.").unwrap_or(host).to_lowercase()
}

/// What a fetched page offered for its card.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preview {
    pub title: Option<String>,
    pub site: Option<String>,
    /// The card's picture, re-encoded as a JPEG at most [`CARD_WIDTH`]
    /// pixels wide ([`card_image`]).
    pub image: Option<Arc<Vec<u8>>>,
}

#[derive(Clone, Debug)]
enum Entry {
    Pending,
    Done(Preview),
}

/// A card as the host draws it, in the packet's `links`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Card {
    pub url: String,
    pub title: String,
    pub site: String,
    /// The card has a picture: the host reads it as the surface's image.
    pub image: bool,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<String, Entry>,
    order: VecDeque<String>,
    wanted: Vec<String>,
    /// Each card resource the last view showed, with its link.
    shown: BTreeMap<String, String>,
}

/// The cards' shared state: what each link's page offered, and the pages
/// still to fetch. Cloning shares it.
#[derive(Clone, Default)]
pub struct LinkPreviews(Arc<Mutex<Inner>>);

impl LinkPreviews {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// The card for `url` as it stands, asking for its page the first time.
    pub fn card(&self, url: &str) -> Card {
        let mut inner = self.lock();
        if !inner.entries.contains_key(url) {
            while inner.order.len() >= MAX_KEPT {
                if let Some(old) = inner.order.pop_front() {
                    inner.entries.remove(&old);
                }
            }
            inner.entries.insert(url.to_owned(), Entry::Pending);
            inner.order.push_back(url.to_owned());
            inner.wanted.push(url.to_owned());
        }
        inner.shown.insert(resource(url), url.to_owned());
        let site = site(url);
        match inner.entries.get(url) {
            Some(Entry::Done(preview)) => Card {
                url: url.to_owned(),
                title: preview
                    .title
                    .clone()
                    .unwrap_or_else(|| preview.site.clone().unwrap_or_else(|| site.clone())),
                site: preview.site.clone().unwrap_or(site),
                image: preview.image.is_some(),
            },
            _ => Card {
                url: url.to_owned(),
                title: site.clone(),
                site,
                image: false,
            },
        }
    }

    /// The pages to fetch now; each is handed out once.
    pub fn take_wanted(&self) -> Vec<String> {
        std::mem::take(&mut self.lock().wanted)
    }

    /// A fetch ended: what the page offered, or nothing (a plain card).
    pub fn finish(&self, url: &str, preview: Option<Preview>) {
        let mut inner = self.lock();
        if inner.entries.contains_key(url) {
            inner
                .entries
                .insert(url.to_owned(), Entry::Done(preview.unwrap_or_default()));
        }
    }

    /// Some shown card's page is still being read.
    #[must_use]
    pub fn pending(&self) -> bool {
        let inner = self.lock();
        inner
            .shown
            .values()
            .any(|url| matches!(inner.entries.get(url), Some(Entry::Pending)))
    }

    /// The picture a `link:` surface shows, when its page offered one.
    #[must_use]
    pub fn image(&self, resource: &str) -> Option<Arc<Vec<u8>>> {
        let inner = self.lock();
        let url = inner.shown.get(resource)?;
        match inner.entries.get(url) {
            Some(Entry::Done(preview)) => preview.image.clone(),
            _ => None,
        }
    }

    /// Forget which cards the view shows, before it is built again.
    pub fn begin_view(&self) {
        self.lock().shown.clear();
    }

    /// The cards the last view showed, by resource.
    #[must_use]
    pub fn shown(&self) -> BTreeMap<String, Card> {
        let shown: Vec<String> = self.lock().shown.values().cloned().collect();
        shown
            .into_iter()
            .map(|url| (resource(&url), self.card(&url)))
            .collect()
    }
}

/// A page's preview tags: `og:title` (or `<title>`), `og:site_name`, and
/// `og:image` (or `twitter:image`), as written; the image may be relative.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageMeta {
    pub title: Option<String>,
    pub site: Option<String>,
    pub image: Option<String>,
}

/// Read the preview tags from the start of a page, up to its `</head>`.
#[must_use]
pub fn page_meta(html: &str) -> PageMeta {
    let head_end = find_ci(html, "</head").unwrap_or(html.len());
    let head = &html[..head_end];
    let mut meta = PageMeta::default();
    let mut twitter_image = None;
    let mut rest = head;
    while let Some(at) = find_ci(rest, "<meta") {
        let tag = &rest[at + 5..];
        let close = tag.find('>').unwrap_or(tag.len());
        let attrs = attributes(&tag[..close]);
        rest = &tag[close..];
        let key = attrs
            .iter()
            .find(|(name, _)| name == "property" || name == "name")
            .map(|(_, value)| value.to_lowercase());
        let Some(content) = attrs
            .iter()
            .find(|(name, _)| name == "content")
            .map(|(_, value)| value.trim().to_owned())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        match key.as_deref() {
            Some("og:title") if meta.title.is_none() => meta.title = Some(clean(&content)),
            Some("og:site_name") if meta.site.is_none() => meta.site = Some(clean(&content)),
            Some("og:image" | "og:image:url" | "og:image:secure_url") if meta.image.is_none() => {
                meta.image = Some(content);
            }
            Some("twitter:image" | "twitter:image:src") if twitter_image.is_none() => {
                twitter_image = Some(content);
            }
            _ => {}
        }
    }
    if meta.image.is_none() {
        meta.image = twitter_image;
    }
    if meta.title.is_none()
        && let Some(open) = find_ci(head, "<title")
    {
        let after = &head[open..];
        if let Some(start) = after.find('>') {
            let body = &after[start + 1..];
            let end = find_ci(body, "</title").unwrap_or(body.len().min(512));
            let title = clean(&decode(&body[..end]));
            if !title.is_empty() {
                meta.title = Some(title);
            }
        }
    }
    meta.title = meta.title.filter(|title| !title.is_empty());
    meta.site = meta.site.filter(|site| !site.is_empty());
    meta
}

/// One line of at most [`MAX_TITLE`] characters, without control
/// characters.
fn clean(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let line: String = words
        .join(" ")
        .chars()
        .filter(|ch| !ch.is_control())
        .collect();
    if line.chars().count() > MAX_TITLE {
        let mut short: String = line.chars().take(MAX_TITLE - 1).collect();
        short.push('…');
        short
    } else {
        line
    }
}

/// Where `needle` (ASCII, lowercase) first appears in `text`, ignoring case.
fn find_ci(text: &str, needle: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > bytes.len() {
        return None;
    }
    (0..=bytes.len() - needle.len()).find(|&at| {
        bytes[at..at + needle.len()]
            .iter()
            .zip(needle)
            .all(|(a, b)| a.to_ascii_lowercase() == *b)
    })
}

/// A tag's attributes, names lowercased, values decoded.
fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut found = vec![];
    let mut chars = tag.char_indices().peekable();
    while let Some(&(start, ch)) = chars.peek() {
        if ch.is_whitespace() || ch == '/' {
            chars.next();
            continue;
        }
        let mut end = start;
        while let Some(&(at, ch)) = chars.peek() {
            if ch.is_whitespace() || ch == '=' || ch == '/' {
                break;
            }
            end = at + ch.len_utf8();
            chars.next();
        }
        let name = tag[start..end].to_lowercase();
        while chars.peek().is_some_and(|&(_, ch)| ch.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_some_and(|&(_, ch)| ch == '=') {
            chars.next();
            while chars.peek().is_some_and(|&(_, ch)| ch.is_whitespace()) {
                chars.next();
            }
            let value = match chars.peek() {
                Some(&(at, quote @ ('"' | '\''))) => {
                    chars.next();
                    let begin = at + 1;
                    let mut stop = tag.len();
                    for (at, ch) in chars.by_ref() {
                        if ch == quote {
                            stop = at;
                            break;
                        }
                    }
                    &tag[begin..stop.max(begin)]
                }
                Some(&(begin, _)) => {
                    let mut stop = tag.len();
                    while let Some(&(at, ch)) = chars.peek() {
                        if ch.is_whitespace() {
                            stop = at;
                            break;
                        }
                        chars.next();
                    }
                    &tag[begin..stop]
                }
                None => "",
            };
            found.push((name, decode(value)));
        } else if !name.is_empty() {
            found.push((name, String::new()));
        }
    }
    found
}

/// The common HTML character references.
fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some(semi) = tail[..tail.len().min(12)].find(';') else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let name = &tail[1..semi];
        let ch = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match ch {
            Some(ch) => {
                out.push(ch);
                rest = &tail[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A fetched picture made safe to show: a PNG or JPEG of at most
/// [`MAX_IMAGE_BYTES`] and 4096 pixels a side, decoded here and re-encoded
/// as a JPEG at most [`CARD_WIDTH`] pixels wide, so the phone never decodes
/// the page's own bytes. `None` for anything else.
#[must_use]
pub fn card_image(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return None;
    }
    let format = image::guess_format(bytes).ok()?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return None;
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    if decoded.width() < 32 || decoded.height() < 32 {
        return None;
    }
    let decoded = if decoded.width() > CARD_WIDTH {
        decoded.resize(
            CARD_WIDTH,
            MAX_DIMENSION,
            image::imageops::FilterType::Triangle,
        )
    } else {
        decoded
    };
    // Transparent parts show on white, as a browser shows them.
    let rgba = decoded.into_rgba8();
    let rgb = image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let over = |c: u8| ((u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8;
        image::Rgb([over(r), over(g), over(b)])
    });
    let mut out = Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80)
        .encode_image(&rgb)
        .ok()?;
    Some(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_names_its_links_once_and_not_in_code() {
        let text = "See [the roadmap](https://openagents.com/roadmap). Also \
                    https://github.com/OpenAgentsInc/openagents, and again \
                    https://openagents.com/roadmap.\n\
                    `https://example.com/inline`\n```\nhttps://example.com/fenced\n```\n\
                    http://insecure.example.com and https://en.wikipedia.org/wiki/Rust_(language)!\n\
                    https://fourth.example.com";
        assert_eq!(
            answer_links(text),
            [
                "https://openagents.com/roadmap",
                "https://github.com/OpenAgentsInc/openagents",
                "https://en.wikipedia.org/wiki/Rust_(language)",
            ]
        );
        assert!(answer_links("no links here").is_empty());
        assert!(answer_links("https://").is_empty());
    }

    #[test]
    fn resources_are_short_identifiers() {
        let id = resource("https://openagents.com/roadmap");
        assert!(id.starts_with("link:") && id.len() == 21, "{id}");
        assert_ne!(id, resource("https://openagents.com/"));
        assert_eq!(site("https://www.GitHub.com:443/a?b#c"), "github.com");
    }

    #[test]
    fn page_meta_reads_open_graph_tags() {
        let html = r#"<!doctype html><html><head>
            <title>Fallback &amp; title</title>
            <meta property="og:title" content="The &quot;Roadmap&quot;">
            <meta content='/images/card.png' property='og:image' />
            <meta name=og:site_name content=OpenAgents>
            <meta name="twitter:image" content="https://example.com/t.png">
            </head><body><meta property="og:image" content="https://late.example.com/x.png"></body>"#;
        let meta = page_meta(html);
        assert_eq!(meta.title.as_deref(), Some("The \"Roadmap\""));
        assert_eq!(meta.image.as_deref(), Some("/images/card.png"));
        assert_eq!(meta.site.as_deref(), Some("OpenAgents"));

        let bare = page_meta("<HEAD><TITLE>\n  Just   a title </TITLE></HEAD>");
        assert_eq!(bare.title.as_deref(), Some("Just a title"));
        assert_eq!(bare.image, None);
        assert_eq!(page_meta("").title, None);
        let twitter = page_meta(r#"<meta name="twitter:image" content="https://t.example/p.jpg">"#);
        assert_eq!(twitter.image.as_deref(), Some("https://t.example/p.jpg"));
    }

    #[test]
    fn cards_ask_for_their_page_once_and_fall_back_to_plain() {
        let previews = LinkPreviews::default();
        let url = "https://openagents.com/roadmap";
        let card = previews.card(url);
        assert_eq!((card.title.as_str(), card.image), ("openagents.com", false));
        assert!(previews.pending());
        assert_eq!(previews.take_wanted(), [url]);
        previews.card(url);
        assert!(previews.take_wanted().is_empty());
        previews.finish(url, None);
        assert!(!previews.pending());
        assert_eq!(previews.card(url).title, "openagents.com");

        let other = "https://example.com/a";
        previews.card(other);
        previews.finish(
            other,
            Some(Preview {
                title: Some("A page".into()),
                site: None,
                image: Some(Arc::new(vec![1, 2, 3])),
            }),
        );
        let card = previews.card(other);
        assert_eq!(
            (card.title.as_str(), card.site.as_str(), card.image),
            ("A page", "example.com", true)
        );
        assert_eq!(previews.image(&resource(other)).map(|b| b.len()), Some(3));
        assert_eq!(previews.shown().len(), 2);
        previews.begin_view();
        assert!(previews.image(&resource(other)).is_none());
    }

    #[test]
    fn card_images_are_reencoded_and_bounded() {
        let mut png = Cursor::new(Vec::new());
        image::RgbImage::from_pixel(1200, 630, image::Rgb([20, 120, 220]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let jpeg = card_image(png.get_ref()).expect("a card picture");
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert_eq!(decoded.width(), CARD_WIDTH);
        assert_eq!(
            image::guess_format(&jpeg).unwrap(),
            image::ImageFormat::Jpeg
        );
        assert!(card_image(b"<html>not a picture</html>").is_none());
        let mut tiny = Cursor::new(Vec::new());
        image::RgbImage::new(8, 8)
            .write_to(&mut tiny, image::ImageFormat::Png)
            .unwrap();
        assert!(card_image(tiny.get_ref()).is_none());
    }
}
