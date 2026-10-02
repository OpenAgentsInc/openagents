//! Paint-only syntax spans. Call the highlighter on an adapter's worker.
//!
//! Fonts, text, and layout never change when spans arrive. Unknown languages
//! and inputs beyond the bounds stay plain. No content leaves this process.
use serde::Serialize;

pub use code_highlight::{Kind, MAX_BYTES};
pub const MAX_SPANS: usize = 16 * 1024;

/// A UTF-8 byte range and its foreground color. It has no font attributes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub foreground: [u8; 4],
}

/// Retains compiled queries on a worker instead of loading them per block.
pub struct Highlighter {
    grammars: code_highlight::Grammars,
    palette: Palette,
}
/// Foregrounds only. A palette changes neither input bytes nor text metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    colors: [[u8; 4]; code_highlight::KINDS],
    text: [u8; 4],
}
impl Palette {
    pub const fn plain(text: [u8; 4]) -> Self {
        Self {
            colors: [text; code_highlight::KINDS],
            text,
        }
    }
    #[must_use]
    pub const fn with(mut self, kind: Kind, color: [u8; 4]) -> Self {
        self.colors[kind as usize] = color;
        self
    }
    pub const fn color(self, kind: Kind) -> [u8; 4] {
        self.colors[kind as usize]
    }
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            text: [230, 232, 235, 255],
            colors: [
                [123, 137, 151, 255],
                [163, 190, 140, 255],
                [163, 190, 140, 255],
                [163, 190, 140, 255],
                [180, 142, 173, 255],
                [143, 188, 187, 255],
                [143, 188, 187, 255],
                [143, 188, 187, 255],
                [235, 203, 139, 255],
                [235, 203, 139, 255],
                [230, 232, 235, 255],
                [208, 135, 112, 255],
                [230, 232, 235, 255],
                [208, 135, 112, 255],
                [129, 161, 193, 255],
                [136, 192, 208, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
                [230, 232, 235, 255],
            ],
        }
    }
}
impl Palette {
    /// Foregrounds for a light background: the same kinds as the default
    /// dark palette, darkened to keep contrast on white.
    pub const fn light() -> Self {
        let text = [36, 41, 47, 255];
        Self::plain(text)
            .with(Kind::Comment, [106, 115, 125, 255])
            .with(Kind::String, [3, 102, 50, 255])
            .with(Kind::StringSpecial, [3, 102, 50, 255])
            .with(Kind::Escape, [3, 102, 50, 255])
            .with(Kind::Keyword, [155, 35, 146, 255])
            .with(Kind::Function, [0, 92, 197, 255])
            .with(Kind::FunctionBuiltin, [0, 92, 197, 255])
            .with(Kind::Macro, [0, 92, 197, 255])
            .with(Kind::Type, [149, 88, 0, 255])
            .with(Kind::TypeBuiltin, [149, 88, 0, 255])
            .with(Kind::Number, [176, 64, 16, 255])
            .with(Kind::Constant, [176, 64, 16, 255])
            .with(Kind::Operator, [60, 90, 140, 255])
            .with(Kind::Property, [5, 80, 174, 255])
    }
}

/// A span in UTF-16 code units, for platform text systems: `start` and `len`
/// in the paragraph, and the foreground as RGBA.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Utf16Span {
    pub start: u32,
    pub len: u32,
    pub rgba: [u8; 4],
}

/// Converts UTF-8 spans of `text` to UTF-16 ranges. Spans that do not name
/// character boundaries of `text` are dropped rather than rounded.
pub fn utf16_spans(text: &str, spans: &[Span]) -> Vec<Utf16Span> {
    let mut out = Vec::with_capacity(spans.len());
    let mut units = 0u32;
    let mut byte = 0usize;
    // Spans arrive in order and do not overlap, so one pass converts them.
    let mut at = |target: usize| -> Option<u32> {
        if target < byte || !text.is_char_boundary(target) {
            return None;
        }
        units += text[byte..target].encode_utf16().count() as u32;
        byte = target;
        Some(units)
    };
    for span in spans {
        let (Some(start), Some(end)) = (at(span.start), at(span.end)) else {
            continue;
        };
        if end > start {
            out.push(Utf16Span {
                start,
                len: end - start,
                rgba: span.foreground,
            });
        }
    }
    out
}

