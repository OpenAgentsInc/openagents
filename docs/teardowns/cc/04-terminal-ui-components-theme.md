# Teardown 04 — Terminal UI, Ink Components, Keybindings & Theme

**Claude Code** (`~/work/projects/repos/cc`) vs **OpenAgents Coder** (`packages/openagents-cli/src`)

> Mapping note: the brief names `coder-status.ts`, `coder-prompt.ts`, and `coder-events.ts` as the OpenAgents counterparts. Those files do not exist. Their concerns are folded into one module: the status line, composer/input handling, and event subscription all live in `coder-ui.ts` (1,572 ln), supported by `coder-markdown.ts` (490 ln), `coder-plain.ts` (190 ln), and the `onChange`/`snapshot()` emitter in `coder-session.ts`. This report compares against what actually exists.

## Component / Subsystem Breakdown

| Concern | Claude Code | OpenAgents Coder |
|---|---|---|
| Renderer | vendored Ink fork: `ink/ink.tsx` (1,723 ln), `reconciler.ts`, `renderer.ts`, `frame.ts`, `screen.ts`, `optimizer.ts` | none — direct ANSI writes in `coder-ui.ts` |
| Layout | Yoga WASM via `ink/layout/{engine,yoga,node}.ts` | fixed-row template (constants for fleet/composer/spacer rows) |
| Components | `components/` — 144 entries; design-system, messages/, PromptInput/, Spinner/, StructuredDiff/ | inline string builders (`justify`, `hints`, `fleetColor`, `childScreenLines`) |
| Screens | `screens/REPL.tsx` (5,005 ln), `Doctor.tsx` (574), `ResumeConversation.tsx` (398) | one `runCoderUi()` closure with three modes: `chat`, `skills`, `child` |
| Keybindings | `keybindings/` — 15 modules, 9,136 ln total | raw escape-sequence comparisons inline in `onData` |
| Theme | `utils/theme.ts`: 6 themes × ~70 semantic tokens; `ThemeProvider` with preview/save | 11 hardcoded SGR constants (`DIM`, `BOLD`, `CYAN`, …) |
| Input parsing | `ink/parse-keypress.ts` (801 ln) incremental state machine | chunk walker + 40 ms lone-ESC timer |

## Claude Code Implementation Details

**The Ink fork.** CC does not consume upstream Ink; it vendors and heavily modifies a React-based TUI. `reconciler.ts` mounts `react-reconciler` over a DOM of `Box`/`Text` nodes (`dom.ts`), lays out with Yoga (`layout/yoga.ts`), and renders through a double-buffered frame pipeline: `frame.ts` holds front/back `Screen`s built from interned pools (`CharPool`, `StylePool`, `HyperlinkPool` in `screen.ts`), `renderer.ts` diffs back→front and refuses to blit when `prevFrameContaminated` (post-render selection overlays, alt-screen enter, SIGCONT, forceRedraw), and `optimizer.ts` collapses the patch stream in one pass (merge cursor moves, cancel hide/show pairs, dedupe hyperlinks). Render scheduling is throttled at `FRAME_INTERVAL_MS = 16`. Alt-screen gets DECSTBM scroll-region hints (`scrollHint`), park patches on resize, and ScrollBox drain frames. Beyond core rendering: `hit-test.ts` dispatches clicks, `selection.ts` (917 ln) implements native-style text selection, `terminal-querier.ts` round-trips DECRPM/keyboard-protocol queries, and `searchHighlight.ts` overlays match positions.

**Component library.** `components/design-system/` provides `ThemedText`/`ThemedBox`/`Dialog`/`Tabs`/`FuzzyPicker`/`ProgressBar`/`StatusIcon` on top of the theme context. `components/messages/` maps every message type to a renderer; `VirtualMessageList` virtualizes long transcripts with scroll chrome; `PromptInput/` composes `TextInput` or `VimTextInput` (vim modal editing), history search, paste normalization, suggestions footer, and a `ShimmeredInput`. `Spinner/` is a 1,268-ln subsystem of glyph/shimmer/glimmer animations including teammate trees. `StructuredDiff/` renders syntax-aware diffs with word-level highlighting driven by theme tokens `diffAddedWord`/`diffRemovedWord`.

