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
}
const NAMES: [&str; 9] = [
    "comment", "string", "keyword", "function", "type", "number", "constant", "operator",
    "property",
];
const COLORS: [[u8; 4]; 9] = [
    [123, 137, 151, 255],
    [163, 190, 140, 255],
    [180, 142, 173, 255],
    [143, 188, 187, 255],
    [235, 203, 139, 255],
    [208, 135, 112, 255],
    [208, 135, 112, 255],
    [129, 161, 193, 255],
    [136, 192, 208, 255],
];
impl Default for Highlighter {
    fn default() -> Self {
        let mut configs = std::collections::BTreeMap::new();
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
            (
                "rust",
                tree_sitter_rust::LANGUAGE,
                tree_sitter_rust::HIGHLIGHTS_QUERY,
            ),
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
                config.configure(&NAMES);
                configs.insert(name, config);
            }
        }
        Self { configs }
    }
}
impl Highlighter {
    pub fn spans(&self, language: &str, text: &str) -> Vec<Span> {
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
                        .and_then(|index| COLORS.get(*index))
                        .copied()
                        .unwrap_or([230, 232, 235, 255]);
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
}
type Wake = std::sync::Arc<dyn Fn() + Send + Sync>;
type ResultRow = ((String, String), Vec<Span>);
struct Job {
    key: (String, String),
    results: std::sync::mpsc::Sender<ResultRow>,
    wake: Option<Wake>,
}
static WORKER: std::sync::LazyLock<std::sync::mpsc::SyncSender<Job>> =
    std::sync::LazyLock::new(|| {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<Job>(16);
        let _ = std::thread::Builder::new()
            .name("syntax-highlight".into())
            .spawn(move || {
                let highlighter = Highlighter::default();
                while let Ok(job) = receiver.recv() {
                    let spans = highlighter.spans(&job.key.0, &job.key.1);
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
        }
    }
}
impl Cache {
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
