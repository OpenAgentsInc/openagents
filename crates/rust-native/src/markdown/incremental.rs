//! Streaming Markdown: reparse only the tail of a growing message.
//!
//! Text before the start of the last top-level block can't change when text
//! is appended, with two exceptions this module handles explicitly:
//!
//! - When the last block starts on the final, unterminated line, the rest of
//!   that line can reclassify it. For example, `1. a\n\n1` is a list and a
//!   paragraph, but `1. a\n\n1. b` is one loose list. The reparse then starts
//!   one block earlier, at the block the line could join.
//! - A link reference definition (`[label]: url`) resolves references
//!   anywhere in the document, so a later definition can restyle an earlier
//!   block. Every definition contains `]:`, so once the source contains `]:`,
//!   each update parses the whole source.
//!
//! Footnotes, heading attributes, and other extensions whose meaning crosses
//! blocks are not enabled in [`super::parse`], so they need no rule. The
//! parity tests stream corpora in random pieces and compare every step with a
//! full parse.
//!
//! Once the source has a component block (```` ```openui-lang ````), each
//! update parses its Markdown fallback ([`super::shown`]). The fallback
//! usually grows at its end like the source, so only its tail is reparsed;
//! when it changes earlier, as when a statement fills in a forward
//! reference, it is parsed whole.

#[cfg(test)]
use super::parse;
use super::{Block, Start, mend, parse_starts, shown};
use std::borrow::Cow;

/// A Markdown source that grows while a reply streams, with its parsed
/// blocks.
///
/// [`blocks`](Self::blocks) always equals [`parse`] of the whole source.
/// [`display_blocks`](Self::display_blocks) replaces the last block with a
/// mended copy that closes half-written emphasis, code, and links, so the
/// tail doesn't reflow when the closing marker arrives. The mended copy is
/// for display only: a link whose destination is still streaming has an
/// empty destination, and a bare URL at the very end stays text until
/// something follows it.
#[derive(Clone, Debug, Default)]
pub struct IncrementalMarkdown {
    /// The text as received.
    raw: String,
    /// The source has a component block, so `source` is its fallback.
    ui: bool,
    /// The Markdown parsed: `raw`, or its fallback.
    source: String,
    blocks: Vec<Block>,
    starts: Vec<Start>,
    stable: usize,
    /// The source may contain a link reference definition.
    references: bool,
    reparsed: usize,
    /// A mended tail that replaces `blocks[at..]` for display.
    display: Option<(usize, Vec<Block>)>,
}

impl IncrementalMarkdown {
    pub fn new(source: &str) -> Self {
        let mut markdown = Self::default();
        markdown.set(source);
        markdown
    }

    /// The whole source, as received.
    pub fn source(&self) -> &str {
        &self.raw
    }