**Keybindings** form a real subsystem: `schema.ts` (zod, generates JSON schema), `parser.ts` (keystroke strings → normalized `ParsedKeystroke` with ctrl/alt/shift/meta/super), `match.ts` (Ink `Key` → name matching), `resolver.ts` (pure resolve returning `match | none | unbound | chord_started | chord_cancelled`), 18 named contexts (`Global`, `Chat`, `Confirmation`, `HistorySearch`, `DiffDialog`, …) so bindings scope by focus, `defaultBindings.ts` (340 ln, 20 context blocks, ~79 distinct action IDs, platform-aware: `shift+tab` falls back to `meta+m` on Windows without VT mode), `loadUserBindings.ts` (watches `~/.claude/keybindings.json`, hot reload), `reservedShortcuts.ts` (`ctrl+c`/`ctrl+d`/`ctrl+m` non-rebindable, terminal-reserved warnings), and `useShortcutDisplay`/`shortcutFormat.ts` so help text reflects actual user bindings.

**Theme.** `utils/theme.ts` defines `THEME_NAMES` = dark/light × {default, daltonized, ansi}; each theme is ~70 named semantic tokens (success/error/warning, diff quadrants, subagent palette `*_FOR_SUBAGENTS_ONLY`, shimmer variants of several tokens, rate-limit gauge fills). `ThemeProvider` resolves `'auto'` through OS detection (`systemTheme.ts`), supports live preview with save/cancel, persists to global config, and feeds a `ThemePicker` that renders live sample diffs.

## OpenAgents Coder Implementation State

`coder-ui.ts` is a deliberate zero-dependency ANSI painter — the header documents why: OpenTUI's FFI is Bun-only while the CLI must run on Node. It enters the alternate screen (`?1049h`), enables alt-scroll (`?1007h`), the kitty keyboard-disambiguation protocol (`>1u`), and bracketed paste (`?2004h`), then runs a closed-loop redraw cycle:

- **Differential painting.** `render()` builds all rows as strings; `paint()` compares against the previous `painted[]` array and emits `\x1b[N;1H` + `ERASE_LINE` + content only for changed rows, plus erases rows the last frame had. Nothing clears the screen or scrolls, preserving the terminal's own scrollback discipline. Resize drops `painted[]` and repaints everything.
- **Input.** `onData` treats a chunk as *many* keypresses, walking bytes: paste bodies are lifted out whole (held in `pendingPaste` until the terminator arrives), incomplete escapes held in `pendingEscape`, and a bare `\x1b` waits a 40 ms `ESCAPE_WINDOW_MS` before meaning Escape — so a single press interrupts. `controlFromKeyboardProtocol` maps kitty-style sequences back to control codes.
- **Modal screens.** `chat` / `skills` / `child` each own the keyboard; a stray letter never falls through into an invisible composer. The child screen pages tool output (`CHILD_OUTPUT_ROWS = 12`); skills screen is arrows + space-toggle.
- **Layout.** Fixed constants (`STATUS_ROWS`, `COMPOSER_ROWS = 3`, `SPACER_ROWS`, `FLEET_ROWS_MAX = 8`, `PREVIEW_ROWS = 3`, sidebar 34 cols gated at ≥100 terminal columns) replace a layout engine. Below threshold the fleet renders inline — a graceful-degradation rule stated in comments.
- **Scrolling.** An absolute-line `anchor` keeps a scrolled-up reader parked while content streams; snapping to follow happens at the bottom.
- **Markdown.** `coder-markdown.ts` provides `visibleWidth` (ANSI-aware), `wrapStyled`, and `renderMarkdown` for transcript entries; `coder-plain.ts` renders the identical snapshot line-oriented for non-TTY, both fed by one snapshot from `coder-session.ts`'s `onChange` emitter.

