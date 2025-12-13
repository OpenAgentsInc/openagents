# Plan: Adopt streamdown/remend Best Practices for wgpui Markdown Streaming

## Problem Statement

Currently, when streaming markdown like `"This is **bold"` arrives, wgpui shows the literal `**` markers because pulldown_cmark (correctly per CommonMark spec) treats unclosed markers as literal text. This creates poor UX during LLM streaming where users expect formatting to apply immediately.

## Key Insight from streamdown

streamdown uses **remend** as a preprocessing step that **completes incomplete markers** before parsing:
- Input: `"This is **bold"`
- After remend: `"This is **bold**"`
- Parser sees complete markdown, renders correctly

This is elegant because:
1. No parser modifications needed - pulldown_cmark works as-is
2. Parity-based counting (odd count = incomplete)
3. Context-aware (skips markers in code blocks, escaped markers, etc.)

## Implementation Plan

### Phase 1: Create `remend` Module

**New file**: `crates/wgpui/src/markdown/remend.rs`

Port the core remend logic from TypeScript to Rust:

```rust
/// Complete incomplete markdown markers for streaming.
pub fn remend(text: &str) -> String {
    let mut result = text.to_string();
    result = complete_bold(&result);
    result = complete_italic(&result);
    result = complete_inline_code(&result);
    result = complete_strikethrough(&result);
    result = complete_links(&result);
    result = prevent_setext_heading(&result);
    result
}
```

**Key functions to implement**:

1. **`complete_bold(text: &str) -> String`**
   - Count `**` pairs
   - If odd count and ends with `**...text`, append `**`
   - Skip if within code block or escaped

2. **`complete_italic(text: &str) -> String`**
   - Count single `*` (not part of `**`)
   - If odd count and ends with `*...text`, append `*`
   - Check it's not a list marker

3. **`complete_inline_code(text: &str) -> String`**
   - Count backticks `` ` ``
   - If odd count and ends with `` `...text ``, append `` ` ``

4. **`complete_strikethrough(text: &str) -> String`**
   - Count `~~` pairs
   - If odd count and ends with `~~...text`, append `~~`

5. **`complete_links(text: &str) -> String`**
   - Detect `[text](incomplete` pattern
   - Replace with `[text](streamdown:incomplete-link)` placeholder

6. **`prevent_setext_heading(text: &str) -> String`**
   - If last line is only `-` or `=`, append zero-width space
   - Prevents accidental setext heading interpretation

**Helper functions**:

```rust
fn is_within_code_block(text: &str, position: usize) -> bool;
fn is_escaped(text: &str, position: usize) -> bool;
fn is_word_char(c: char) -> bool;
fn count_marker_pairs(text: &str, marker: &str) -> usize;
```

### Phase 2: Integrate into StreamingMarkdown

**Modify**: `crates/wgpui/src/markdown/streaming.rs`

```rust
use super::remend::remend;

impl StreamingMarkdown {
    fn reparse(&mut self) {
        // NEW: Apply remend preprocessing before parsing
        let processed = if !self.document.is_complete {
            remend(&self.source)
        } else {
            self.source.clone()
        };

        self.document = self.parser.parse(&processed);
        self.document.is_complete = false; // Still streaming
    }

    pub fn complete(&mut self) {
        // When complete, parse original source (no remend needed)
        self.document = self.parser.parse(&self.source);
        self.document.is_complete = true;
    }
}
```

### Phase 3: Handle Edge Cases

**Context-aware skipping** (critical for correctness):

1. **Code blocks**: Don't complete markers inside ``` blocks
   ```rust
   fn is_within_fenced_code_block(text: &str) -> bool {
       let fence_count = text.matches("```").count();
       fence_count % 2 == 1 // Odd = inside block
   }
   ```

2. **Inline code**: Don't complete markers inside backticks
   ```rust
   fn is_within_inline_code(text: &str, pos: usize) -> bool {
       let before = &text[..pos];
       let backtick_count = before.matches('`').count();
       backtick_count % 2 == 1
   }
   ```

3. **Escaped markers**: Skip `\*`, `\**`, etc.

4. **List markers**: Don't treat `* item` as italic start

5. **Word-internal asterisks**: `foo*bar` is not italic

### Phase 4: Optimize for Performance

Since wgpui targets 60fps rendering:

1. **Tail-focused regex**: Only check patterns at end of string
   ```rust
   lazy_static! {
       static ref BOLD_TAIL: Regex = Regex::new(r"\*\*[^*]*$").unwrap();
       static ref ITALIC_TAIL: Regex = Regex::new(r"(?<!\*)\*[^*]*$").unwrap();
   }
   ```

2. **ASCII fast path** for character checks (matches remend's optimization)

3. **Avoid allocations**: Reuse buffers where possible

4. **Early exit**: If no incomplete markers detected, return original string

## Files to Modify

| File | Changes |
|------|---------|
| `crates/wgpui/src/markdown/remend.rs` | **NEW** - Core marker completion logic |
| `crates/wgpui/src/markdown/streaming.rs` | Integrate remend in `reparse()` |
| `crates/wgpui/src/markdown/mod.rs` | Export remend module |

## Testing Strategy

Port key tests from streamdown:

```rust
#[test]
fn test_incomplete_bold() {
    assert_eq!(remend("This is **bold"), "This is **bold**");
    assert_eq!(remend("**bold** and **more"), "**bold** and **more**"); // Complete, no change
}

#[test]
fn test_incomplete_italic() {
    assert_eq!(remend("This is *italic"), "This is *italic*");
}

#[test]
fn test_code_block_preservation() {
    // Should NOT complete - it's inside a code block
    assert_eq!(remend("```\n**bold\n"), "```\n**bold\n");
}

#[test]
fn test_escaped_markers() {
    assert_eq!(remend(r"This is \**not bold"), r"This is \**not bold");
}

#[test]
fn test_streaming_chunks() {
    // Simulate real streaming
    let chunks = ["Here is", " a **bold", " statement**"];
    assert_eq!(remend(chunks[0]), "Here is");
    assert_eq!(remend(&chunks[..2].join("")), "Here is a **bold**");
    assert_eq!(remend(&chunks.join("")), "Here is a **bold statement**");
}
```

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| False positives (completing markers that shouldn't be) | Extensive context checks, conservative matching |
| Performance overhead on each tick | Tail-focused regex, early exits, ASCII fast paths |
| Edge cases in nested formatting | Follow remend's battle-tested patterns |

## Success Criteria

1. Streaming `**bold` shows bold text (no visible `**`)
2. Streaming `*italic` shows italic text (no visible `*`)
3. Streaming `` `code `` shows code styling (no visible backtick)
4. Streaming `~~strike` shows strikethrough
5. Code blocks preserve literal markers
6. No performance regression (maintain 60fps)
