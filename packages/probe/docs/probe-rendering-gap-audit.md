# Probe Rendering Gap Audit

**Date**: 2026-06-08
**Updated**: 2026-06-08 (opentui-based integration path added)
**References**:
- `projects/repos/opencode/` — Opencode's TUI rendering architecture
- `projects/repos/opentui/` — `@opentui/core` v0.3.4 native Zig terminal engine

---

## Current State

Probe's entire terminal output pipeline lives in
`packages/runtime/src/cli.ts` (~1741 lines). Rendering is hand-rolled ANSI
escape sequences and a custom `marked.Renderer` subclass. There is no TUI
framework, no syntax highlighting, no proper diff rendering, no session
transcript view, and no component tree.

The current markdown renderer (`renderMarkdown`, lines 847-872) handles bold,
italic, inline code, links, blockquotes, headings, and lists with crude ANSI
wrapping. Code blocks are rendered in plain gray with no syntax highlighting.
Inline markdown streaming (`formatInlineMarkdown`, lines 874-893) uses regex
replacement rather than a parser.

The diff preview (`createDiffPreview` in `file-mutation.ts`, lines 53-70) is a
manual line-by-line comparison with `+`/`-` prefixes, truncated to 10 lines
each. Permission prompts filter these lines further to only `+`/`-` prefixed
lines, with no color.

Dependencies are minimal: `effect` and `marked` only. The package already
uses Bun (see `package.json` `"probe": "bun src/cli.ts"`).

---

## Opencode / Opentui Approach (Reference)

Opencode's TUI is built on `@opentui/core` — a **native Zig terminal engine**
with TypeScript bindings. The core exposes a C ABI, manages the render loop,
terminal I/O, input parsing, and composable layout via Yoga (flexbox).

| Surface | Technology | Key Features |
|---|---|---|
| TUI | `@opentui/core` + `@opentui/solid` | 23 renderable types; tree-sitter WASM highlighting; split/unified diff; scrollback; input handling; animation |
| Web App | SolidJS + `@pierre/diffs` + Shiki | Markdown + shiki; interactive review UI; Ghostty terminal |
| Web Share | Astro/SolidJS + Shiki | `codeToHtml` with GitHub themes; side-by-side diff |

### Renderable Hierarchy (23 types)

All renderables live in `packages/core/src/renderables/` and extend
`Renderable` (which uses Yoga for flexbox layout):

- **`CodeRenderable`** — Syntax-highlighted code via tree-sitter WASM.
  Accepts `content`, `filetype`, `syntaxStyle`, `streaming`, `conceal`,
  `onHighlight`/`onChunks` callbacks. Falls back to plain text if tree-sitter
  parser is unavailable.
- **`DiffRenderable`** — Unified or split diff view. Built on top of
  `CodeRenderable` + `LineNumberRenderable`. Uses `parsePatch` from `diff`
  npm package. Colors driven by `addedBg`, `removedBg`, `addedSignColor`,
  `removedSignColor`, etc. Sync-scrolls in split mode.