/// Highlights synchronously with one process-wide highlighter, for adapters
/// that call from their own worker thread (the phone hosts). The first call
/// compiles the grammar queries. Unknown languages and oversized inputs
/// return no spans.
pub fn highlight_utf16(language: &str, text: &str, light: bool) -> Vec<Utf16Span> {
    static SHARED: std::sync::LazyLock<std::sync::Mutex<Highlighter>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(Highlighter::default()));
    if text.len() > MAX_BYTES || language.is_empty() {
        return vec![];
    }
    let palette = if light {
        Palette::light()
    } else {
        Palette::default()
    };
    let spans = match SHARED.lock() {
        Ok(highlighter) => highlighter.spans_with_palette(language, text, palette),
        Err(_) => return vec![],
    };
    utf16_spans(text, &spans)
}

impl Default for Highlighter {
    fn default() -> Self {
        Self {
            grammars: code_highlight::Grammars::default(),
            palette: Palette::default(),
        }
    }
}
impl Highlighter {
    pub fn with_palette(palette: Palette) -> Self {
        Self {
            palette,
            ..Self::default()
        }
    }
    pub fn spans(&self, language: &str, text: &str) -> Vec<Span> {
        self.spans_with_palette(language, text, self.palette)
    }
    pub fn spans_with_palette(&self, language: &str, text: &str, palette: Palette) -> Vec<Span> {
        let mut output: Vec<Span> = vec![];
        for class in self.grammars.classes(language, text) {
            let foreground = class.kind.map_or(palette.text, |kind| palette.color(kind));
            if let Some(last) = output.last_mut()
                && last.end == class.start
                && last.foreground == foreground
            {
                last.end = class.end;
            } else {
                output.push(Span {
                    start: class.start,
                    end: class.end,
                    foreground,
                });
            }
            if output.len() > MAX_SPANS {
                return vec![];
            }
        }
        output
    }
}

/// Bounded asynchronous highlighting for native adapters. Calling `request`
/// never waits for the grammar engine; a completed result can wake the adapter.
pub struct Cache {
    results: std::sync::mpsc::Receiver<ResultRow>,
    sender: std::sync::mpsc::Sender<ResultRow>,
    entries: std::collections::HashMap<(String, String), Option<std::sync::Arc<Vec<Span>>>>,
    bytes: usize,
    palette: Palette,
}
type Wake = std::sync::Arc<dyn Fn() + Send + Sync>;
type ResultRow = ((String, String), Vec<Span>);
struct Job {
    key: (String, String),
    results: std::sync::mpsc::Sender<ResultRow>,
    wake: Option<Wake>,
    palette: Palette,
}
static WORKER: std::sync::LazyLock<std::sync::mpsc::SyncSender<Job>> =
    std::sync::LazyLock::new(|| {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<Job>(16);
        let _ = std::thread::Builder::new()
            .name("syntax-highlight".into())
            .spawn(move || {
                let highlighter = Highlighter::default();
                while let Ok(job) = receiver.recv() {
                    let spans = highlighter.spans_with_palette(&job.key.0, &job.key.1, job.palette);
                    if job.results.send((job.key, spans)).is_ok()
                        && let Some(wake) = job.wake
                    {
                        wake();
                    }
                }
            });
        sender
    });
