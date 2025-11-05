# Markdown Parsing Approach (iOS Message Renderer)

## Goals
- Render user/assistant messages with predictable, readable list formatting.
- Keep implementation lightweight: no heavy Markdown engine; rely on SwiftUI + small parser.
- Preserve inline emphasis where possible via `AttributedString(markdown:)` while we control block layout.
- Be resilient to imperfect Markdown from LLMs (mixed bullets, inconsistent spacing, tabs).

## High‑Level Strategy
- Treat messages as a sequence of paragraphs split by blank lines.
- For each paragraph, detect whether it is a list block (all lines are list items) or a free paragraph.
- Parse list items into a compact model (items with `level`, `kind`, `marker`, and `content`).
- Normalize indentation relative to the minimum indent of the block to keep nesting visually stable.
- Render items using SwiftUI:
  - Ordered items show their numeric marker (e.g., `"1."`, `"2)"`).
  - Unordered items show a small dot.
  - Indentation = `level * 14pt`.

## Data Model (AcpThreadView)
- `MDBulletKind`:
  - `none` — not a list item
  - `unordered` — hyphen/asterisk/dot bullet
  - `ordered(number: Int, separator: Character)` — numbered list
- `MDItem` (Identifiable):
  - `level: Int` — computed nesting level after baseline normalization
  - `bullet: Bool` — item vs paragraph
  - `kind: MDBulletKind` — ordered/unordered/none
  - `marker: String` — visible marker for ordered lists (e.g., `"1."`)
  - `content: String` — text content (rendered via `AttributedString` inline parser)

## Parsing Pipeline
1. Normalize line endings to `\n`.
2. Split into paragraphs on double newlines.
3. For each paragraph:
   - Call `isBulletBlock(para)` — returns true only if every line is a detectable bullet.
   - If list block:
     - For each line, run `bulletInfo(line)` to get `(indent, kind, marker, content)`.
     - Compute `minIndent` across the block; `level = max(0, (indent - minIndent) / 2)` (2 spaces ≈ one level; tabs count as 2 spaces).
     - Special case: When a hyphen/asterisk bullet block immediately follows a numbered block, add one base level to nest sub‑bullets under the last number.
     - Emit `MDItem` for each line with appropriate `kind/marker`.
   - Else:
     - Emit a paragraph `MDItem` (bullet = false).

## Bullet Detection (`bulletInfo`)
- Leading whitespace: spaces count 1, tabs count 2 (configurable).
- Unordered bullets: detect `-`, `*`, `•`, `–` at start of trimmed line, followed by optional spaces/tabs, then content.
- Ordered bullets: one or more digits followed by `.` or `)`, optional spaces/tabs, then content.
- Returns:
  - `isBullet: Bool`
  - `indent: Int` (leading whitespace → nesting candidate)
  - `kind: MDBulletKind`
  - `marker: String` (e.g., `"1."`)
  - `content: String`

## Rendering
- Unordered item: small filled circle + content, `padding(.leading, level*14)`.
- Ordered item: `Text(marker)` (semibold) + content, `padding(.leading, level*14)`.
- Paragraph: content with the same indent scheme (for now, paragraphs after bullets are rendered at the block’s base level; future work will attach them to the active list item).
- Colors: user messages use a slightly lighter gray; assistant uses primary text color. Fonts from `OAFonts.ui`.
- User messages are truncated at ~5 lines for compactness.

## Known Limitations / Next Steps
- Paragraphs under a list item (continuation lines) are not yet attached to the list item; they render as separate paragraphs. Plan: treat non‑bullet lines that immediately follow bullets with greater indent as children of the last bullet.
- Lettered lists (`a.`, `A)`) and roman numerals (`i.`, `iv.`) are not currently recognized.
- Code fences (```), blockquotes (`>`), and tables are not parsed as blocks. Inline emphasis is handled by `AttributedString` with `inlineOnlyPreservingWhitespace`.
- Ordered numbering is not validated or auto‑incremented; we render whatever marker appears in the text.
- Mixed indent width: current rule assumes 2 spaces ≈ 1 level; adjust in code if your content uses a different convention.

## Tuning Knobs (where to edit)
- Indent points per level: `renderMessageMarkdown` padding multiplier `14` (AcpThreadView).
- Spaces per level: `(indent - minIndent) / 2` (in `parseMarkdownItems`).
- Tab width: treated as 2 spaces inside `bulletInfo`.
- Unordered symbols set: `-`, `*`, `•`, `–`.

## Testing Plan
- Unit tests (ios/OpenAgentsTests):
  - `parseMarkdownItems` → assert sequences of `MDItem` for:
    - Mixed ordered then unordered sub‑bullets (the “What I changed/learned” patterns).
    - Multi‑digit numbering (`10.`), `2)` separator, mixed whitespace/tabs.
    - Non‑bullet paragraphs between list blocks.
  - Duration formatting: `formatDuration(124) == "2m 4s"`, `3600 -> "1h"`, etc.
- Snapshot tests (optional) for complex blocks once UI is stable.

## Troubleshooting
- “Bullets disappeared”: likely lines don’t meet bullet detection; confirm markers and leading whitespace. Printing the parsed `MDItem` list will quickly show what the parser saw.
- “Indent looks off”: adjust tab width or per‑level divisor; verify source uses consistent leading spaces.
- “Only a few messages show”: raise `maxMessages` and ensure the initial hydrate uses the same cap (done: current default is 400).

## Why not a full Markdown engine?
- We need tight control over bullets, nesting, and spacing to match design and “liquid glass” styling, and we want to keep dependencies minimal. This tailored parser gives predictable output with good performance and is easy to extend for the patterns we actually see in chats.

## Future Enhancements
- Attach continuation paragraphs to the preceding list item.
- Add support for lettered/roman lists.
- Code block detection → render with `OAFonts.mono` and glass card styling.
- Blockquotes with left rule and softer foreground.
- Markdown‑aware truncation that keeps list structure when clamping user messages.