Testability is strong: `test/coder-ui.test.ts` (1,259 ln, 13 describe blocks) drives fake stdin/stdout and asserts on emitted strings — something the React tree makes far harder.

## Detailed Gap Analysis

1. **Theming (largest gap).** No palette abstraction: colour is 4-bit SGR baked into logic. No light/dark, no accessibility variants (CC ships daltonized pairs), no `NO_COLOR`/`COLORTERM`/truecolor negotiation, no user preference, no `auto`.
2. **Keybindings.** Keys are byte literals scattered through `onData` branches (`"\x1b[A" || "\x1bOA"` appears in four places). No rebindable actions, no contexts, no chords, no user config file, no display-string source — `/help` hints are hand-written literals that can drift from behaviour.
3. **Component reuse.** Every widget is a local function; a second consumer of the fleet block or status line would duplicate it. CC's design-system primitives (select lists, dialogs, progress bars, tabs) have no equivalents.
4. **Rich interaction.** No mouse (click-to-focus a child in the sidebar is the obvious win; OA already owns absolute row math), no text-selection assistance, no transcript search/highlight (CC: `searchHighlight` + ctrl+o transcript toggle), no history search (ctrl+r), no vim mode, no rewind/message-selector, no context-window visualization, no diff dialog with file list/detail drill-down, no Doctor diagnostics screen.
5. **Animation/feedback.** A boolean `pulse` vs CC's shimmer/glimmer/spinner grammar with stalled-intensity and token counters; status line lacks cost/token/context facts (CC tracks these centrally).
6. **Layout generality.** Fixed-row templates cannot express nested panes or bottom-anchored overlays (dialogs, pickers) without ad-hoc math; CC's Yoga layout generalizes, at heavy cost.

Architecturally the trade is explicit and mostly sound: OA trades Ink's ecosystem (mouse, selection, hyperlinks, devtools, arbitrary layouts) for Node-portability, zero deps, string-level testability, and a codebase one person can hold in their head. The gaps that hurt users are theming and rebinding; the gaps that hurt *development velocity* are the missing input-abstraction and widget layers.

## Concrete Actionable Porting Recommendations

1. **Port a token table first** (`coder-theme.ts`): copy `utils/theme.ts`'s semantic shape (subset: text, subtle, success/error/warning, diffAdded/diffRemoved, accent) mapped to SGR sequences, with a `NO_COLOR` and 4-bit fallback per palette. Mechanically replace the 11 constants. Unblocks every other item.
2. **Extract input parsing** (`coder-keys.ts`): normalize each walked chunk item into `{ name, ctrl, alt, shift }` (port `parser.ts`/`match.ts` concepts, not the Ink coupling). Handlers switch on logical names; delete duplicated byte literals. Keep the kitty protocol + 40 ms ESC fallback exactly as is — it is better tested than it looks.
3. **Add a binding table**: `{ action → sequences[] }` with CC-style reserved-shortcut validation (`ctrl+c/d/m`). Ship defaults matching today's behaviour, then read optional overrides from the existing persisted-configuration store — no watcher needed initially.
4. **Formalize the painter** into a tiny `Screen` class (row buffer + `diff()` + flush + cursor placement) extracted from `paint()`. Zero new deps; gives every future widget a testable contract.
5. **Promote widgets**: move `justify`, `hints`, `fleetColor`, status-line assembly, and child screen into `coder-widgets.ts`; then port CC's `ProgressBar`, `Tabs`, and select-list as pure string functions (patterns only — the React code will not transplant).
6. **Mouse**: send `?1006h` alongside the existing `?1007h`; decode SGR clicks; hit-test is trivial given owned row math — wire click-on-sidebar-row to the existing child screen.
7. **Transcript search**: scan rendered `lines[]` for a query, jump `anchor` to matches, highlight via reverse video; mirrors CC's ctrl+o/search pairing at 10% the surface.
8. **Diff presentation**: extend `summarizeToolCall` output with word-level added/removed colouring using the new theme tokens (parity with `StructuredDiff/Fallback.tsx`, not the full syntax highlighter).
