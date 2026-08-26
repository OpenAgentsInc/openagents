# coder-lite TUI parity inventory

The inventory issue #117 gates its porting half on. Every feature below carries
exactly one disposition — **ported**, **dropped**, or **to-port** — and every
claim carries a file, a line, or a count. Best practice V3: parity claims
quantify, and adjectives are not evidence.

Read `docs/coder/autoimprove.md` §4 and `docs/coder/best-practices.md` V2/V3
first. The porting half of #117 remains blocked on #116 (the PTY harness): no
entry here may be marked done on headless evidence.

---

## 0. Two notes on freshness

**Tree state.** Everything below was read at `f3bad5277b` (2026-08-26). Line
numbers are from that tree.

**Release 0.0.2 changes the framing, not the boundary.** As of release 0.0.2
the shipped artifact *is* `coder-lite`, and it answers two fronts:

- bare invocation with no arguments → the interactive TUI session
- `<binary> issue list` and peers → dispatches into the `openagents-cli`
  command set

So the module doc in `crates/coder-lite/src/main.rs` that currently says this
binary "is not `openagents`", and the framing of `oa coder` as a separately
shipped second front end, is **stale on arrival**. This document is written
against the post-release shape: one artifact, two fronts. Where a count below
comes from the pre-release tree it says so.

The consequence for #117's guardrail is that the runtime boundary gets
**sharper, not softer**. It is no longer a binary seam that a linker enforces;
it is a module seam inside one shipped executable, held only by review. Tools,
refusal patterns, composer semantics, lanes, the model catalog, plugins,
skills, goals, and metering stay in `openagents-cli`. `coder-lite` owns the
frame, the palette, the spinner, the system prompt, and the session loop. A
"to-port" that would move runtime logic across that seam is a boundary
violation and is flagged as one in §7 rather than scheduled.

`--version` reports the release version after 0.0.2, not `CARGO_PKG_VERSION` as
`main.rs:129` does today.

---

## 1. What is actually being compared

The issue says "the TypeScript and Rust coder UIs". There are **three**
implementations, not two, and all three are live.