impl Default for Cache {
    fn default() -> Self {
        let (sender, results) = std::sync::mpsc::channel();
        Self {
            results,
            sender,
            entries: Default::default(),
            bytes: 0,
            palette: Palette::default(),
        }
    }
}
impl Cache {
    /// Discards old results and pending deliveries when foreground colors change.
    pub fn set_palette(&mut self, palette: Palette) -> bool {
        if self.palette == palette {
            return false;
        }
        let (sender, results) = std::sync::mpsc::channel();
        self.sender = sender;
        self.results = results;
        self.entries.clear();
        self.bytes = 0;
        self.palette = palette;
        true
    }
    /// Drains completed work. A result for an evicted block is discarded.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((key, spans)) = self.results.try_recv() {
            if let Some(entry) = self.entries.get_mut(&key) {
                // Input bytes and span storage have separate fixed bounds.
                *entry = Some(std::sync::Arc::new(spans));
                changed = true;
            }
        }
        changed
    }
    pub fn request(&mut self, language: &str, text: &str, wake: Option<Wake>) {
        if text.len() > MAX_BYTES || language.is_empty() {
            return;
        }
        if self.entries.keys().any(|(l, t)| l == language && t == text) {
            return;
        }
        if self
            .entries
            .values()
            .filter(|entry| entry.is_none())
            .count()
            >= 16
        {
            return;
        }
        // At most 32 cached blocks, 2 MiB of input, and 16 pending results.
        if self.entries.len() >= 32 || self.bytes + text.len() > 2 * 1024 * 1024 {
            self.entries.retain(|_, entry| entry.is_none());
            self.bytes = self.entries.keys().map(|(_, text)| text.len()).sum();
        }
        let key = (language.to_owned(), text.to_owned());
        let job = Job {
            key: key.clone(),
            results: self.sender.clone(),
            wake,
            palette: self.palette,
        };
        if WORKER.try_send(job).is_ok() {
            self.bytes += text.len();
            self.entries.insert(key, None);
        }
    }
    pub fn get(&self, language: &str, text: &str) -> Option<&[Span]> {
        // Borrow the stored strings; painting doesn't allocate a code copy.
        self.entries
            .iter()
            .find(|((l, t), _)| l == language && t == text)
            .and_then(|(_, spans)| spans.as_deref())
            .map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_changes_preserve_bytes_and_refuse_old_pending_colors() {
        let palette = Palette::plain([220, 220, 220, 255])
            .with(Kind::Keyword, [110, 120, 210, 255])
            .with(Kind::String, [80, 190, 130, 255]);
        let source = "let café = \"hello\";\n";
        let spans = Highlighter::with_palette(palette).spans("rust", source);
        assert_eq!(
            spans
                .iter()
                .map(|s| &source[s.start..s.end])
                .collect::<String>(),
            source
        );
        assert!(
            spans.iter().any(|s| &source[s.start..s.end] == "let"
                && s.foreground == palette.color(Kind::Keyword))
        );
        assert!(
            spans
                .iter()
                .any(|s| s.foreground == palette.color(Kind::String))
        );
        let mut cache = Cache::default();
        let key = ("rust".into(), source.into());
        let old_delivery = cache.sender.clone();
        cache.entries.insert(key.clone(), None);
        old_delivery.send((key.clone(), vec![])).unwrap();
        assert!(cache.set_palette(palette));
        cache.entries.insert(key.clone(), None);
        assert!(!cache.poll());
        assert!(old_delivery.send((key.clone(), vec![])).is_err());
        cache.sender.send((key, spans.clone())).unwrap();
        assert!(cache.poll());
        assert_eq!(cache.get("rust", source), Some(spans.as_slice()));
        assert!(!cache.set_palette(palette));
        assert_eq!(cache.get("rust", source), Some(spans.as_slice()));
    }
    #[test]
    fn phone_spans_are_utf16_and_cover_the_text() {
        let text = "// café 🦀\nfn main() { let s = \"héllo\"; }\n";
        for light in [false, true] {
            let spans = highlight_utf16("rust", text, light);
            assert!(!spans.is_empty());
            let total: u32 = spans.iter().map(|s| s.len).sum();
            assert_eq!(total as usize, text.encode_utf16().count());
            let mut next = 0;
            for span in &spans {
                assert_eq!(span.start, next);
                next = span.start + span.len;
            }
            let palette = if light {
                Palette::light()
            } else {
                Palette::default()
            };
            assert!(spans.iter().any(|s| s.rgba == palette.color(Kind::Keyword)));
        }
        assert!(highlight_utf16("unknown", text, false).is_empty());
        assert!(highlight_utf16("", text, false).is_empty());
        let bad = [Span {
            start: 1,
            end: 3,
            foreground: [0; 4],
        }];
        assert!(utf16_spans("é", &bad).is_empty());
    }
    #[test]
    fn highlights_common_languages_without_changing_bytes() {
        let highlighter = Highlighter::default();
        for (language, text) in [
            ("rust", "fn main() { let café = 42; }\n"),
            ("python", "# comment\nprint(\"hi\")\n"),
            ("json", "{\"answer\":42}\n"),
            ("bash", "echo \"hello\"\n"),
            ("javascript", "function answer() { return 42; }\n"),
            ("typescript", "function answer(): number { return 42; }\n"),
            ("go", "package main\nfunc answer() int { return 42 }\n"),
            ("cpp", "int main() { return 42; }\n"),
            ("c", "int main() { return 42; }\n"),
            ("html", "<p class=\"word\">hello</p>\n"),
            ("css", "p { color: red; }\n"),
        ] {
            let spans = highlighter.spans(language, text);
            assert!(!spans.is_empty(), "{language}");
            assert!(
                spans
                    .iter()
                    .any(|span| span.foreground != spans[0].foreground),
                "{language}"
            );
            assert_eq!(
                spans
                    .iter()
                    .map(|span| &text[span.start..span.end])
                    .collect::<String>(),
                text
            );
        }
        assert!(
            highlighter
                .spans("unsupported-future-language", "x")
                .is_empty()
        );
        assert!(
            highlighter
                .spans("rust", &"a".repeat(MAX_BYTES + 1))
                .is_empty()
        );
    }
}
