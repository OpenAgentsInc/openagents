# View and adapter specification

Rust Native separates semantic presentation from application state and native
widget ownership. The implemented core validates and serializes views, and it
lays out transcripts for adapters (see [Transcript layout](#transcript-layout)).
It does not mount views or provide an application runtime.

## View and interaction contract

`View::new` uses schema `rust-native.view.v2`; `View::new_v3` explicitly opts into
`rust-native.view.v3`. Both carry a surface `instance`, a positive
`revision`, and a root `Node<I>`. Each node contains a stable `key`, a resolved
`Style`, and an element:

| Element | Meaning |
| --- | --- |
| `Stack` | Ordered children on a horizontal or vertical axis, or `wrap`: left to right, continuing on the next line when the next child does not fit. An adapter without `wrap` lays the children out vertically. |
| `List` | A labeled bounded window of stable rows. Paging and access to original data remain application responsibilities. |
| `Text` | Unicode text with a body, heading, code, status, Markdown, or terminal role. A Markdown role conveys selectable document meaning; links and embedded content remain inert unless separately admitted by the application. It does not itself parse or render Markdown. A terminal role is one row, or one run of a row, of a fixed-cell character grid: adapters draw it monospaced on a single line, never wrap it, and keep every space. The application sizes the grid from the cell size its adapter reports. |
| `RichText` (v3) | Selectable Unicode runs with generic foreground/background colors, bold, italic, underline, strikethrough, and dim state. Runs contain literal text, never HTML or links. Its TextRole distinguishes fixed-cell rows from wrapping text. |
| `Field` (v3) | A labeled, bounded editable value with placeholder, enabled, multiline, and secret state. A `FieldChange` carries activation identity and a value; `change_field` resolves the current `on_change` intent after identity, enabled, line, and byte checks. Secret fields carry an empty initial value and cannot be multiline; the adapter and owning controller retain private input. Debug output redacts field changes. |
| `Choice` (v3) | An enabled selectable action with an accessible label, explicit selected state, and optional presentation children. Children cannot contain interactive controls. Selection remains application state. |
| `Dialog` (v3) | A labeled modal focus scope with explicit open state and children. Dismissal resolves its current `on_close` intent. A closed scope refuses descendant events. The adapter handles focus containment and restores originating focus. |
| `Button` | A nonempty label, an enabled state, the application's typed intent, and an optional `icon`: a `glyph` from a closed set (`back`, `compose`, `edit`, `search`, `settings`, `pin`, `archive`, `restore`, `menu`, `history`, `folder`, `computer`, `cloud`, `add`, `arrow_up`, `stop`, `paperclip`, `clipboard`, `more`, `check`, `ask`, `flag`, `terminal`, `wallet`, `key`, `person`, `unchecked`, `checked`), `circular`, and `pill`. `unchecked` and `checked` make the button a checkbox: the box leads the label, which says what the setting allows, and activating it asks the application to flip the setting. A circular icon draws only the glyph in a circle, and the label becomes its spoken name; otherwise the glyph leads the visible label, and with `pill` both sit in a filled capsule, as a suggestion chip or a selector. An adapter that can't draw the glyph shows the label. An optional `shortcut` is a bounded display hint; it grants no activation or key binding. |
| `Surface` | A nonempty accessibility label and an opaque local resource ID. An adapter explicitly registers the renderer; the tree cannot name a URL, library, executable, or shader to load. |
| `Transcript` | A conversation, oldest row first, with a nonempty label and an optional `earlier` control (label, `loading`, intent). Adapters keep the newest row in view while the reader is at the bottom, stop following when the reader scrolls up, and then offer a jump to the bottom. Activating the transcript node runs `earlier` unless it is loading. An optional `source` names a transcript source that holds the rows instead of `children`, which must then be empty (see [Transcript layout](#transcript-layout)). |
| `Message` | One message with a `role` (`user`, `assistant`, or `system`), an optional short `note`, and children. Adapters draw a user message as a trailing bubble, an assistant message full width, and a system message as a quiet row. |
| `Markdown` | Blocks the application parsed (`rust_native::markdown`): headings, paragraphs, lists with task states, fenced code with its language, quotes, tables with alignment, and rules; inline spans carry bold, italic, strikethrough, code, and a link destination. Bare `https` URLs and `openagents.com` paths in text become links, and a site path such as `/projects` points at `https://openagents.com`. A reply's ```` ```openui-lang ```` component blocks are parsed as their Markdown fallback: links, numbered steps, and each command a code block. Adapters lay out the blocks; they never parse Markdown or load images, and they open only the links `markdown::opens` admits (`https` URLs), in the system browser. For a streaming reply, `IncrementalMarkdown` reparses only the last top-level block, reports how many leading blocks stayed unchanged, and offers a display copy whose tail closes half-written emphasis, code, and links (a streaming link has an empty destination, and a bare URL at the very end stays text until something follows it). Its canonical blocks always equal a full parse. |
| `Tool` | A tool call: a nonempty `name`, a one-line `detail`, a `state` (`running`, `done`, or `failed`), and children the reader can expand. Expansion is adapter state. |
| `Working` | A nonempty label that says the assistant is working. |
| `Composer` | A text field with a send control: an input `token`, a placeholder, a byte bound, `enabled`, `busy`, an optional `stop` intent, up to four `choices`, and an optional `draft`. A send is an input answer bound to `token` that the application checks with `ValidatedView::accept_composer`; while `busy`, the send control becomes stop, and activating the composer node runs `stop`. Each choice is `{token, label}`: another way to send the same text, which the adapter offers on a long press of the send control (a menu) and answers with the choice's own token, distinct from every other token; the application gives each token its meaning. A `draft` is text the adapter puts in the field when `token` is new, such as a message to edit. With `focus`, the adapter puts the text cursor in the field once when `token` is new and the composer is enabled, so a screen made for writing opens ready to type. |

`ValidatedView<I>` exposes an immutable tree and its checked serialization and
activation paths. Unknown core fields and variants are rejected. The
application supplies a closed serializable intent type and validates domain
identifiers within it. Compiled application serialization code is trusted host
code, not sandboxed plugin code.

| Bound | Limit |
| --- | --- |
| Surface, node, and style identifiers | 1–96 ASCII bytes: letters, digits, `_`, `-`, `.`, and `:` |
| Encoded view | 512 KiB, including escaping and intent data |
| Nodes | 1,024 |
| Node depth | 16, counting the root as one |
| JSON nesting | 96 containers, including intent payloads |
| Text or control label | 64 KiB in UTF-8 bytes |

Input byte and nesting limits apply before decoding. Constructed trees also
pass structural and encoded limits. These bounds apply to one view, not an
application's backing data store. Use bounded windows for long content while
retaining access to every original record. Both schemas refuse earlier versions
rather than silently converting them. V2 also refuses the four added v3
elements. Existing v2 constructors and serialized views retain their schema and
meaning; adapters must explicitly declare v3 support before mounting the new
elements. The transcript display list's row admission remains v2 and does not
establish v3 native rendering.

Buttons inside transcript rows produce a native `button` widget with the
semantic node key and enabled state. Their labels and bounds come from the
shared row layout. A transcript button fills the reading band, except a
`pill` button or one with `style.intrinsic_width`, which is as wide as its
label (wrapping inside the band when the label is longer); a `pill` button
is drawn as a capsule. An adapter returns the key only after a matching pointer
release; the application resolves it against its current action registry and
checks authority. Replacing a row cancels a press in progress. A display-list
button carries no executable command or application intent. Transcript stack
spacing maps `none`, `xs`, `sm`, `md`, and `lg` to 0, 4, 8, 16, and 24 points.
Explicit stack padding and background frame a card inside the reading band.

Transcript display lists can identify code paragraph fields in `code_blocks`.
The shared `syntax::Cache` returns bounded foreground spans asynchronously;
adapters apply them to shaped clusters without changing fonts, wrapping, or
row geometry. Unknown languages and oversized inputs remain plain. Native
adapters opt in to the worker and provide their own wake callback. Phone
adapters call `syntax::highlight_utf16` (`rust_native_syntax_spans`, or JNI
`TranscriptNative.highlight`) from their own worker for spans in UTF-16 code
units, in the dark palette or `Palette::light`, and repaint a row when its
spans arrive.

A stack whose style sets `menu: context` is a card with a context menu: its
first child is the card, and every other child is a button the menu offers.
Phone adapters present the items natively (`UIMenu` from a long press on
iOS, `PopupMenu` on Android); an adapter without menus lays the children out
on the stack's axis. Items activate through the view like any button. A
native menu can stay open while the application publishes newer views, so
the adapter resolves a chosen item's key against the newest view it has;
applications key items by their subject and action (as `talk-…-pin`), so a
key never changes meaning, and an item that no longer applies is missing and
refuses. Validation requires a stack with the card and at least one button
to offer.

An attached image is a `Surface` whose resource is `image:{id}` and whose
label is its alternative text. The application owns the decoded, bounded
bytes (`openagents-chat-app::attachments`); the phone adapter asks the
application for them by resource and draws them with the platform decoder
(`UIImage`, `Bitmap`). The desktop paints the same node with
`rust_native_desktop::image`.

`selection::Selection` stores stable row keys and grapheme-safe byte positions
in display paragraph fields. Prepending and appending rows preserve endpoints;
changing an endpoint's prefix or removing its row cancels them. Copy preserves
selected bytes and adds a newline between fields and rows, including offscreen
rows. Platform hit-testing supplies positions; the shared core owns selection
validation, ordering, and copying.

## Revision and lifetime

The application allocates a fresh instance when it mounts a new surface
lifetime and increases its revision for each new immutable view. It must never
reuse one revision for different contents. The core validates individual views;
it does not maintain a global revision ledger.

An `Activation` carries `instance`, `revision`, and `node`. Resolving it against
the current validated view rejects stale instance or revision values, missing
or noninteractive nodes, and disabled buttons. It returns the intent already
stored in that view, never a caller-supplied replacement action.

Resolution is not authentication, proof of a physical gesture, authorization,
or idempotency. The application checks the event source, current domain state,
and permission before performing any effect. Repeated activations can resolve
the same intent; deduplication belongs to the application's operation contract.
A concurrent refresh can cause a visible stale-view refusal.

## Adapter responsibilities

Native adapters and their mounting protocol remain separate implementation
work. Each adapter must specify its supported elements and style properties,
faithful fallbacks, and refusals. Validation of a tree does not prove that a
particular adapter supports it.

The intended boundary is:

1. Application state produces a validated immutable view.
2. The adapter checks support, prepares changes, and applies them on the
   platform's UI thread.
3. The adapter acknowledges the revision it actually applied. Receiving a
   revision is distinct from displaying it.
4. Native callbacks name the instance, applied revision, and stable node key.
5. The application resolves the intent and checks current authority.

Reconciliation should preserve native control identity, focus, scroll position,
and selection when keys remain stable. Detached or recycled controls must lose
their old callbacks and private contents. A late mount must not replace a newer
one. Disposal releases native resources and presentation subscriptions; it does
not implicitly cancel durable application work.

Use native controls for selection, scrolling, system menus, input composition,
and accessibility. A text editor requires an additional explicit contract for
edit acknowledgments, selection units, marked text, composition ownership, and
stale updates. Those semantics are not implemented by serializing a string.

The shared core owns neither network access, persistence, a palette, clock sources,
credentials, nor task execution. Platform objects belong to adapters;
application-specific components and effects belong to their application.

## Local text editing

`edit::Editor` implements bounded local editing for adapters. It reimplements
Zeron's composer editing design with directional selection, grapheme and word
movement, logical line movement, multiline replacement, and bounded undo/redo.
Selections and replacement ranges use UTF-8 byte offsets on character
boundaries. Keyboard movement and deletion at a caret preserve whole graphemes.
The UTF-16 conversion helpers reject invalid offsets, including positions
inside surrogate pairs.

IME preedit owns a marked range and retains the text and selection from before
composition. Updates replace that range; commit records the composition as one
undo step; cancellation restores the earlier state. A failed edit changes no
text, selection, history, or revision. The caller supplies monotonic elapsed
milliseconds for undo coalescing; the editor owns no clock. Text is bounded by
the input's byte limit, and each undo and redo history is bounded at 128 steps
and 512 KiB of retained text. This is an in-process API, not a new view schema.

`rust_native_desktop::composer::ComposerDraft` binds this editor to a validated
`Composer` node. Rerenders with the same instance, node, and input token preserve
local editing state and ignore the view's initial draft. Input callbacks name
the applied view revision, an editing lifetime, and the editor's revision.
A new token resets the editor; disposal retires the lifetime. Submissions check
the current composer, its enabled/busy state, its own choice tokens, composition,
and byte bound. An accepted submission clears only its exact editing sequence,
so it never discards text typed afterwards. Domain admission and message
idempotency remain application responsibilities.

The desktop adapter connects these APIs to native key and IME events,
clipboard access, caret and selection painting, and visual line movement.
Its reusable `Field` takes an application-provided placeholder. Search and
title fields use the same editor with separate editing lifetimes. Platform
accessibility remains separate work under #10003.

### Phone fields

`edit::mirror::Mirror` lets a platform text field (`UITextView`,
`EditText`) edit through the same editor while it keeps drawing, the caret,
dictation, and the system keyboard. The field reports each change as its
whole text, selection, and marked (IME) range in UTF-16 code units; the
mirror turns the change into one editor operation (a replacement, a preedit,
or a commit) and returns the canonical state, which the field shows. A
deletion that would split a grapheme removes the whole grapheme, whatever the
keyboard sent; the delete key, undo, and redo go to the editor directly.
Every call names a stamp (the composer's input token, the mirror's editing
lifetime, and the editor revision); any other stamp is stale, changes
nothing, and the refusal returns the current state without echoing the
change. A new token starts a new lifetime with the view's `draft`, a
rerender with the same token keeps the draft and its history, and a changed
byte bound without a new token refuses. `submitted` clears only the exact
draft a send carried. Adapters reach it through `rust_native_editor_*` in
`include/rust_native_layout.h` or the Android JNI's `EditorNative`, with one
JSON request per call; the editor is fast enough for the UI thread.

## Transcript layout

The `layout` module lays out a `Transcript`'s rows on the adapter's behalf.
Applications still emit the semantic elements above; layout is part of how an
adapter presents them, not a separate wire contract.

- **Input.** An `Update` carries the viewport width (1–16,384 points), a text
  scale (0.5–4), an optional text-size curve, the row order (or none, to keep the previous order), the rows
  that are new or changed as intent-free `Node<()>` values, the expanded tool
  keys, and the `earlier` control's label and loading state. Each row passes
  the same checks as a one-node view. A layout holds at most 20,000 rows.
- **Transcript sources.** An application that holds its rows in Rust can keep
  them out of the view. `layout::source::detach` moves each transcript's rows
  into a named source (`{scope}:{key}`) in the same process and leaves the
  node with its label, its `earlier` control, and `source`. Each row still
  passes the one-node view checks, but the transcript's total no longer
  counts against the view's 512 KiB and 1,024-node bounds. The adapter's
  update then names the source instead of carrying `order`, `rows`, and
  `earlier`, and Rust reads the newest publication and lays out only the rows
  whose content changed. The adapter decodes and encodes no rows. A
  republished row that is unchanged keeps its hash without another
  validation. A detach retires the scope's sources that its view no longer
  shows. A process holds at most 64 sources. Adapters whose rows arrive as
  JSON can publish a transcript node with `rust_native_source_publish`.
- **Text sizes.** The curve lists at most 16 `[nominal, scaled]` points, such
  as Dynamic Type's size for each text style, each 0.5–4 times its nominal
  size. A size between two points interpolates, and a size outside them keeps
  the nearest point's ratio. Without a curve, every size scales by `scale`.
- **Measurement.** Text goes through the adapter's `Measurer`, which breaks a
  styled paragraph into lines and reports each line's UTF-16 range, width,
  ascent, descent, and leading, plus the x offset of each style boundary. The
  iOS adapter implements it with CoreText. Results are cached by paragraph
  text, fonts, and width, so a row laid out again at the same width, or a
  display list for a laid-out row, does not measure again.
- **Shaping in Rust.** With the `shaping` feature, `layout::shape` measures
  without a platform callback. It shapes with one bundled variable face
  under the SIL Open Font License: Paper Mono (`crates/paper-mono`), the
  typeface every surface uses. `FontSpec` names the face and variations for
  each display-list font (`wght` 400–700, and `calt` off for code), and the
  adapter paints with exactly those. Paper Mono has no italic, so italic
  text is drawn upright. Lines break greedily at UAX #14
  opportunities with CoreText's tailoring, trailing spaces hang, and a word
  wider than the line breaks between grapheme clusters. Tabs advance to
  28-point stops. A character the faces lack is measured as a platform
  fallback roughly draws it (1 em for wide characters and emoji, else
  0.6 em), so such text can wrap differently from the platform. A
  ground-truth test compares the breaks with CoreText's for the same fonts
  over about 3,400 paragraphs (`fixtures/coretext-lines.json`, made by
  `tools/coretext-lines.swift`): it allows at most one line-count difference
  among paragraphs Paper Mono covers and requires at least 99.7% exact line
  starts. The C interface adds
  `rust_native_layout_create_shaped`, `rust_native_font_spec`, and
  `rust_native_font_data`.
- **Frame.** Every row has a key, a content version, an exact height, and a
  cumulative offset. `rows_in(y0, y1)` is a binary search. A row is laid out
  again only when its content, the width, the text sizes, or its expansion
  changes, so a streamed token lays out one row. Each update publishes an
  immutable `Frame` of keys, versions, offsets, and display lists. A frame can
  be read on any thread while the next update runs on another, so an adapter
  lays out off its UI thread and swaps frames when one is ready.
- **Display lists.** `display(i)` returns what to paint: paragraph texts,
  styles (font size, weight, italic, monospace, a palette role or explicit
  color, opacity, underline, and strikethrough), text runs with UTF-8 and
  UTF-16 ranges, `x`, and baseline, rounded rectangles for bubbles, code
  blocks, quotes, tables, rules, and inline code, link rectangles, and
  native widgets (copy, disclosure toggle and chevron, tool state, checkbox,
  working indicator, spinner, the earlier control, a `link` tap target over
  each link that opens, outside sideways scrollers, and a `surface` box: a
  `Surface` row with a `min_height` reserves that height, at most 360 points
  wide and 480 tall, for the adapter to draw the resource in, such as a
  link's preview card; without one it shows its label). Code blocks keep their
  lines, and a code block or table wider than the row becomes a sideways
  scroller: a clip rectangle, a content width, and the ranges of runs,
  rectangles, and links that scroll inside it. Lines longer than 8,192 points
  are truncated, and table columns wrap past 260 points at the default text
  size. It also carries the
  row's accessibility label, value, hint, and button trait, and the plain text
  a Copy action uses.

The adapter paints runs at the given positions with the same fonts it
measured with, supplies the widgets, scrolls, and keeps each row an
accessibility element built from the display list. It keeps these behaviors:
follow the newest row while the reader is at the bottom; stop following when
the reader drags; resume when a scroll comes to rest, or momentum carries the
list, within 70 points of the bottom; offer a jump to the bottom; and keep the
first visible row still on screen when rows are prepended or change above it.
Tool expansion stays adapter state that the adapter passes to each update.
Painted text can be selected in place: the adapter hit-tests its runs, draws
the highlight and handles, and copies the selected text. It may fade in the
new text of a row that changed without a geometry change, such as a streamed
reply.

The C interface (`include/rust_native_layout.h`, the `ffi` feature) exposes
one handle per transcript: create with a measurer callback, update with JSON,
take the current frame, and, on the handle or a frame, query placements into a
caller array, find a row by key, and fetch a row's key or display list as
JSON. Calls on one handle must not overlap but may come from any thread; a
frame is reference-counted, outlives later updates, and is released once.
Calls catch panics and bound their input; the measurer runs only during
update, on the updating thread.

## Input requests

A view can't collect text yet. `input::InputRequest<P>` asks the adapter for
one value beside the view: a `token` (a bounded identifier), the
application's closed `purpose` type `P`, a nonempty accessible `label`, a
one-sentence `prompt`, `scan` to open a scanner first, `secret`, and
`max_bytes` (1 byte to 64 KiB). Unknown fields are rejected, and `secret` is
required. `validate` checks the structure. `accept` checks that an answer
names the current token and fits `max_bytes`; its errors never contain the
value. The application then validates what the value means.

When `secret` is true, the adapter shows a masked field, such as a SwiftUI
`SecureField` or an Android password-type input. It never echoes, logs,
persists, autofills, or suggests the value, and it clears the field after
submitting. The value goes only to the application.

## Drawing surfaces

`surface::Viewport` checks physical dimensions (at most 8,192 on either axis
and 16,777,216 pixels in total) and finite logical-to-physical scale
(0.25–8). Zero dimensions represent a temporarily hidden surface. An adapter
can impose smaller GPU or device bounds.

`SurfaceLifecycle` describes one native mount. It starts inactive. Frame
timestamps must be finite, nonnegative, and monotonic within an active period.
The first frame after activation or a hidden viewport returns zero elapsed
time; later frames return at most 50 ms. A destroyed mount refuses reuse.
The adapter supplies timestamps; the core starts no timer or thread.

The native layer or window must outlive its GPU renderer. Stop display callbacks,
clear held input, suspend the application's surface subscriptions, release the
renderer, and only then release the native object. Deactivation must not replay
background elapsed time as movement. Applications decide whether unrelated
durable work continues. A resource registration grants no network, credential,
or task-execution authority.

Native drawing is distinct from native text and controls. Keep accessible
labels and controls in the semantic tree or platform controls. A GPU drawing
surface does not make its pixels, geometry, or custom text accessible by itself.

## Bundled font selection

A display-list `Font` has one family, `FontFamily::PaperMono`, the default.
Every weight maps to the matching `wght` value of Paper Mono's variable face,
and italic and monospace styles use the same face. The family stays a type so
that it participates in measurement and glyph cache identity, and
`TranscriptLayout::set_font_family` still invalidates measured rows on the
next update without mutating a published frame.
The existing font-spec C ABI continues to select the default pair.

`Style.radius` selects 0–128 point rounded corners. `Style.border` adds a
one-point inset stack border. `Style.fill_height` fills the remaining height
of a bounded vertical container; the desktop split adapter allocates its body
from the actual header and footer sizes. These properties compose and reset
like the existing style leaves. Adapters that do not implement them retain
their established rendering.

`TranscriptLayout::set_metrics` selects bounded reading width, body font size
and line height, row spacing, and bubble geometry for a mounted reader. The
next update invalidates affected measurements. The defaults preserve existing
reader geometry. A native field can likewise select bounded font, line,
padding, and height metrics without changing editing or IME state.
Optional Markdown metrics can select inline-code size, vertical paint inset,
corner radius in half points, and explicit sRGB text color. These affect
presentation only; source ranges, selection, and inert link destinations stay
in the same display list. Without them, existing inline-code rendering remains.
An optional strong-emphasis weight preserves a heavier heading or table-header
weight instead of replacing it.

For measured designs, optional `Style.padding_points`, `gap_points`,
`text_size`, `line_height`, `min_height`, and `button_padding` select exact
logical-point dimensions. Explicit padding replaces semantic padding;
button padding controls its horizontal and vertical content insets. A stack
border reserves one point on each edge. View validation bounds all dimensions
before an adapter lays out the tree. Existing semantic spacing and renderer
defaults apply when these leaves are absent.


Control styles can set `glyph_size`, `glyph_gap`, and `glyph_color` independently
of the button's hit area. `monospace` selects the monospaced member of the
application's font family. These tokens compose and reset through `StylePatch`;
adapters document their support and preserve the control label as a fallback.
A circular icon button takes an explicit `min_height` as its side, so a
titlebar can draw compact 24-point controls; without it the adapter's icon size
applies.

An optional `Style.viewport` shows a vertical stack through a bounded, scrolled
window: `max_height` (1–4096 points) caps the stack's height, the application's
`offset` scrolls its content, including its padding, and `fade` (at most 128
points) fades each edge that hides content toward the stack's background. The
application owns the offset, so hover can never move rows; an adapter clamps it
to the content, clips drawing and hit targets to the window, and reports the
laid-out region. Other axes are invalid. An adapter without viewports lays out
the whole stack.


## Button initials and text weights

An optional `Style.button_avatar` places a circular initial before a button's
label. The application supplies the initial, diameter, text size, weight, and
colors. The initial must be a visible character, and both sizes are bounded to
1–128 logical points. The semantic button retains one label and one action;
its avatar carries no separate interaction or account state. Adapters without
avatar painting preserve the button label and action.

Text weights are `normal` (400), `medium` (500), `semibold` (600), and `bold`
(700). An explicit weight overrides the role's default. Optional hover foreground
and background colors apply to an enabled button; unset properties preserve
adapter defaults. These properties compose and reset independently.