| # | Implementation | Entry | Size | Status |
|---|---|---|---|---|
| 1 | **coder-lite** (Rust) | bare `coder-lite` → `interactive::run_tui` | 3,976 app lines + 24,127 vendored markdown | the shipped UI (#105) |
| 2 | **`oa coder`** (Rust) | `cli.rs:1446` → `interactive::run_tui` | 5,176 TUI-stack lines + 2,017 shared composer | live, second front |
| 3 | **`coder-ui.ts`** (TypeScript) | `cli.ts:57` `runCoderUi` | 3,862 lines | live in the npm CLI |

Per-file sizes:

- **coder-lite**: `markdown/` 26,113 (of which the port is 24,127 `.rs`),
  `interactive.rs` 581, `tui.rs` 504, `runtime.rs` 503, `transcript.rs` 485,
  `commands.rs` 419, `acp_tool.rs` 416, `acp.rs` 310, `export.rs` 276,
  `main.rs` 213, `osc8.rs` 191, `acp_harness.rs` 60, `lib.rs` 18.
- **`oa coder`**: `interactive.rs` 1,370, `markdown.rs` 1,120, `diff.rs` 1,092,
  `tui.rs` 841, `pty.rs` 753.
- **shared composer** (`openagents-cli/src/composer/`): `edit.rs` 650,
  `mod.rs` 463, `complete.rs` 324, `history.rs` 308, `keys.rs` 272.
- **TypeScript**: `coder-ui.ts` 1,767, `coder-session.ts` 1,415,
  `coder-markdown.ts` 490, `coder-plain.ts` 190.

Test weight, since the whole point of #116 is that none of it observes a real
terminal:

| Surface | Tests | What drives them |
|---|---|---|
| coder-lite | 674 (596 in `markdown/`, 49 integration, 29 unit) | `TestBackend` buffers; **15** are frame tests (`tests/frame.rs` 8, `tests/rebase_contract.rs` 7) |
| `oa coder` | 189 over the TUI stack (170 `#[test]`, 19 `#[tokio::test]`) | `TestBackend` for display; **real** PTYs, child processes, git repos, and SSE for the rest |
| TypeScript | 150 cases (`test/coder-ui.test.ts`, 1,353 lines, 14 `describe`) | fake stdin/stdout, string assertions |

**Nothing anywhere drives a real terminal.** `oa coder` gets closest — it runs
real child processes under real pseudoterminals and asserts a genuine
`SIGWINCH` round-trip (`tests/coder_tui_test.rs:1934`) — but even there the
display side is a `TestBackend`. That is exactly the gap #116 fills, and
`openagents-cli` already depends on `portable-pty` (`pty.rs`), so the harness
has its dependency in the workspace already.

---

## 2. Where the issue's premises did not survive contact

Recorded per the honesty contract. Three of the six existing CC teardowns had
to do the same and were better for it.

**2.1 — "composer multi-line editing, prompt history recall, kill/yank" are
not things to port *from* TypeScript. coder-lite already wins all three.**

The TypeScript composer is **one flat string** — `let composer = ""` at
`coder-ui.ts:485`, with **no cursor index variable anywhere in the file**. Text
is only appended (`:1732`) or truncated from the end (`:1702`, `:1709`).
Verified absent in `coder-ui.ts`: keyboard newline insertion (`\r`/`\n` are
unconditionally submit, `:1626-1653`), any intra-composer cursor movement,
word-wise editing, `ctrl+w`/`ctrl+k`/`ctrl+y`/`ctrl+a`/`ctrl+e`, forward
delete, prompt history of any kind (no `history` identifier in the file, no
on-disk file), and Tab completion of any kind (`\t` is bound to
`session.cycleReasoning()` at `:1694`).

coder-lite has all of it, through the grok-derived composer: 22 classified
chords in `composer/keys.rs:38-183`, newline on Alt+Enter / Shift+Enter /
Ctrl+J (`composer/mod.rs:115-125`), a 500-entry history persisted to
`~/.openagents/coder-history` (`composer/history.rs:21`, `:46`), and Tab
completion over commands, paths, and `@`-mentions (`composer/complete.rs`, 15
tests).

The direction of that arrow is the opposite of what the issue assumed.

**2.2 — "coder-lite has a fixed-row template with constants for
fleet/composer/spacer rows" describes `coder-ui.ts`, not `tui.rs`.**

The phrasing comes from `docs/teardowns/cc/04-terminal-ui-components-theme.md`,
where it correctly describes the **TypeScript** layout: `STATUS_ROWS = 1`,
`COMPOSER_ROWS = 3`, `SPACER_ROWS = 1`, `FLEET_ROWS_MAX = 8`,
`PREVIEW_ROWS = 3`, `SIDEBAR_WIDTH = 34`, `CHILD_OUTPUT_ROWS = 12`,
`GUTTER = 4`, `ESCAPE_WINDOW_MS = 40` (`coder-ui.ts:107-176`).

`tui.rs` uses a ratatui `Layout` with `[Min(0), Length(input_box_height),
Length(1)]` (`tui.rs:273-280`) where the composer grows 1..8 rows with its
content (`tui.rs:268-271`). It has no fleet rows and no spacer row because it
has no fleet display at all.

**2.3 — coder-lite is not missing scrollback.** `tui.rs:170-173` carries
`scroll_override: Option<u16>` / `scroll_max` / `transcript_height`;
`effective_scroll` (`:370-376`) follows the bottom on `None` and clamps an
explicit offset; `scroll_by` (`:381-390`) releases the override at the bottom
so the viewport resumes following. PageUp/PageDown move one viewport
(`interactive.rs:465-472`); Up/Down scroll one line once the prompt history is
exhausted (`:457-464`). `entries` is an unbounded `Vec` with no cap and no ring
— the same as both other implementations.

**2.4 — `packages/input-bindings` is not a source for this work.** It is 1,125
lines with **zero runtime consumers** (the only files that mention it are its
own `src/`, `package.json`, `README.md`, and one assurance-spec fixture). Its
binding shapes are DOM-native — `KeyboardEvent.code`, `KeyboardEvent.key`,
mouse buttons, wheel deltas (`src/index.ts:52-79`) — and its README says it was
written for "Autopilot Desktop and Verse", an app that was deleted on
2026-08-04 (#9325). It has a `"terminal"` context literal (`:19`) and nothing
that consumes it. A terminal binding table cannot be built from
`KeyboardEvent.code`. Recorded as **dropped (not applicable)**; coder-lite's
binding surface is `commands.rs:KEYS` plus `composer/keys.rs`.

**2.5 — `--offline` does not exist in coder-lite.** Its argument parser
(`main.rs:105-150`) accepts exactly five flags: `-h`/`--help`, `-V`/`--version`,
`--dev`, `--lane`, `--reasoning`; anything else is refused by name. Issue #116
names `--offline` in its minimum assertions ("use `--offline`, so the harness
needs no provider credential and no network"). The flag exists only on the
other front (`cli.rs:646`, `run_offline_coder` at `cli.rs:1411`). **This blocks
#116**, and it is item 1 of §6 for that reason.

**2.6 — "streaming markdown is already ported" is true, and the degree is
99.8%.** See §3.C.

---

## 3. The feature matrix

Legend: **P** ported · **D** dropped · **T** to-port. `CL` = coder-lite,
`OA` = `oa coder`, `TS` = `coder-ui.ts`.

### A. Composer and input

| # | Feature | CL | OA | TS | Disposition |
|---|---|---|---|---|---|
| A1 | Multi-line edit (Alt+Enter / Shift+Enter / Ctrl+J newline) | yes | yes | no | **P** — `composer/mod.rs:115-125` |
| A2 | Cursor movement: grapheme, word, logical line | yes | yes | no | **P** — 22 chords, `composer/keys.rs:38-183` |
| A3 | Kill: Ctrl+U to line start, Ctrl+K to line end, Ctrl+W word | yes | yes | partial | **P** — TS has only Ctrl+U, and it clears the whole composer (`coder-ui.ts:1708`) |
| A4 | Forward delete (`Delete`, Alt+D) | yes | yes | no | **P** — `composer/keys.rs:69`, `:155` |
| A5 | Prompt history, 500 entries, persisted to disk | yes | yes | no | **P** — `composer/history.rs:21`, `:46`, `:54` |
| A6 | Tab completion: commands, paths, `@`-mentions | yes | yes | no | **P** — `composer/complete.rs`, 15 tests |
| A7 | Kitty keyboard disambiguation | yes | no | yes | **P** — `interactive.rs:133-136` (3 flags); TS uses `\x1b[>1u` |
| A8 | Caret drawn where the caret is, plus a block cursor | yes | yes | n/a | **P** — `tui.rs:334-361`; TS has no caret to draw |
| A9 | **Bracketed paste** | no | no | yes | **T** — §6.3 |
| A10 | **Paste placeholdering** (`[Pasted text #1 +10 lines]`) | no | no | yes | **T** — `coder-paste.ts:15-44`, thresholds 800 chars / 1 line |
| A11 | **Image paste** (dropped paths to `[Image #N]`) | no | no | yes | **T** — `coder-image.ts`, 4 extension families; boundary-split, §7 |
| A12 | Kill-ring / yank (Ctrl+Y) | no | no | no | **T (low)** — absent everywhere; upstream has it at `xai-ratatui-textarea/src/textarea.rs:2452` |
| A13 | Undo / redo | no | no | no | **T (low)** — upstream `textarea.rs:2245`, `:2261` |
| A14 | Tier / reasoning cycling on Tab / Shift+Tab | no | no | yes | **D** — removed from the Rust line deliberately; the recorded reason is at `oa coder` `tui.rs:596-607` (a `Tab: effort` hint removed for being inert). Tab is completion in coder-lite and that is the better claim on the key |

The composer port itself: 922 of upstream `xai-ratatui-textarea`'s 12,237
`src/` lines were taken (`editor_keys.rs` 205 to `keys.rs` 272 with added docs
and 5 tests; `editor.rs` 1,002 to `edit.rs` 650, trimmed). `textarea.rs` (3,612
— the widget, with selection, undo, and the kill buffer) and `wrapping.rs`
(605) were not: the wrap geometry is written locally in `composer/mod.rs:235`
`wrap_rows`. A12 and A13 are what that omission costs.

### B. Keybindings

| # | Feature | CL | OA | TS | Disposition |
|---|---|---|---|---|---|
| B1 | Binding count | 7 session + 5 composer-level + 22 chords = **34** | **68** | **38** | — |
| B2 | Advertised-vs-handled invariant, commands | yes | yes | no | **P** — `interactive.rs:563-580` (two tests). TS's `/help` is a literal and has already drifted: it advertises 8 commands and omits `/plugin` (`coder-session.ts:940-948`) |
| B3 | Advertised-vs-handled invariant, **keys** | no | yes | no | **T (small)** — `oa coder` presses every hint the bar names (`tests/coder_tui_test.rs:294`, `:1992`). coder-lite's `KEYS` table (`commands.rs:49-60`) is asserted by nothing |
| B4 | Rebindable actions, contexts, chords, user config | no | no | no | **D** — no implementation has it; `packages/input-bindings` is not usable here (§2.4). Revisit only with owner direction |
| B5 | Esc / Ctrl+C / Ctrl+D / Ctrl+Q all exit | yes | partial | partial | **P** — `interactive.rs:325-337`, checked before the composer sees the key. Note: **no interrupt-a-running-turn key**; see E9 |

### C. Rendering — the vendored markdown engine

The port is **structural, not semantic**. Against the local grok-build
checkout:

| Measure | Value |
|---|---|
| Upstream `xai-grok-markdown/src` lines | 20,663 |
| Vendored lines from that crate | 20,699 (**99.8%** carried, 8 of 23 files byte-identical) |
| Harvested from `xai-grok-pager-render` | 3,145 (`core.rs`, `wrapping.rs`, `line_utils.rs`, `util.rs`) |
| Total vendored `.rs` | 24,127 |
| Semantic (not path-rewrite) changes | 3: `util.rs` narrowed 484 to 56 (2 of 20 functions kept), `line_utils.rs` dropped the `tool_paths` re-export, `streaming.rs` gained a `reparsed_bytes` counter |
| Tests carried | 596 in `markdown/` |

| # | Feature | Disposition |
|---|---|---|
| C1 | Streaming checkpoint / tail rewind, 8 checkpoint kinds | **P** — `markdown/checkpoint.rs:41-58`, `streaming.rs:335-350` |
| C2 | Incremental open-code-block highlighting | **P** — `open_code_highlighter.rs`, 439 lines |
| C3 | Syntax highlighting, full `two-face` extended grammar set | **P** — `syntax.rs:41` |
| C4 | Tables with alignment, cell wrap, links in cells | **P** — `parse.rs:1045-1061` |
| C5 | Task lists, strikethrough (double-tilde) | **P** — `core.rs:23-25` |
| C6 | LaTeX to Unicode, 4 delimiter forms plus `\begin{equation}` | **P** — `latex/` 1,990 plus `latex_delimiters.rs` 1,305 |
| C7 | Mermaid to Unicode box art (`graph`/`flowchart`/`sequenceDiagram`/`stateDiagram`) | **P** — `mermaid.rs`, 5,237 lines byte-identical |
| C8 | CJK / emoji width-correct soft wrap | **P** — `wrapping.rs:14`, `:89`. TS has no wcwidth at all (`coder-markdown.ts:39-41` counts a CJK glyph as 1 column) |
| C9 | Hyperlink metadata plus wrapped-fragment grouping | **P** — `hyperlinks.rs` 784 |
| C10 | Plain-URL autodetection in prose | **P** — `url_scan.rs` |
| C11 | OSC-8 emission | **P** — own emitter, `osc8.rs` 191 lines. Upstream's lives in the pager; TS parses no markdown links at all |
| C12 | Wrap cache keyed `(width, generation)`, O(n^2) to O(n) | **P** — `transcript.rs`, observable via `WrapStats` |
| C13 | Mermaid SVG/PNG raster | **D** — upstream `xai-grok-mermaid` (2,170 lines: dagre layout, resvg raster, `mmdc`) not ported. A terminal cannot show raster without a graphics protocol coder-lite does not negotiate |
| C14 | Footnotes | **D** — absent upstream too (`core.rs:22-27` does not set `ENABLE_FOOTNOTES`; the `FootnoteReference` arm at `parse.rs:815` is unreachable). Faithful port of an upstream gap, not a porting loss |
| C15 | Criterion benches (603 lines), fuzz target plus 9 seed corpora, 3 playground binaries (877 lines) | **D** — 1,515 non-library lines, no product value here |
| C16 | Inline graphics (Kitty protocol, iTerm2 detection) | **D** — upstream keeps it in `xai-grok-pager-render/src/terminal/image.rs`, outside the markdown crate. Out of scope for #117 |
| C17 | **NO_COLOR / 256 / 16-colour degradation** | **T** — the machinery is ported (`colors.rs:53-95` `detect_color_level`, honours `NO_COLOR` at `:56`) and then **defeated**: `transcript.rs:277` calls `theme::amberize`, which overwrites every span's fg/bg with `Rgb(255,176,0)` / `Rgb(8,6,0)` (`theme.rs:112-123`). `tui.rs` hardcodes the same two RGB values for all chrome. So `NO_COLOR=1` changes nothing a reader sees. See §6.7 |

The engine is about 6x the size of the application consuming it: 24,127
vendored lines against 3,976 in the rest of coder-lite. Only three files touch
it — `transcript.rs:23-25`, `tui.rs:15` (palette constants only), and `mod.rs`.

### D. Rendering — the frame

| # | Feature | CL | OA | TS | Disposition |
|---|---|---|---|---|---|
| D1 | Transcript scrollback with follow-the-bottom | yes | yes | yes | **P** — §2.3 |
| D2 | Per-role styling | 6 roles | 5 roles | 5 roles | **P** — `tui.rs:22-38`. CL adds `Role::Output` for command output, so a `/diff` is markdown-rendered but exported as a notice rather than as a model step |
| D3 | Braille spinner while streaming | yes | pulse | pulse | **P** — `tui.rs:20`, 10 frames. OA and TS use a 2-phase glyph pulse (400 ms / 500 ms) |
| D4 | Tool-call header with per-tool titles | yes | no | yes | **P** — `runtime.rs:398-416`, 5 tools. **OA renders no tool calls at all**: `Role::Tool` exists (`tui.rs:74`) and nothing constructs it |
| D5 | `— failed` on the header of a failed call | yes | no | yes | **P** — `interactive.rs:384-386` |
| D6 | Differential painting | ratatui | ratatui | hand-rolled | **P** — ratatui diffs cells; TS diffs a `painted[]` row array (`coder-ui.ts:1202-1219`) |
| D7 | Resize handling | implicit | explicit | explicit | **P** — CL redraws from `f.area()` each loop iteration, so a resize is absorbed. Untested; #116 names it |
| D8 | **Tool-call collapse / expand** | no | no | yes | **T** — `coder-ui.ts:1371-1377`, `expanded: Set<callId>`, Ctrl+O, newest call only. CL is fixed at exactly 5 lines |
| D9 | **Tool status marks** (running / ok / failed) | no | no | yes | **T** — `coder-ui.ts:675-680`. CL shows a bare bullet and a text suffix on failure only |
| D10 | **Tool box pads to 5 rows even for 1 line of output** | bug | — | — | **T (fix)** — `tui.rs:454` `for i in 0..5` emits 5 rows unconditionally, so a one-line tool result costs four blank transcript rows |
| D11 | **Diff inspector pane** | no | yes | no | **T** — CL renders a static fenced `diff` block through the markdown engine (`commands.rs:154-191`); OA has a navigable pane: unified plus side-by-side (`v`), Tab file cycling, `CONTEXT = 3`, line-level Myers with `MAX_EDIT_DISTANCE = 1500`, 18 tests |
| D12 | Word-level / intra-line diff | no | no | no | **T (low)** — absent everywhere |
| D13 | Diff syntax highlighting | yes | no | n/a | **P** — CL's fence goes through syntect. **OA's dedicated pane has none** (`diff.rs` imports only `truncate_spans`). CL wins this one |
| D14 | **PTY pane** (`/run` under a pseudoterminal) | no | yes | no | **T** — CL's `/run` is a plain non-interactive `/bin/sh -c` into the 5-line box and says so (`commands.rs:213-219`). OA has `vt100::Parser` emulation, `TIOCSWINSZ`/SIGWINCH, Ctrl+] detach, a 19-arm key encoder, 20 tests |
| D15 | Transcript search / highlight | no | no | no | **T (low)** — CC has it; nothing here does |
| D16 | Splash / empty state | no | no | yes | **T (low)** — TS has a 3-tier wordmark with a deterministic texture (`coder-ui.ts:295-411`) |
| D17 | Box frame with badge title | composer only | yes | no | **T (owner-gated)** — OA draws `OpenAgents │ openagents coder` in a bordered header (`tui.rs:280-293`). #105 names CL's frame as the product, so this is a look change, not a port |
| D18 | Mouse support | no | no | no | **D** — TS states the reason (`coder-ui.ts:61-65`): alternate scroll (`?1007h`) was chosen instead so the terminal's own text selection keeps working. Adopt that reasoning |

### E. Fleet, delegation, and status

| # | Feature | CL | OA | TS | Disposition |
|---|---|---|---|---|---|
| E1 | **Live child rows during `/delegate` fan-out** | no | no | yes | **T (highest)** — §6.2 |
| E2 | **Child full-screen transcript view** | no | no | yes | **T** — `coder-ui.ts:915-980`, 5 entry kinds, `CHILD_OUTPUT_ROWS = 12` |
| E3 | **Stop the fleet** (Ctrl+X) | no | no | yes | **T** — `coder-ui.ts:1685-1690` |
| E4 | Token usage in the status bar | yes | yes | no | **P** — `tui.rs:342-352`. **TS never renders it**: `CoderMetrics` exists on every entry (`coder-session.ts:173-179`) and `coder-ui.ts` reads it zero times |
| E5 | **Model in the status bar** | no | yes | yes | **T** — `ui.model` is set from `Control::Model` (`interactive.rs:397`) and read only by `/export` (`:531`). Never rendered |
| E6 | **Lane in the status bar** | no | yes | no | **T** — CL prints the lane once in the opening notice (`interactive.rs:110`) and never again |
| E7 | **Repo / branch in the status bar** | no | no | yes | **T** — CL sets both from `git_info()` (`interactive.rs:71-72`) and reads them only in `/export` |
| E8 | **Reasoning level surfaced** | no | no | yes | **T** — `ui.reasoning` is assigned at `interactive.rs:73` and **read nowhere in the crate**. A dead field |
| E9 | **Interrupt a running turn** | no | no | yes | **T** — TS `esc`/`ctrl+c` interrupt, then exit when there is nothing to interrupt (`coder-ui.ts:1347-1369`). In CL every exit key leaves the session (`interactive.rs:325-337`); there is no way to stop a turn without leaving |
| E10 | **Reasoning summaries in the transcript** | no | no | yes | **T** — `Role::Reasoning` has styling (`tui.rs:477`) and is constructed **only by a test** (`tests/rebase_contract.rs:64`). OA accumulates reasoning in `runtime.rs` and never surfaces it |
| E11 | Elapsed time on the running turn | no | no | yes | **T (small)** — `working… (2m 5s)` (`coder-ui.ts:1109-1111`) |
| E12 | Scroll-position indicator | no | no | yes | **T (small)** — `coder-ui.ts:1112-1114` |
| E13 | Home / End to top and bottom of transcript | no | diff pane only | yes | **T (small)** — `coder-ui.ts:1612-1613` |
| E14 | Status state word (running / streaming / ready) | no | yes | no | **T (small)** — `oa coder` `tui.rs:655-674` |
| E15 | Key hints in the status bar, dropped whole when narrow | no | yes | partial | **T (small)** — OA has 14 hints across 5 pane-aware lists (`tui.rs:608-624`) and drops them rather than showing half of one (`tests/coder_tui_test.rs:461`) |
| E16 | Thread id | no | no | no | **T** — named by #117; no implementation shows it. See §8 |
| E17 | Cost, context-window percentage | no | no | no | **D** — out of scope for #117; neither fact is computed anywhere in the three surfaces |
| E18 | `ui.agents` (discovered ACP agents) rendered | no | n/a | n/a | **T (small)** — assigned at `interactive.rs:91`, read nowhere. Announced once in the opening notice text instead |

### F. Commands

The native Coder handles **10**: `/clear`, `/diff`, `/export`, `/goal`, `/help`,
`/info`, `/login`, `/logout`, `/resume`, and `/run`.
`oa coder` handles **6** (`interactive.rs:63-79`): the same minus `/login`.
TypeScript handles **10**: `/reload`, `/skills`, `/plugin load`, `/resume`,
`/delegate` (UI side) and `/system`, `/goal`, `/export`, `/help`, `/?`
(session side).

| # | Command | Disposition |
|---|---|---|
| F1 | `/clear`, `/diff`, `/export`, `/help`, `/resume`, `/run` | **P** |
| F2 | `/login` — device flow driven from inside the TUI | **P** — coder-lite only (`commands.rs:106-116`); neither other surface has it |
| F3 | `/delegate` as a typed command | **T** — TS `parseDelegateCommand` accepts `[<n>x] <prompt>` (`coder-delegate.ts:96`). CL can only delegate through the model tool |
| F4 | `/skills` screen | **T** — TS has an arrows plus space-toggle screen (`coder-ui.ts:1490-1518`). Boundary-split, §7 |
| F5 | `/system` (describe the assembled context) | **T (small)** — `coder-session.ts:864-874` |
| F6 | `/plugin load <path>` | **T (low)** — `coder-ui.ts:1294-1307` |
| F7 | `/goal` | **P** — #138 ports the store and budget policy into the Rust runtime; the TUI dispatches the command and renders its active-goal footer field |
| F8 | `/reload` | **D (not applicable)** — TS re-execs itself from a source checkout (`coder-ui.ts:1264-1280`). A compiled binary has no equivalent |

TypeScript defect worth recording while retiring it: `coder-session.ts:968-1019`
is about 52 lines of **unreachable duplicate** `/export` and `/help` handlers,
guarded out by the live ones at `:910` and `:935`; the dead `/help` carries a
stale, shorter text than the live one.

### G. ACP

| # | Feature | Disposition |
|---|---|---|
| G1 | ACP agent discovery, availability checks, and the `acp` tool | **P — coder-lite only.** `acp.rs` 310, `acp_tool.rs` 416, `acp_harness.rs` 60. Nothing to port; this is a coder-lite capability neither other surface has |

---

## 4. Disposition counts

| Disposition | Count |
|---|---|
| **ported** | **32** |
| **dropped** | **10** |
| **to-port** | **31** |
| **boundary violation as scoped** (not schedulable without a split) | **2** |
| **could not determine** | **4** |

Total feature rows: 79. Dropped items each carry their reason in the matrix:
C13, C14, C15, C16 (upstream scope), D18 (text selection wins over mouse), A14
and F8 (not applicable to a compiled binary), B4 and §2.4 (no usable source),
E17 (out of #117's scope).

---

## 5. Which behaviour wins, and what that means for retirement

#105 said the finished state should be "**one** coder TUI, not two plus a
bridge". Three are live. Recording the winner per subsystem so the losers can
be retired knowingly rather than left to drift:

| Subsystem | Winner | Why | Loser's fate |
|---|---|---|---|
| Composer editing | **coder-lite** (shared composer) | 22 classified chords, persisted history, path and command completion, against a TypeScript composer with no cursor variable | TS composer: retire with `coder-ui.ts` |
| Markdown rendering | **coder-lite** (vendored engine) | 24,127 lines, 596 tests, tables/LaTeX/mermaid/CJK against a 490-line hand-written renderer and a 1,120-line per-line lexer | Retire `coder-markdown.ts`; retire `openagents-cli/src/markdown.rs` when `oa coder`'s frame goes |
| Hyperlinks | **coder-lite** | Only implementation that emits OSC-8 (`osc8.rs`) | — |
| Token accounting display | **coder-lite** | The only one that renders it; TS computes and discards | — |
| Diff **content** | **coder-lite** | Syntect-highlighted through the shared engine; `oa coder`'s pane has no highlighting | — |
| Diff **navigation** | **`oa coder`** | A real pane with file cycling and side-by-side; coder-lite prints one static block | Port the pane (§6.9), then retire |
| PTY / interactive `/run` | **`oa coder`** | Real terminal emulation with 20 tests; coder-lite explicitly does not attempt it | Port (§6.4), then retire |
| Status bar composition | **`oa coder`** | Pane-aware hints, segment-dropping under width pressure, an advertised-key test | Port the shape (§6.5); keep coder-lite's amber palette |
| Fleet / delegation display | **`coder-ui.ts`** | The only implementation with any; two surfaces, 4 columns, 8 activity renderers | Port (§6.2), then retire |
| Paste handling | **`coder-ui.ts`** | The only implementation with bracketed paste, placeholdering, or image drop | Port (§6.3), then retire |
| ACP | **coder-lite** | Sole implementation | — |

Sequence implied: land §6.1 through §6.5 and `coder-ui.ts` has nothing
coder-lite lacks except the child view (E2, E3); land §6.9's diff pane and the
`oa coder` frame has nothing left either. Retiring a front end is its own issue,
not part of #117.

---

## 6. The ordered to-port list

Dependency order, smallest first inside each tier, one landing per group —
what #117 asks for. Sizes: **S** under 100 lines, **M** 100 to 400, **L** over
400.

Every entry names the PTY assertion (#116) that proves it. Until #116 lands,
none of these may be claimed done (best practice V2).

### 6.1 — `--offline` on coder-lite · **S** · unblocks #116

**Files:** `crates/coder-lite/src/main.rs` (parser plus `HELP`),
`crates/coder-lite/src/runtime.rs` (select a stand-in reply source),
`crates/coder-lite/src/interactive.rs` (skip token validation and the login
prompt when offline).

**Why first:** #116's minimum assertions assume it. Without it the harness
needs a provider credential and a network, which makes it not CI-safe, which
was the point.

**Reuse, do not reinvent:** `cli.rs:814-832` already carries the stand-in
answer text and its rationale. The offline reply source is runtime, so it
belongs in `openagents-cli` and coder-lite selects it — see §7.3.

**PTY assertion:** `coder-lite --offline` starts, renders a composer, accepts a
typed prompt, submits it, and the transcript shows the stand-in reply — all
with no `OPENAGENTS_API_KEY` in the environment and no listener on the API
origin.

### 6.2 — Live fleet rows during `/delegate` fan-out · **M** · highest product value

**Files:** `crates/coder-lite/src/runtime.rs` (new `Control` variants),
`crates/coder-lite/src/tui.rs` (a fleet block),
`crates/coder-lite/src/interactive.rs` (`apply` arms), and one call site in
`crates/openagents-cli/src/delegate.rs:1604`.

**Current behaviour:** `delegate x3 <prompt>` renders one tool header and a
5-line box that stays empty until every child finishes, then fills with a wall
of text of which the last 5 lines are visible. `Control` (`runtime.rs:43-70`)
has ten variants and none carries child state. `ui.agents` (`tui.rs:177`) is
assigned once and read never.

**Why it is cheap:** the machinery already exists and is already consumed.
`ChildEvent` (`delegate.rs:83-97`) carries `Started { id, lane, workspace, pid }`,
`Output { id, text }`, `Activity { id, text }`, and `Finished`.
`DelegationSupervisor::dispatch_streaming(prompt, events: mpsc::UnboundedSender<ChildEvent>)`
exists at `delegate.rs:387`, and `oa delegate` already renders from it. The only
reason coder-lite sees nothing is that `fanout_for_tool` (`delegate.rs:1578`)
calls the blocking `dispatch()` at `:1604` and returns one string at the end.

**Boundary:** the `openagents-cli` edit plumbs an existing event channel
outward; it does not move logic inward. The supervisor, the lanes, the child
options, and the refusals stay where they are. Give `fanout_for_tool` an
optional `ChildEvent` sink so the existing blocking signature keeps working for
callers that do not want rows.

**Shape to copy** (`coder-fleet.ts:210-233`): 4 columns — a tree branch, a
status mark with 5 states (pending, running, completed, failed, stopped), a
`description` padded to `min(28, max(12, floor(room * 0.35)))`, and a tail of
`taskActivity` plus `taskCounters`. Cap the block at 8 rows (`FLEET_ROWS_MAX`)
with a `+N more` line, and 3 activity preview lines per child (`PREVIEW_ROWS`).
Skip the 34-column sidebar for now — it is a second layout mode, and the inline
block is the load-bearing half.

**PTY assertion:** with a fixture fan-out of 3 children, the frame shows 3
fleet rows within one draw of the first `Started`; a row's mark changes from
running to completed when its child finishes; and the block never exceeds 8
rows for a fan-out of 12.

### 6.3 — Bracketed paste, paste placeholders, image paste · **M**

**Files:** `crates/coder-lite/src/interactive.rs` (enable the mode, read
`Event::Paste`), `crates/coder-lite/src/tui.rs` (render the placeholder token),
plus a new paste-token module.

**Current behaviour:** grep confirms `EnableBracketedPaste`, `Event::Paste`,
and `2004` appear **nowhere** in `crates/`. `interactive.rs:166` is
`let Event::Key(key) = event::read()? else { continue; }`, so a paste event
would be dropped even if the mode were on. Today a multi-line paste arrives as
a stream of individual key events and the first embedded newline **submits the
prompt mid-paste**.

**Port from** `coder-paste.ts`: `PASTE_TEXT_THRESHOLD = 800` characters,
`PASTE_MAX_LINES = 1`, the `[Pasted text #1]` / `[Pasted text #1 +10 lines]`
placeholder, expansion at submit, and whole-token backspace. From
`coder-image.ts`: dropped-path detection for `png|jpe?g|gif|webp`, quote
stripping, backslash unescaping, `[Image #N]`.

**Boundary:** the placeholder token, its map, and its rendering are
presentation and belong here. Turning an image path into a content part (MIME
sniffing, base64, the multimodal message shape) is runtime and belongs in
`openagents-cli` — see §7.2.

**PTY assertion:** writing a bracketed-paste sequence carrying two lines leaves
the composer holding two lines and **does not submit**; a 900-character paste
renders as a `[Pasted text #1 +N lines]` token; one backspace removes the whole
token.

### 6.4 — `/run` under a pseudoterminal · **L**

**Files:** a new pane in `crates/coder-lite/src/tui.rs`, pane dispatch in
`crates/coder-lite/src/interactive.rs`, `commands.rs` `/run` rewired.

**Current behaviour:** `commands.rs:211-300` spawns `/bin/sh -c` with piped
stdio into the 5-line box, and the doc comment states the trade honestly ("Not
a pseudoterminal: there is no pane to attach to and nothing takes keys while it
runs"). The cost is the usual one: `git` drops its colour, `top` and `vim`
refuse to draw, and anything that asks the kernel for its width gets nothing.

**Reuse:** `openagents-cli/src/pty.rs` is 753 lines and complete —
`PtySession` over `portable-pty`, `PtyScreen` over `vt100::Parser`, `resize`
raising a real `SIGWINCH`, `DETACH = Ctrl+]`, and a 19-arm `encode_key`. It is
already a library module of the crate coder-lite depends on, so this is pane
plumbing, not a port of the emulator.

**Do this after 6.2**, because both introduce a second thing competing for the
transcript's rows and the fleet block is the cheaper one to get the layout
right on.

**PTY assertion:** `/run stty size` in an 80x24 harness prints the pane's inner
dimensions, not the host's; `/run cat` echoes typed characters; Ctrl+] kills the
child and returns keys to the composer; resizing the harness mid-run re-reports
the new size to a `trap 'stty size' WINCH` script.

### 6.5 — The status bar: model, lane, repo, branch, reasoning, state, elapsed, hints · **M**

**Files:** `crates/coder-lite/src/tui.rs` (`render` status region),
`crates/coder-lite/src/interactive.rs` (stop dropping the facts it already
has).

**Current behaviour:** one right-aligned row of
`{p} prompt + {c} completion = {t} tokens` (`tui.rs:342-352`). Five facts the
frame **already holds** are rendered nowhere: `model` (E5), `repo` and `branch`
(E7), `reasoning` (E8, read by nothing at all), and `agents` (E18). The lane is
printed once at startup and never again.

**Shape to copy:** `oa coder` `tui.rs:648-725` — separated segments in priority
order, whole segments dropped from the end when the row narrows, then hints
dropped from the end. Keep coder-lite's amber palette; take the composition,
not the colours (#105 pins the palette as the product).

Land B3 with it: a test that presses every key `commands.rs:KEYS` advertises
and asserts something changed, matching `oa coder`'s
`every_key_the_status_bar_names_does_something`.

**PTY assertion:** after one offline turn the status row contains the model
name, the lane label, and the branch; narrowing the harness to 40 columns drops
whole segments and never renders a partial one; every row is exactly the window
width.

### 6.6 — Tool-call collapse/expand, status marks, and the 5-row padding fix · **M**

**Files:** `crates/coder-lite/src/tui.rs` (`render_entry`, `Role::Tool` arm),
`crates/coder-lite/src/interactive.rs` (a toggle key).

Three defects in one arm (`tui.rs:437-467`):

1. `for i in 0..5` emits exactly five rows whatever the output length, so a
   one-line result costs four blank transcript rows (D10).
2. There is no way to see more than the last five lines of any tool result
   (D8). TS keys this on Ctrl+O against the newest call (`coder-ui.ts:1371`).
3. A running call looks identical to a finished one; only failure is marked,
   and only as the text `— failed` (D9). TS uses three distinct marks.

**PTY assertion:** a tool result of one line renders one output row, not five;
Ctrl+O on the newest tool call grows the box and a second press shrinks it; a
call that is still running shows a different mark from one that has returned.

### 6.7 — Honour `NO_COLOR` · **S**

**Files:** `crates/coder-lite/src/markdown/theme.rs` (`amberize`),
`crates/coder-lite/src/tui.rs` (chrome styles).

The detection is already ported and already correct (`markdown/colors.rs:53-95`,
`NO_COLOR` checked first at `:56`) and then discarded: `transcript.rs:277` runs
`amberize`, which sets every span to `Rgb(255,176,0)` on `Rgb(8,6,0)`
(`theme.rs:112-123`), and `tui.rs` hardcodes the same pair for all chrome.
`NO_COLOR=1` currently changes nothing.

Gate `amberize` and the chrome styles on the detected level: at
`ColorLevel::None` emit `Color::Reset` and keep the modifiers, which is exactly
what `oa coder` does over the finished buffer (`tui.rs:41-50` `drain_color`).

**PTY assertion:** with `NO_COLOR=1` in the child's environment, the captured
frame contains no SGR colour parameters and still contains bold and dim
modifiers.

### 6.8 — Interrupt a running turn · **S**, possibly **M** (see §8.3)

Today every exit key leaves the session (`interactive.rs:177-179`, checked
before the composer sees the key), so the only way to stop a turn is to end it.
TS gives Esc and Ctrl+C a first meaning of "interrupt", falling through to exit
when there is nothing to interrupt (`coder-ui.ts:1347-1369`).

Whether cancellation is even available is an open question — see §8.3.

**PTY assertion:** Esc during a streaming offline turn stops the stream, posts
a notice, and leaves the session up; a second Esc exits.

### 6.9 — Second tier (schedule after the first)

| Item | Size | Note |
|---|---|---|
| Diff inspector pane (D11) | **L** | Port `oa coder`'s pane; add the syntect highlighting it lacks and coder-lite already has |
| `/delegate` as a typed command (F3) | **S** | Pairs with 6.2 |
| Child full-screen transcript view plus Ctrl+X stop (E2, E3) | **M** | Depends on 6.2 |
| Reasoning summaries in the transcript (E10) | **M** | `Role::Reasoning` is already styled and constructed only by a test |
| Scroll indicator, Home/End (E12, E13) | **S** | One landing |
| `/system` (F5) | **S** | — |
| `/skills` screen (F4) | **M** | Boundary-split, §7.4 |
| Splash / empty state (D16) | **S** | Cosmetic |
| Kill-ring/yank, undo/redo (A12, A13) | **M** | Harvest from upstream `textarea.rs` |
| Transcript search (D15) | **M** | Nothing here has it |
| Word-level diff (D12) | **M** | Nothing here has it |
| `/plugin load` (F6) | **S** | — |
| Box frame with badge title (D17) | **S** | **Owner-gated** — #105 pins the frame as the product |

---

## 7. Boundary violations flagged rather than scheduled

Per #117's guardrail. After release 0.0.2 this is a module seam inside one
binary rather than a seam between two, which makes it easier to cross by
accident and no less real.

**7.1 — `/goal` (F7) is ported at the required boundary.** Issue #138 places
the goal store, usage accounting, budget transition, standing turn context,
and model tool in `openagents-cli`. The TUI dispatches the slash commands and
renders the active-goal footer field. No budget policy lives in presentation.

**7.2 — Image paste (A11) must be split.** The placeholder token, its map, and
its rendering are presentation. MIME sniffing, base64 encoding, and assembling a
multimodal content part are runtime and belong beside the other message
construction in `openagents-cli`. Do not let `crates/coder-lite/` learn the
provider's content-part shape.

**7.3 — `--offline` (6.1) is a near miss.** The flag and the "am I offline"
branch are the session loop's, so they are coder-lite's. The stand-in reply
source is a reply source — the same category as the real one — and belongs in
`openagents-cli` next to `run_offline_coder` (`cli.rs:1411`) and the answer text
already written at `cli.rs:814-832`. Writing a second stand-in inside coder-lite
would be two implementations of one thing, which is precisely what
`Cargo.toml`'s dependency comment exists to prevent.

**7.4 — `/skills` (F4) must be split.** The screen is presentation. The skill
catalog, the enable/disable decision, and its effect on the assembled prompt are
runtime; coder-lite reads a list and reports a toggle.

**7.5 — Fleet rows (6.2) are *not* a violation, and the reasoning is worth
keeping.** The change touches `openagents-cli/src/delegate.rs`, but it plumbs an
existing event channel outward to a renderer. No supervision, lane selection,
child-option resolution, or refusal moves. The test of the rule is direction:
logic moving *into* coder-lite is the violation; a fact moving *out* to be drawn
is the boundary working.

---

## 8. What I could not determine

Four things. Recorded rather than guessed, per the honesty contract.

**8.1 — Whether coder-lite orphans child processes on exit.** TS explicitly
kills the fleet in teardown (`session.stopTasks()`, `coder-ui.ts:568-571`, "the
fleet dies with the console"). coder-lite's exit path awaits `session.finish()`
under a 10-second `REVOCATION_GRACE` (`interactive.rs:247-262`), which ends the
*thread*. Children are spawned inside the tool future by the supervisor;
whether they are killed, detached, or left running when the TUI exits is not
visible from the frame code, and I did not trace `DelegationSupervisor`'s drop
behaviour. Establish this before 6.2 ships — a fleet display that shows children
a reader cannot stop is worse than none.

**8.2 — Whether the thread id (E16) is available to the frame at all.**
`Control` has no `Thread` variant and no implementation renders one. Whether
`Session` holds an id it could send was not traced; if it does not, E16 needs a
runtime change and is a larger item than its status-bar row suggests.

**8.3 — Whether a turn can be cancelled (6.8).** `submit` spawns
`session.execute_turn(&text, tx)` on a detached tokio task
(`interactive.rs:517-519`) and keeps no handle. Interruption may require a
cancellation token in the runtime, which would move 6.8 from **S** to **M** and
put part of it on the other side of the seam.

**8.4 — Whether `oa coder`'s frame has any user after release 0.0.2.** The
release makes `coder-lite` the shipped artifact answering both fronts. Whether
`oa coder`'s TUI (`openagents-cli/src/{tui,interactive,diff,pty}.rs`, 4,056
lines, 189 tests) is still reachable from a shipped binary after that, or becomes
a library of parts that §6.4 and §6.9 harvest from, is an owner question. It
changes whether "retire it" in §5 means deleting a front end or absorbing one.

---

## 9. Landing rule for the porting half

Restating #117's acceptance so a later reader does not have to reconstruct it:

- Each `to-port` entry lands with the PTY assertion named beside it in §6, or is
  re-dispositioned here with a reason.
- `pnpm run check` green per landing (`fmt:check`, `lint`, `check:fast`,
  `typecheck`, `test`); the components alone are not the gate.
- No entry closes on headless evidence — best practice V2. The 15 `TestBackend`
  frame tests coder-lite has today are not that evidence, which is the whole
  reason #116 exists.