    /// The exact blocks of the whole source, never mended.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The blocks to draw: the canonical blocks, with the last one mended
    /// when it ends in half-written syntax.
    pub fn display_blocks(&self) -> Cow<'_, [Block]> {
        match &self.display {
            None => Cow::Borrowed(&self.blocks),
            Some((at, tail)) => {
                Cow::Owned(self.blocks[..*at].iter().chain(tail).cloned().collect())
            }
        }
    }

    /// Whether [`display_blocks`](Self::display_blocks) differs from
    /// [`blocks`](Self::blocks).
    pub fn mended(&self) -> bool {
        self.display.is_some()
    }

    /// How many leading blocks the last update left unchanged. Render caches
    /// for those blocks stay valid.
    pub fn stable_prefix(&self) -> usize {
        self.stable
    }

    /// Source bytes the last update parsed, not counting the mended display
    /// copy.
    pub fn reparsed_bytes(&self) -> usize {
        self.reparsed
    }

    /// Append streamed text.
    pub fn append(&mut self, text: &str) {
        if text.is_empty() {
            self.stable = self.blocks.len();
            self.reparsed = 0;
            return;
        }
        let mut from = self.raw.len().saturating_sub(openui_lang::LANG.len());
        while !self.raw.is_char_boundary(from) {
            from -= 1;
        }
        self.raw.push_str(text);
        self.ui = self.ui || self.raw[from..].contains(openui_lang::LANG);
        if self.ui {
            let shown = shown(&self.raw).into_owned();
            self.show(&shown);
        } else {
            self.extend(text);
        }
    }

    /// Replace the source. When `source` extends the current source, only
    /// the tail is reparsed.
    pub fn set(&mut self, source: &str) {
        if let Some(rest) = source.strip_prefix(self.raw.as_str()) {
            self.append(rest);
            return;
        }
        self.raw.clear();
        self.raw.push_str(source);
        self.ui = source.contains(openui_lang::LANG);
        let shown = shown(source).into_owned();
        self.show(&shown);
    }

    /// Parse `shown`, the Markdown for the whole source, reparsing only its
    /// tail when it extends what was parsed before.
    fn show(&mut self, shown: &str) {
        if let Some(rest) = shown.strip_prefix(self.source.as_str()) {
            self.extend(rest);
            return;
        }
        self.source.clear();
        self.source.push_str(shown);
        self.references = shown.contains("]:");
        self.reparse(0);
    }

    /// Parse `text` added to the end of the Markdown.
    fn extend(&mut self, text: &str) {
        if text.is_empty() {
            self.stable = self.blocks.len();
            self.reparsed = 0;
            return;
        }
        let old = self.source.len();
        self.references = self.references
            || text.contains("]:")
            || (self.source.ends_with(']') && text.starts_with(':'));
        let index = self.reparse_index(old);
        self.source.push_str(text);
        if self.references {
            self.reparse(0);
        } else {
            self.reparse(index);
        }
    }

    /// The first start to reparse from after text is appended to a source
    /// of `old` bytes.
    fn reparse_index(&self, old: usize) -> usize {
        let Some(last) = self.starts.len().checked_sub(1) else {
            return 0;
        };
        let open_line = !self.source[self.starts[last].offset..old].contains(['\n', '\r']);
        if open_line {
            last.saturating_sub(1)
        } else {
            last
        }
    }

    /// Reparse from `starts[index]`, or the whole source when `index` is 0.
    fn reparse(&mut self, index: usize) {
        let Start { offset, before } = match index {
            0 => Start {
                offset: 0,
                before: 0,
            },
            _ => self.starts[index],
        };
        let (tail, starts) = parse_starts(&self.source[offset..], false);
        self.reparsed = self.source.len() - offset;
        let old = self.blocks.split_off(before);
        let same = old.iter().zip(&tail).take_while(|(a, b)| a == b).count();
        self.stable = before + same;
        self.blocks.extend(tail);
        self.starts.truncate(index);
        self.starts.extend(starts.into_iter().map(|start| Start {
            offset: offset + start.offset,
            before: before + start.before,
        }));
        self.mend();
    }

    /// Mend the last block for display, then hold back what still can't
    /// render cleanly: a half table row, a table header without its
    /// delimiter row, a bare list or heading marker
    /// ([`markdown_stream::renderable`], the cut every chat surface shares;
    /// #11112). An open code block is already drawn as code. A bare URL at
    /// the very end may still be growing, so it stays text.
    fn mend(&mut self) {
        self.display = self.starts.last().and_then(|last| {
            let source = &self.source[last.offset..];
            let code = matches!(self.blocks.last(), Some(Block::Code { .. }));
            let mended = mend::tail(source, code);
            let shown = mended.as_deref().unwrap_or(source);
            let cut = if code {
                Cow::Borrowed(shown)
            } else {
                markdown_stream::renderable(shown)
            };
            if mended.is_none() && cut.len() == source.len() && !growing_url(source) {
                return None;
            }
            let tail = parse_starts(&cut, true).0;
            (tail[..] != self.blocks[last.before..]).then_some((last.before, tail))
        });
    }
}

/// Whether `text` ends in a bare URL that more text could still extend.
fn growing_url(text: &str) -> bool {
    let word = text
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(at, c)| at + c.len_utf8());
    let word = &text[word..];
    !word.is_empty()
        && markdown_stream::autolink::find(word, true).len()
            != markdown_stream::autolink::find(word, false).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::Span;

    /// A small seeded generator, so a failure names its seed.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    const REPLY: &str = r#"I found the problem. The **retry loop** in `client.rs` never resets its
backoff, so after one failure every request waits *at least* 30 seconds.

## What I changed

1. Reset `backoff` after a successful response.
2. Cap the delay at 5 seconds:

   ```rust
   let delay = backoff.min(Duration::from_secs(5));
   ```

3. Added a test, `retries_reset_after_success`.

| File | Lines | Change |
|:-----|------:|:------:|
| `client.rs` | 12 | **fix** |
| `tests.rs` | 40 | ~~none~~ new |

> Note: the *old* behavior is documented in [the design doc](https://example.test/design "Design").

- [x] Tests pass
- [ ] Docs updated
  - nested *item* with `code`

---

Run it with:

~~~sh
cargo test -p client
~~~

Let me know if you want ***both*** fixes or just the first. Ünïcödé — ok 🙂
"#;

    const EDGE: &str = r#"Setext heading
==============

Another
---

1. a

1. b

2) c
paragraph after

    indented code

    more code

