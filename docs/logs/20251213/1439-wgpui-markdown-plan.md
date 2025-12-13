# Plan: wgpui Markdown Rendering with Syntax Highlighting & Streaming

## Overview

Build a markdown rendering system for wgpui with:
- Full markdown support (headers, lists, code blocks, blockquotes, tables)
- Syntax highlighting via syntect (WASM-compatible)
- Streaming support for incremental rendering

## Key Learnings from Zed's GPUI

1. **TextRun accumulation**: Styled text segments with `(range, style)` pairs
2. **Two-phase rendering**: Parse → AST → Styled spans → GPU primitives
3. **Streaming**: Bounded channels + cooperative yielding + debouncing
4. **Syntax highlighting**: Theme maps token names to colors (longest-match-first)

## Architecture

```
Markdown Source
    ↓
pulldown-cmark Parser
    ↓
MarkdownDocument (AST: blocks + styled lines)
    ↓
MarkdownRenderer → Scene (Quads + TextRuns)
    ↓
GPU Rendering
```

## Dependencies

```toml
# Add to crates/wgpui/Cargo.toml
pulldown-cmark = "0.12"
syntect = { version = "5.2", default-features = false, features = [
    "default-fancy",  # fancy-regex for WASM compatibility
    "parsing",
] }
```

## File Structure

```
crates/wgpui/src/markdown/
├── mod.rs              # Public API exports
├── types.rs            # StyledSpan, StyledLine, MarkdownBlock, MarkdownDocument
├── parser.rs           # pulldown-cmark integration
├── highlighter.rs      # syntect integration
├── streaming.rs        # Incremental parsing + debouncing
└── renderer.rs         # Render to Scene
```

## Core Types

```rust
// types.rs
pub struct TextStyle {
    pub color: Hsla,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub monospace: bool,
    pub background: Option<Hsla>,
}

pub struct StyledSpan {
    pub text: String,
    pub style: TextStyle,
}

pub struct StyledLine {
    pub spans: Vec<StyledSpan>,
    pub line_height: f32,
    pub margin_top: f32,
    pub indent: u32,
}

pub enum MarkdownBlock {
    Paragraph(Vec<StyledLine>),
    Header { level: u8, lines: Vec<StyledLine> },
    CodeBlock { language: Option<String>, lines: Vec<StyledLine> },
    Blockquote(Vec<MarkdownBlock>),
    UnorderedList(Vec<Vec<MarkdownBlock>>),
    OrderedList { start: u64, items: Vec<Vec<MarkdownBlock>> },
    HorizontalRule,
    Table { headers: Vec<Vec<StyledLine>>, rows: Vec<Vec<Vec<StyledLine>>> },
}

pub struct MarkdownDocument {
    pub blocks: Vec<MarkdownBlock>,
    pub is_complete: bool,
}
```

## Syntax Highlighting

```rust
// highlighter.rs
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,  // syntect bundled syntaxes
    theme_set: ThemeSet,    // base16-ocean.dark theme
}

impl SyntaxHighlighter {
    pub fn highlight(&self, code: &str, language: &str) -> Vec<StyledLine>;
}

// Supported languages (WASM-compatible via fancy-regex)
pub const SUPPORTED_LANGUAGES: &[&str] = &[
    "rust", "javascript", "typescript", "python", "json",
    "yaml", "markdown", "bash", "html", "css", "sql",
];
```

## Streaming Support

```rust
// streaming.rs
pub struct StreamingMarkdown {
    source: String,
    document: MarkdownDocument,
    pending_chunks: VecDeque<String>,
    last_parse: Option<Instant>,
}

impl StreamingMarkdown {
    pub fn append(&mut self, chunk: &str);  // Add streaming text
    pub fn complete(&mut self);              // Mark stream complete
    pub fn tick(&mut self) -> bool;          // Process pending, returns if updated
    pub fn document(&self) -> &MarkdownDocument;
}

// Debouncing: Re-parse at most every 16ms (~60fps)
// Cooperative yielding for WASM responsiveness
```

## Renderer API

```rust
// renderer.rs
pub struct MarkdownRenderer {
    config: MarkdownConfig,
}

impl MarkdownRenderer {
    pub fn render(
        &self,
        document: &MarkdownDocument,
        origin: Point,
        max_width: f32,
        text_system: &mut TextSystem,
        scene: &mut Scene,
    ) -> Size;
}

// Convenience function
pub fn render_markdown(
    markdown: &str,
    origin: Point,
    max_width: f32,
    text_system: &mut TextSystem,
    scene: &mut Scene,
) -> Size;
```

## Demo Plan

Update `crates/wgpui/src/lib.rs` main() to show:
1. Static markdown rendering with all block types
2. Syntax-highlighted code blocks (Rust, JS, Python)
3. Streaming simulation (characters appended over time)

Demo markdown content:
- H1/H2/H3 headers
- Paragraphs with **bold**, *italic*, `inline code`
- Fenced code block with Rust syntax highlighting
- Blockquote with accent bar
- Bullet and numbered lists
- Horizontal rule

## Implementation Steps

1. **Add dependencies** to Cargo.toml (pulldown-cmark, syntect)
2. **Create types.rs** - all data structures
3. **Create parser.rs** - pulldown-cmark integration
4. **Create highlighter.rs** - syntect with fancy-regex
5. **Create renderer.rs** - Scene rendering
6. **Create streaming.rs** - incremental parsing
7. **Create mod.rs** - public API
8. **Update lib.rs** - add demo showing all features
9. **Test WASM build** - trunk build

## Critical Files

| File | Purpose |
|------|---------|
| `crates/wgpui/Cargo.toml` | Add dependencies |
| `crates/wgpui/src/lib.rs` | Export markdown module, update demo |
| `crates/wgpui/src/text.rs` | Reference for TextSystem integration |
| `crates/wgpui/src/scene.rs` | Reference for Scene/TextRun |
| `crates/wgpui/src/theme.rs` | Theme colors for markdown |

## Success Criteria

- [ ] All markdown block types render correctly
- [ ] Code blocks have syntax highlighting
- [ ] Streaming demo shows incremental rendering
- [ ] WASM build works (trunk build succeeds)
- [ ] 60fps with moderate markdown content