- **`MarkdownRenderable`** — Full markdown rendering via `marked` lexer.
  Parses into blocks: paragraphs (via `CodeRenderable`), code fences (via
  `CodeRenderable` with language detection), tables (via `TextTableRenderable`),
  blockquotes (via `BoxRenderable`), lists (via `BoxRenderable`), horizontal
  rules. Supports streaming with incremental block re-rendering, conceal mode
  (hides `**`, `` ` ``, `[`, etc.), and `internalBlockMode: "top-level"` for
  assistant-style prose.
- **`LineNumberRenderable`** — Gutter with line numbers, `lineSigns` (+/-),
  per-line colors, configurable `minWidth`. Composes with any
  `LineInfoProvider` (like `CodeRenderable`).
- **`TextRenderable`** — Plain text with foreground/background color, bold,
  italic, underline, dim.
- **`ScrollBoxRenderable`** — Scrollable container with viewport culling,
  sticky scroll, scroll acceleration.
- **`BoxRenderable`** — Bordered container with titles, background color,
  padding, flexbox layout.
- **`TextareaRenderable`** — Multi-line text input with cursor, placeholder,
  wrapping, submit handler.
- **`InputRenderable`** — Single-line text input.
- **`TextTableRenderable`** — Tabular data with column width modes, borders,
  cell padding.

### API Surface (Imperative, No JSX Required)

The imperative API (shown in `packages/examples/src/code-demo.ts`,
`markdown-demo.ts`, `split-mode-demo.ts`) is the pattern probe would follow:

```typescript
import {
  createCliRenderer, CliRenderer, SyntaxStyle, parseColor,
  CodeRenderable, DiffRenderable, MarkdownRenderable,
  BoxRenderable, TextRenderable, ScrollBoxRenderable,
  LineNumberRenderable, TextareaRenderable,
} from "@opentui/core"

// Bootstrap
const renderer = await createCliRenderer({
  exitOnCtrlC: true,
  targetFps: 60,
  screenMode: "fullscreen",      // or "main-screen" / "split-footer"
})

// Create renderables imperatively
const syntaxStyle = SyntaxStyle.fromStyles({
  keyword:   { fg: parseColor("#FF7B72"), bold: true },
  string:    { fg: parseColor("#A5D6FF") },
  comment:   { fg: parseColor("#8B949E"), italic: true },
  // ...
})

const code = new CodeRenderable(renderer, {
  content: srcCode,
  filetype: "typescript",
  syntaxStyle,
  conceal: true,
  streaming: true,
  width: "100%",
})

// Wrap in line numbers
const withLines = new LineNumberRenderable(renderer, {
  target: code,
  minWidth: 3,
  paddingRight: 1,
  width: "100%",
})

// Place in a scrollable box
const scrollBox = new ScrollBoxRenderable(renderer, {
  border: true,
  scrollY: true,
  flexGrow: 1,
  width: "100%",
})
scrollBox.add(withLines)

// Mount to root
renderer.root.add(scrollBox)
renderer.start()

// Update content reactively
code.content = newSrcCode
code.filetype = "rust"
```

For diffs:

```typescript
const diff = new DiffRenderable(renderer, {
  diff: patchText,
  filetype: "typescript",
  syntaxStyle,
  view: "unified",          // or "split"
  showLineNumbers: true,
  addedBg: "#1a4d1a",
  removedBg: "#4d1a1a",
  addedSignColor: "#22c55e",
  removedSignColor: "#ef4444",
  width: "100%",
})

// Switch views at runtime
diff.view = "split"
diff.diff = newPatchText
```

For markdown:

```typescript
const md = new MarkdownRenderable(renderer, {
  content: markdownText,
  syntaxStyle,
  conceal: true,
  internalBlockMode: "top-level",
  streaming: true,
  width: "100%",
})

// Stream content incrementally
md.content = fullMarkdownText
```

### Screen Modes

Opentui supports three screen modes via `renderer.screenMode`:

- **`"alternate-screen"`** (default) — Full-screen TUI on alternate screen
- **`"main-screen"`** — Inline mode, renders in-place (probe's current
  behavior, but with TUI layout)
- **`"split-footer"`** — Scrollback region + fixed footer for composer/input

For a coding agent, **`"main-screen"`** mode preserves probe's existing inline
output behavior while adding rich rendering. **`"split-footer"`** gives a
chat-like UX with persistent input area.

### Key Dependencies

- Zig must be installed for the native binary (resolved at `npm install` time)
- Platform-specific packages: `@opentui/core-darwin-arm64`, etc.
- Tree-sitter WASM parsers loaded at runtime (probe would register its own)
- `diff` npm package (already a transitive dependency via `marked`)

---

## Gap Analysis

### Gap 1: No Syntax Highlighting

**Severity**: High
**Current**: All code blocks are plain gray. The `_language` parameter to
`marked.Renderer.codes()` is ignored.
**Impact**: Code in tool results, assistant responses, and permission diffs is
unreadable. The most important signal for a coding agent — the code itself —
has zero visual structure.
**Opentui**: `CodeRenderable` with tree-sitter WASM parsers, language
detection from filetype, `SyntaxStyle` for theme-driven coloring.
**Opentui Path**: `new CodeRenderable(renderer, { content, filetype, syntaxStyle })`.
Adds tree-sitter WASM parsers for 30+ languages. Falls back to plain text.

---

### Gap 2: No Proper Diff Rendering

**Severity**: High
**Current**: `createDiffPreview()` compares lines positionally (no LCS/Myers
diff algorithm), shows at most 10 old + 10 new lines, truncates at 200 chars,
and renders with plain `+`/`-` prefixes (no color). Permission prompts filter
to only `+`/`-` lines, losing context.
**Impact**: Users cannot see what changed in a file edit. The diff shown for
permission approval is essentially useless for anything but the simplest
single-line changes.
**Opentui**: `DiffRenderable` with `parsePatch`, unified/split views, hunk
headers, colored backgrounds, gutter signs, line numbers, sync-scroll.
**Opentui Path**: `new DiffRenderable(renderer, { diff: patchText, view: "unified", ... })`.
Replace `createDiffPreview()` with `parsePatch` + `DiffRenderable`.

---

### Gap 3: No TUI Framework

**Severity**: Medium
**Current**: Flat `process.stdout.write()` with hand-rolled ANSI codes. No
layout system, no scrolling, no paging, no viewport management.
**Impact**: Long outputs are unreadable (scroll off screen). There is no way
to interact with output (scroll back, search, expand/collapse). Session
history is lost between turns.
**Opentui**: Full Yoga-based layout engine, viewport culling, scroll
containers, event loop, input handling, screen mode management.
**Opentui Path**: `await createCliRenderer({ screenMode: "main-screen" })`.
Probe keeps inline behavior but gains layout, scrolling, and input handling
from opentui.

---

### Gap 4: No Session Transcript Display

**Severity**: Medium
**Current**: The interactive chat loop accumulates messages in a flat array
and re-sends them on each turn. There is no command to view a previous
session, no scroll-back beyond terminal history, no rendered transcript.
**Impact**: Users cannot review what happened in a previous turn or session.
Debugging is limited to what fits on screen.
**Opentui**: `MarkdownRenderable` + `ScrollBoxRenderable` for transcript view.
Renders each message distinctly: assistant text via `MarkdownRenderable`,
reasoning via `CodeRenderable`, file diffs via `DiffRenderable`, tool output
via `CodeRenderable`.
**Opentui Path**: Build a scrollable transcript with opentui renderables.
Persist sessions to disk for `probe session show <id>` replay.

---

### Gap 5: Tool Output Is Invisible

**Severity**: Medium
**Current**: `formatToolResultValue()` returns one-line summaries like
`"{path} ({content.length} chars)"`. The actual content read from files or
returned by tools is sent to the LLM but hidden from the user.
**Impact**: Users cannot see what the agent read, what search results it got,
or what command output it received. This makes debugging agent behavior
nearly impossible.
**Opentui**: Tool outputs rendered with `CodeRenderable` and line numbers,
file changes via `DiffRenderable`, file trees via `BoxRenderable`.
**Opentui Path**: Add a configurable `verbose` mode. Render `read_file`
results in `CodeRenderable` with detected filetype, `search_code` results
with context lines, shell command output in `ScrollBoxRenderable`.

---

### Gap 6: No ANSI Passthrough / Terminal Emulation

**Severity**: Low
**Current**: ANSI escape sequences are unconditionally stripped from shell
tool output before display.
**Impact**: Colored output from tools (e.g., `npm test` with pass/fail colors,
`git diff` with its own coloring) is lost.
**Opentui**: Opentui owns the terminal; ANSI from spawned processes would need
explicit handling (either strip or render through a terminal emulator
renderable). The opentui codebase has `FrameBufferRenderable` for pixel
graphics but no built-in terminal emulator.
**Path**: Strip ANSI for now (as opencode's TUI does). Add a `--passthrough`
flag for simple cases. A full terminal emulator (Ghostty/hterm) would be a
separate project.

---

### Gap 7: Streaming Markdown Uses Regex, Not a Parser

**Severity**: Low
**Current**: `formatInlineMarkdown()` uses simple regex replacements for bold,
inline code, links, etc. It does not handle nested formatting, does not parse
code blocks, and does not handle incomplete streaming content correctly.
**Impact**: Streaming assistant text can show raw markdown syntax (especially
code fences that span multiple chunks).
**Opentui**: `MarkdownRenderable` uses the `marked` lexer with incremental
parsing. Handles incomplete fenced code blocks, heals incomplete links,
supports `streaming` mode with stable block tracking. Per-block
reconciliation avoids re-rendering unchanged blocks during streaming.
**Opentui Path**: Replace `renderMarkdown()` with `MarkdownRenderable`.
Replace `formatInlineMarkdown()` with `MarkdownRenderable` in streaming mode
with `streaming: true`.

---

## Opentui Integration Architecture

### Phase 1: Dependency + Bootstrap

**Install**:
```bash
bun add @opentui/core
# Zig must be on PATH for native build
```

**Bootstrap** (replace current CLI init):
```typescript
// packages/runtime/src/cli.ts
import { createCliRenderer, CliRenderer } from "@opentui/core"

const renderer = await createCliRenderer({
  screenMode: "main-screen",  // preserves inline behavior
  exitOnCtrlC: true,
  targetFps: 30,
})
```

**Architecture change**: The current `process.stdout.write()` calls are
replaced by adding renderables to `renderer.root`. The `renderMarkdown()`
function is replaced by `MarkdownRenderable`. The streaming loop writes to
`renderable.content` instead of `process.stdout.write()`.

### Phase 2: Assistant Message Rendering

Replace `renderMarkdown()` with `MarkdownRenderable`:

```typescript
const messageBlock = new ScrollBoxRenderable(renderer, {
  id: "message-scroll",
  scrollY: true,
  flexGrow: 1,
  width: "100%",
})
renderer.root.add(messageBlock)

const assistantText = new MarkdownRenderable(renderer, {
  content: "",
  syntaxStyle,
  conceal: true,
  internalBlockMode: "top-level",
  streaming: true,
  width: "100%",
})
messageBlock.add(assistantText)

// On each text-delta event:
assistantText.content = accumulatedText
```

### Phase 3: Diff Permissions

Replace `createDiffPreview()` + filtered permission prompt with
`DiffRenderable`:

```typescript
import { parsePatch } from "diff"

// Generate proper diff
function createDiffPreview(oldText: string, newText: string): string {
  // Use diff package to create unified patch
  // return patch string
}

// In permission handler, render via DiffRenderable
const diff = new DiffRenderable(renderer, {
  diff: patchText,
  filetype: fileExtension,
  syntaxStyle,
  view: "unified",
  showLineNumbers: true,
  addedBg: "#1a4d1a",
  removedBg: "#4d1a1a",
  width: "100%",
})
```

### Phase 4: Tool Output Visibility

```typescript
// For read_file results:
const codeBlock = new CodeRenderable(renderer, {
  content: fileContent,
  filetype: detectFiletype(filePath),
  syntaxStyle,
  width: "100%",
})
const withLines = new LineNumberRenderable(renderer, {
  target: codeBlock,
  minWidth: 3,
  paddingRight: 1,
  width: "100%",
})

// For search_code results:
// Use CodeRenderable with ripgrep output, or build a structured view
```

### Phase 5: Session Transcript

```typescript
const session = new ScrollBoxRenderable(renderer, {
  scrollY: true,
  flexGrow: 1,
  width: "100%",
})
renderer.root.add(session)

// For each turn, append renderables to session:
function appendMessage(text: string) {
  const msg = new MarkdownRenderable(renderer, {
    content: text,
    syntaxStyle,
    conceal: true,
    internalBlockMode: "top-level",
    width: "100%",
  })
  session.add(msg)
}

function appendToolResult(path: string, content: string) {
  const code = new CodeRenderable(renderer, {
    content,
    filetype: detectFiletype(path),
    syntaxStyle,
    width: "100%",
  })
  session.add(code)
}
```

---

## Implementation Priority (Opentui Path)

| Phase | Components | Effort | Priority |
|---|---|---|---|
| **P0** | Install `@opentui/core`, bootstrap renderer, replace `renderMarkdown` with `MarkdownRenderable`, replace `createDiffPreview` with `DiffRenderable` | Medium | **Immediate** |
| **P1** | Wire streaming markdown via `MarkdownRenderable.streaming`, add `CodeRenderable` for tool output with line numbers, add `SyntaxStyle` theme | Medium | **Next** |
| **P2** | Build session transcript view with `ScrollBoxRenderable`, add `probe session show` command for replaying persisted sessions | Medium | **Soon** |
| **P3** | Add `TextareaRenderable` for composer in split-footer mode, add ANSI handling for shell output, add theme switching | Low | **Later** |

### Syntax Style Configuration

Probe would define a default `SyntaxStyle` via `SyntaxStyle.fromStyles()`:

```typescript
import { SyntaxStyle, parseColor } from "@opentui/core"

export function createDefaultSyntaxStyle(): SyntaxStyle {
  return SyntaxStyle.fromStyles({
    keyword:   { fg: parseColor("#FF7B72"), bold: true },
    string:    { fg: parseColor("#A5D6FF") },
    comment:   { fg: parseColor("#8B949E"), italic: true },
    number:    { fg: parseColor("#79C0FF") },
    function:  { fg: parseColor("#D2A8FF") },
    type:      { fg: parseColor("#FFA657") },
    operator:  { fg: parseColor("#FF7B72") },
    variable:  { fg: parseColor("#E6EDF3") },
    property:  { fg: parseColor("#79C0FF") },
    bracket:   { fg: parseColor("#F0F6FC") },
    delimiter: { fg: parseColor("#C9D1D9") },
    // Markdown styles
    "markup.heading":   { fg: parseColor("#00D7FF"), bold: true },
    "markup.bold":      { fg: parseColor("#F0F6FC"), bold: true },
    "markup.italic":    { fg: parseColor("#F0F6FC"), italic: true },
    "markup.list":      { fg: parseColor("#FF7B72") },
    "markup.quote":     { fg: parseColor("#8B949E"), italic: true },
    "markup.raw":       { fg: parseColor("#A5D6FF"), bg: parseColor("#161B22") },
    "markup.link":      { fg: parseColor("#58A6FF"), underline: true },
    "markup.link.url":  { fg: parseColor("#58A6FF"), underline: true },
    conceal:            { fg: parseColor("#6E7681") },
    default:            { fg: parseColor("#E6EDF3") },
  })
}
```

### Filetype Detection

Opentui's `CodeRenderable` accepts a `filetype` string that maps to a
tree-sitter grammar. Probe should:

1. Map file extensions to filetypes (open code has `LANGUAGE_EXTENSIONS`)
2. Register tree-sitter WASM parsers at startup via
   `getTreeSitterClient().addFiletypeParser(...)` or rely on opentui's
   built-in parser registry
3. Fall back to plain text (no highlighting) when no parser is registered for
   a filetype — opentui's `CodeRenderable` handles this automatically

---

## Code Migration Strategy

The migration plan is **incremental** — each piece works independently:

1. Install `@opentui/core` and create the renderer (no visual change yet)
2. Replace `renderMarkdown()` — use `MarkdownRenderable` for assistant text
3. Replace `createDiffPreview()` — use `parsePatch` + `DiffRenderable`
4. Replace `formatInlineMarkdown()` — use `MarkdownRenderable.streaming = true`
5. Add `CodeRenderable` for tool results in verbose mode
6. Add `ScrollBoxRenderable` for session transcript persistence
7. Add `TextareaRenderable` for input composer (conditional, split-footer mode)

The existing `process.stdout.write()` path can coexist during migration by
rendering to a buffer/text renderable instead of stdout.