<div>
html *block*
</div>
<p>second</p>

* star list
+ plus list

text
- interrupts

> quote
lazy continuation
> > nested

a | b
- | -
1 | 2

```
unclosed fence *not emphasis*
"#;

    const CRLF: &str = "# Title\r\n\r\nLine one\r\nline two with **bold**\r\n\r\n- x\r\n- y\r\n\r\n```\r\ncode\r\n```\r\n";

    const NESTED: &str = r#"- a

  b

- c
  1. x

     1\. y
- ```
  in item
  ```

<!--

comment

-->

  > q
  > q2 **x

text
    lazy

* * *

| a |
| - |
| 1 |
trailing
~~~~
~~~
"#;

    const REFERENCES: &str = "See [the docs][docs] and [more].\n\n- item\n\n[docs]: https://example.test\n[more]: /more\n";

    fn stream(corpus: &str, seed: u64, max: usize) {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut markdown = IncrementalMarkdown::default();
        let mut end = 0;
        while end < corpus.len() {
            let mut next = (end + 1 + rng.below(max)).min(corpus.len());
            while !corpus.is_char_boundary(next) {
                next += 1;
            }
            if rng.below(4) == 0 {
                markdown.set(&corpus[..next]);
            } else {
                markdown.append(&corpus[end..next]);
            }
            end = next;
            assert_eq!(
                markdown.blocks(),
                parse(&corpus[..end]),
                "seed {seed}, prefix {:?}",
                &corpus[..end]
            );
            assert!(markdown.stable_prefix() <= markdown.blocks().len());
            if !markdown.mended() {
                assert_eq!(markdown.display_blocks()[..], markdown.blocks()[..]);
            }
        }
    }

    #[test]
    fn streaming_matches_a_full_parse_at_every_step() {
        for corpus in [REPLY, EDGE, NESTED, CRLF, REFERENCES] {
            for seed in 0..60 {
                stream(corpus, seed, 1 + (seed as usize % 3) * 12);
            }
        }
    }

    #[test]
    fn replacing_with_unrelated_text_parses_it_whole() {
        let mut markdown = IncrementalMarkdown::new("# One\n\ntwo");
        markdown.set("- three");
        assert_eq!(markdown.blocks(), parse("- three"));
        assert_eq!(markdown.stable_prefix(), 0);
        assert_eq!(markdown.reparsed_bytes(), "- three".len());
    }

    #[test]
    fn stable_prefix_counts_unchanged_leading_blocks() {
        let mut markdown = IncrementalMarkdown::new("# A\n\npara one\n\npara");
        assert_eq!(markdown.stable_prefix(), 0);
        markdown.append(" two");
        assert_eq!(markdown.blocks().len(), 3);
        assert_eq!(markdown.stable_prefix(), 2);
        markdown.append("\n\n- x");
        assert_eq!(markdown.blocks().len(), 4);
        assert_eq!(markdown.stable_prefix(), 3);
        markdown.append("");
        assert_eq!(markdown.stable_prefix(), 4);
        // A line that joins the list before it changes that list.
        let mut list = IncrementalMarkdown::new("1. a\n\n1");
        assert_eq!(list.blocks().len(), 2);
        list.append(". b");
        assert_eq!(list.blocks(), parse("1. a\n\n1. b"));
        assert_eq!(list.stable_prefix(), 0);
    }

    #[test]
    fn per_append_work_is_bounded_by_the_tail() {
        let paragraph = "Some *streamed* text with `code` and a [link](https://x.test).\n\n";
        let corpus = paragraph.repeat(400);
        let mut markdown = IncrementalMarkdown::default();
        let mut total = 0;
        for piece in corpus.as_bytes().chunks(7) {
            markdown.append(std::str::from_utf8(piece).expect("ASCII"));
            // At most the last two paragraphs and the new piece.
            assert!(markdown.reparsed_bytes() <= 2 * paragraph.len() + 7);
            total += markdown.reparsed_bytes();
        }
        assert_eq!(markdown.blocks(), parse(&corpus));
        // A full parse per append would read about 1.8 GB; the tail reads
        // under 1 MB.
        assert!(total < 2 * paragraph.len() * corpus.len() / 7);
        assert!(total < 1_000_000, "{total}");
    }

    #[test]
    fn a_reference_definition_forces_full_parses() {
        let mut markdown = IncrementalMarkdown::new("[a] first\n\nsecond\n\n[a]");
        markdown.append(": /x\n");
        assert_eq!(markdown.blocks(), parse("[a] first\n\nsecond\n\n[a]: /x\n"));
        assert_eq!(markdown.reparsed_bytes(), markdown.source().len());
        let Block::Paragraph { spans } = &markdown.blocks()[0] else {
            panic!("paragraph")
        };
        // A site path points at the site.
        assert_eq!(spans[0].link.as_deref(), Some("https://openagents.com/x"));
    }

    fn display_spans(markdown: &IncrementalMarkdown) -> Vec<Span> {
        match markdown.display_blocks().last() {
            Some(Block::Paragraph { spans }) => spans.clone(),
            other => panic!("paragraph, got {other:?}"),
        }
    }

    fn styled(text: &str, style: impl FnOnce(&mut Span)) -> Span {
        let mut span = Span {
            text: text.into(),
            ..Span::default()
        };
        style(&mut span);
        span
    }

    #[test]
    fn mending_closes_each_construct_for_display_only() {
        let cases: [(&str, Vec<Span>); 9] = [
            (
                "a **bold",
                vec![styled("a ", |_| {}), styled("bold", |s| s.bold = true)],
            ),
            (
                "a *em",
                vec![styled("a ", |_| {}), styled("em", |s| s.italic = true)],
            ),
            (
                "a _em",
                vec![styled("a ", |_| {}), styled("em", |s| s.italic = true)],
            ),
            (
                "a ~~gone",
                vec![styled("a ", |_| {}), styled("gone", |s| s.strike = true)],
            ),
            (
                "run `cargo te",
                vec![
                    styled("run ", |_| {}),
                    styled("cargo te", |s| s.code = true),
                ],
            ),
            (
                "see [the docs](https://exa",
                vec![
                    styled("see ", |_| {}),
                    styled("the docs", |s| s.link = Some(String::new())),
                ],
            ),
            (
                "**a *b",
                vec![
                    styled("a ", |s| s.bold = true),
                    styled("b", |s| {
                        s.bold = true;
                        s.italic = true
                    }),
                ],
            ),
            ("**a*", vec![styled("a", |s| s.bold = true)]),
            ("Hello **", vec![styled("Hello", |_| {})]),
        ];
        for (source, want) in cases {
            let markdown = IncrementalMarkdown::new(source);
            assert!(markdown.mended(), "{source}");
            assert_eq!(display_spans(&markdown), want, "{source}");
            // The canonical tree keeps the literal markers.
            assert_eq!(markdown.blocks(), parse(source), "{source}");
        }
    }

    #[test]
    fn mending_keeps_emphasis_outside_a_partial_link() {
        let markdown = IncrementalMarkdown::new("**see [docs](ht");
        assert_eq!(
            display_spans(&markdown),
            vec![
                styled("see ", |s| s.bold = true),
                styled("docs", |s| {
                    s.bold = true;
                    s.link = Some(String::new())
                }),
            ]
        );
    }

    #[test]
    fn mending_leaves_code_blocks_and_finished_paragraphs_alone() {
        let fence = IncrementalMarkdown::new("```rust\nfn main() { *x\n``");
        assert!(fence.mended());
        assert_eq!(
            fence.display_blocks().last(),
            Some(&Block::Code {
                language: Some("rust".into()),
                text: "fn main() { *x\n".into()
            })
        );
        assert_eq!(
            fence.blocks().last(),
            Some(&Block::Code {
                language: Some("rust".into()),
                text: "fn main() { *x\n``".into()
            })
        );
        let open = IncrementalMarkdown::new("```\nlet *a = 1;");
        assert!(!open.mended());
        assert!(matches!(
            open.display_blocks().last(),
            Some(Block::Code { .. })
        ));
        // A blank line ends the paragraph, so its markers are final.
        assert!(!IncrementalMarkdown::new("a **b\n\n").mended());
        assert!(!IncrementalMarkdown::new("plain text").mended());
        // A heading's open marker doesn't carry into the next paragraph.
        let heading = IncrementalMarkdown::new("# A *b\ntext");
        assert_eq!(heading.display_blocks()[..], heading.blocks()[..]);
    }

    #[test]
    fn mending_works_inside_list_items_and_quotes() {
        let list = IncrementalMarkdown::new("- first\n- second **bo");
        let Some(Block::List { items, .. }) = list.display_blocks().last().cloned() else {
            panic!("list")
        };
        assert_eq!(
            items[1].blocks,
            vec![Block::Paragraph {
                spans: vec![styled("second ", |_| {}), styled("bo", |s| s.bold = true)]
            }]
        );
        let quote = IncrementalMarkdown::new("> quoted `co");
        let Some(Block::Quote { blocks }) = quote.display_blocks().last().cloned() else {
            panic!("quote")
        };
        assert_eq!(
            blocks,
            vec![Block::Paragraph {
                spans: vec![styled("quoted ", |_| {}), styled("co", |s| s.code = true)]
            }]
        );
    }

    /// What mending can't close is held back for display: a half table
    /// row, a header without its delimiter row, a bare list or heading
    /// marker (#11112).
    #[test]
    fn a_half_table_or_bare_marker_is_held_back_for_display() {
        for (source, shown) in [
            ("Intro\n\n| a | b |\n", "Intro\n\n"),
            (
                "| a | b |\n|---|---|\n| 1 | 2 |\n| 3",
                "| a | b |\n|---|---|\n| 1 | 2 |\n",
            ),
            ("- a\n- ", "- a\n"),
            ("Intro\n\n##", "Intro\n\n"),
        ] {
            let markdown = IncrementalMarkdown::new(source);
            assert_eq!(
                markdown.display_blocks()[..],
                parse(shown)[..],
                "{source:?}"
            );
            assert_eq!(markdown.blocks(), parse(source), "{source:?}");
        }
    }

    fn link_targets(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .flat_map(|block| match block {
                Block::Paragraph { spans } => spans.clone(),
                _ => vec![],
            })
            .filter_map(|span| span.link)
            .collect()
    }

    #[test]
    fn a_bare_url_is_linked_only_once_it_ends() {
        let source = "Sign in at https://openagents.com/device, then come back.";
        let end = source.find(',').expect("comma");
        let mut markdown = IncrementalMarkdown::default();
        for (at, _) in source.char_indices().skip(1) {
            markdown.set(&source[..at]);
            assert_eq!(markdown.blocks(), parse(&source[..at]));
            let shown = link_targets(&markdown.display_blocks());
            if at <= end {
                // Still growing: no partial URL is ever a link.
                assert!(shown.is_empty(), "{:?}: {shown:?}", &source[..at]);
            } else if at > end + 1 {
                assert_eq!(
                    shown,
                    ["https://openagents.com/device"],
                    "{:?}",
                    &source[..at]
                );
            }
        }
        markdown.set(source);
        assert_eq!(
            link_targets(markdown.blocks()),
            ["https://openagents.com/device"]
        );
        // An ended reply whose last word is a URL is linked by a full parse.
        assert_eq!(
            link_targets(&parse("Go to openagents.com/device")),
            ["https://openagents.com/device"]
        );
    }

    #[test]
    fn a_streaming_component_block_never_shows_its_statements() {
        let reply = "Connect it here.\n\n```openui-lang\n\
root = Stack([web, steps])\n\
web = Card(\"On the web\", [Button(\"Connect GitHub\", href=\"/projects\")])\n\
steps = Steps([Step(\"Install Coder\", [Command(\"curl -fsSL https://openagents.com/cli/install.sh | bash\")])])\n\
```\n\nThat's all.";
        let mut markdown = IncrementalMarkdown::default();
        for (at, _) in reply.char_indices().skip(1) {
            markdown.append(&reply[markdown.source().len()..at]);
            assert_eq!(markdown.source(), &reply[..at]);
            assert_eq!(markdown.blocks(), parse(&reply[..at]), "{:?}", &reply[..at]);
            let shown = crate::markdown::plain(&markdown.display_blocks());
            for raw in ["root", "Stack(", "Card(", "href", "openui"] {
                assert!(
                    !shown.contains(raw),
                    "{:?} shows {raw}: {shown}",
                    &reply[..at]
                );
            }
            // A half-written command never shows.
            assert!(
                !shown.contains("curl") || shown.contains("install.sh | bash"),
                "{shown}"
            );
        }
        markdown.set(reply);
        let shown = crate::markdown::plain(&markdown.display_blocks());
        assert!(shown.contains("Connect GitHub"), "{shown}");
        assert!(shown.contains("That's all."), "{shown}");
    }

    #[test]
    fn mended_text_never_reaches_the_canonical_blocks() {
        let source = "Use **bold** and [a link](https://x.test) and `code`.";
        let mut markdown = IncrementalMarkdown::default();
        for (at, _) in source.char_indices().skip(1) {
            markdown.set(&source[..at]);
            assert_eq!(markdown.blocks(), parse(&source[..at]));
        }
        markdown.set(source);
        assert!(!markdown.mended());
        assert_eq!(markdown.display_blocks()[..], parse(source)[..]);
    }
}
