//! Paint-only syntax spans. Call the highlighter on an adapter's worker.
//!
//! Fonts, text, and layout never change when spans arrive. Unknown languages
//! and inputs beyond the bounds stay plain. No content leaves this process.
use serde::Serialize;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent};

pub const MAX_BYTES: usize = 64 * 1024;
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
    configs: std::collections::BTreeMap<&'static str, HighlightConfiguration>,
    palette: Palette,
}
/// Semantic syntax colors independent of an application's palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Comment,
    String,
    StringSpecial,
    Escape,
    Keyword,
    Function,
    FunctionBuiltin,
    Macro,
    Type,
    TypeBuiltin,
    Constructor,
    Number,
    Boolean,
    Constant,
    Operator,
    Property,
    VariableSpecial,
    Tag,
    Attribute,
    Label,
    MarkupHeading,
    MarkupRaw,
    MarkupLink,
    MarkupReference,
    MarkupEmphasis,
    MarkupStrong,
    Invalid,
}
/// Foregrounds only. A palette changes neither input bytes nor text metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    colors: [[u8; 4]; 27],
    text: [u8; 4],
}
impl Palette {
    pub const fn plain(text: [u8; 4]) -> Self {
        Self {
            colors: [text; 27],
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
const CAPTURES: [(&str, Kind); 30] = [
    ("comment", Kind::Comment),
    ("string", Kind::String),
    ("string.special", Kind::StringSpecial),
    ("escape", Kind::Escape),
    ("keyword", Kind::Keyword),
    ("function", Kind::Function),
    ("function.builtin", Kind::FunctionBuiltin),
    ("function.macro", Kind::Macro),
    ("type", Kind::Type),
    ("type.builtin", Kind::TypeBuiltin),
    ("constructor", Kind::Constructor),
    ("number", Kind::Number),
    ("boolean", Kind::Boolean),
    ("constant", Kind::Constant),
    ("operator", Kind::Operator),
    ("property", Kind::Property),
    ("variable.special", Kind::VariableSpecial),
    ("tag", Kind::Tag),
    ("attribute", Kind::Attribute),
    ("label", Kind::Label),
    ("markup.heading", Kind::MarkupHeading),
    ("markup.raw", Kind::MarkupRaw),
    ("markup.link", Kind::MarkupLink),
    ("markup.link.label", Kind::MarkupReference),
    ("markup.italic", Kind::MarkupEmphasis),
    ("markup.bold", Kind::MarkupStrong),
    ("error", Kind::Invalid),
    ("string.escape", Kind::Escape),
    ("markup.emphasis", Kind::MarkupEmphasis),
    ("markup.strong", Kind::MarkupStrong),
];
impl Default for Highlighter {
    fn default() -> Self {
        let mut configs = std::collections::BTreeMap::new();
        let names = CAPTURES.map(|(name, _)| name);
        // The upstream Rust query groups literals with constants. Preserve
        // their semantic roles so applications can color them independently.
        let rust = tree_sitter_rust::HIGHLIGHTS_QUERY
            .replace(
                "(integer_literal) @constant.builtin",
                "(integer_literal) @number",
            )
            .replace(
                "(float_literal) @constant.builtin",
                "(float_literal) @number",
            )
            .replace(
                "(boolean_literal) @constant.builtin",
                "(boolean_literal) @boolean",
            );
        let cpp = format!(
            "{}\n{}",
            tree_sitter_c::HIGHLIGHT_QUERY,
            tree_sitter_cpp::HIGHLIGHT_QUERY
        );
        let typescript = format!(
            "{}\n{}",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_typescript::HIGHLIGHTS_QUERY
        );
        for (name, language, query) in [
            ("rust", tree_sitter_rust::LANGUAGE, rust.as_str()),
            (
                "python",
                tree_sitter_python::LANGUAGE,
                tree_sitter_python::HIGHLIGHTS_QUERY,
            ),
            (
                "json",
                tree_sitter_json::LANGUAGE,
                tree_sitter_json::HIGHLIGHTS_QUERY,
            ),
            (
                "bash",
                tree_sitter_bash::LANGUAGE,
                tree_sitter_bash::HIGHLIGHT_QUERY,
            ),
            (
                "javascript",
                tree_sitter_javascript::LANGUAGE,
                tree_sitter_javascript::HIGHLIGHT_QUERY,
            ),
            (
                "typescript",
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
                typescript.as_str(),
            ),
            (
                "go",
                tree_sitter_go::LANGUAGE,
                tree_sitter_go::HIGHLIGHTS_QUERY,
            ),
            ("cpp", tree_sitter_cpp::LANGUAGE, cpp.as_str()),
            ("c", tree_sitter_c::LANGUAGE, tree_sitter_c::HIGHLIGHT_QUERY),
            (
                "html",
                tree_sitter_html::LANGUAGE,
                tree_sitter_html::HIGHLIGHTS_QUERY,
            ),
            (
                "css",
                tree_sitter_css::LANGUAGE,
                tree_sitter_css::HIGHLIGHTS_QUERY,
            ),
        ] {
            if let Ok(mut config) =
                HighlightConfiguration::new(language.into(), name, query, "", "")
            {
                config.configure(&names);
                configs.insert(name, config);
            }
        }
        Self {
            configs,
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
        if text.len() > MAX_BYTES || text.lines().any(|line| line.len() > 8192) {
            return vec![];
        }
        let language = match language {
            "rs" => "rust",
            "py" => "python",
            "js" | "jsx" => "javascript",
            "ts" => "typescript",
            "shell" | "sh" => "bash",
            "c++" | "cc" | "cxx" => "cpp",
            value => value,
        };
        let Some(config) = self.configs.get(language) else {
            return vec![];
        };
        let mut highlighter = tree_sitter_highlight::Highlighter::new();
        let Ok(events) = highlighter.highlight(config, text.as_bytes(), None, None, |_| None)
        else {
            return vec![];
        };
        let mut output: Vec<Span> = vec![];
        let mut stack = vec![];
        for event in events {
            match event {
                Ok(HighlightEvent::HighlightStart(value)) => stack.push(value.0),
                Ok(HighlightEvent::HighlightEnd) => {
                    stack.pop();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    let foreground = stack
                        .last()
                        .and_then(|index| CAPTURES.get(*index))
                        .map(|(_, kind)| palette.color(*kind))
                        .unwrap_or(palette.text);
                    if let Some(last) = output.last_mut()
                        && last.end == start
                        && last.foreground == foreground
                    {
                        last.end = end;
                    } else {
                        output.push(Span {
                            start,
                            end,
                            foreground,
                        });
                    }
                    if output.len() > MAX_SPANS {
                        return vec![];
                    }
                }
                Err(_) => return vec![],
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
