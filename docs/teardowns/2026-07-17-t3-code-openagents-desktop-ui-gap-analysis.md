# T3 Code and OpenAgents Desktop: UI gap analysis — 2026-07-17

## Executive conclusion

OpenAgents Desktop now resembles T3 Code in three conspicuous places: the
right-aligned user-message bubble, the compact recent-chat sidebar, and the
settings-mode navigation rail. That work is real, but it is a first slice of
visual convergence rather than broad UI parity.

T3 Code still has the more coherent coding-workbench composition. Its chat is
one pane in a responsive, resizable system whose other panes can host files,
individual file tabs, diffs, terminals, plans, and browser previews. The same
product grammar continues through project-grouped navigation, a context-rich
composer, a virtualized transcript with a minimap, Git actions, settings,
dialogs, menus, empty states, update notices, and narrow-window behavior.

OpenAgents Desktop has a strong component catalog and a clearer authority
model, but the mounted React product is currently much narrower than that
catalog suggests. The visible React shell mounts chat and settings. Review,
files, terminal, project-home, and other typed foundations elsewhere in the
repository do not count as UI parity while they remain absent from the current
React workbench. The result is an interface with several individually polished
components but no equivalent multi-surface workbench around them.

The recommended direction is therefore:

1. adopt T3's **component composition, density, responsive behavior, and
   interaction completeness** for shared coding-workbench concepts;
2. preserve OpenAgents' dark Khala palette, explicit Queue/Steer and Full Auto
   semantics, typed authority, privacy, and fail-closed states;
3. converge whole component families, not isolated screenshots; and
4. make the mounted React surface the parity denominator—dormant views and
   unmounted foundations are implementation inputs, not closed gaps.

The first implementation wave should finish the chat workbench: header,
timeline, composer, and a real tabbed right panel. The second should finish the
shell: project/thread navigation, settings primitives, command palette,
dialogs, menus, toasts, and narrow-window adaptation. Cosmetic token matching
without those two waves would make the products look momentarily closer while
leaving their interaction models far apart.

## Why this document lives in `docs/teardowns`

The existing
[full gap analysis](./2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md)
already establishes T3 Code as a read-only product reference and compares its
runtime, orchestration, worktree, remote, mobile, release, and provider
capabilities with OpenAgents Desktop. This document is its UI-specific
companion. Keeping both analyses together avoids creating a second T3 status
authority under `docs/sol/`.

This document is planning evidence for Sol, not independent roadmap authority.
The current [Sol master roadmap](../sol/MASTER_ROADMAP.md), product contracts,
invariants, current source, and admitted work packets still decide what may be
implemented and in what order.

## Scope, snapshots, and method

This is a source-led UI audit with screenshot corroboration, not a marketing
comparison. The two exact snapshots are:

| Product    | Revision                                   | Snapshot note                                                                                                                                                                                       |
| ---------- | ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T3 Code    | `8b5469863ae1dd696e696de30240ec3da607962d` | Upstream `pingdotgg/t3code` `main`, fetched 2026-07-17. The repository is locally stored at `projects/repos/t3code`; the user's “t3chat” reference resolves to this current T3 Code product source. |
| OpenAgents | `b1dd27135524f546846b4cf46156cfb97643d1d2` | `OpenAgentsInc/openagents` `origin/main`, including the 2026-07-17 user-message/sidebar/settings alignment sequence and Queue/Steer control lowering.                                               |

Primary inspected sources:

- T3: `apps/web/src/components/Sidebar.tsx`, `AppSidebarLayout.tsx`,
  `ChatView.tsx`, `chat/ChatHeader.tsx`, `chat/MessagesTimeline.tsx`,
  `chat/ChatComposer.tsx`, `RightPanelTabs.tsx`, `CommandPalette.tsx`,
  `components/settings/*`, `components/ui/*`, `rightPanelStore.ts`, and
  `index.css`.
- OpenAgents: `apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx`,
  `react-timeline.tsx`, `react-composer.tsx`, `react-review.tsx`, `settings.ts`,
  `shell.ts`, `packages/ui/src/workbench/*`, and
  `packages/ui/src/desktop-workbench.css`.
- Visual corroboration: T3's checked-in current product screenshot and
  OpenAgents' component-library/sidebar/settings captures. The OpenAgents
  captures predate part of the 2026-07-17 convergence sequence, so current
  source—not those older pixels—is authoritative for closed-gap claims.

The comparison asks four questions for every surface:

1. Is the component visibly mounted in the current React product?
2. Does it cover the same user job and state family as T3's component?
3. Does it have comparable interaction, accessibility, performance, and
   responsive behavior?
4. Which OpenAgents-specific semantics or safety properties must survive any
   visual port?

### Status legend

| Mark  | Meaning                                                                                        |
| ----- | ---------------------------------------------------------------------------------------------- |
| **A** | Aligned enough to use as the house precedent for the next ports                                |
| **N** | Near: same mounted job and shape, with bounded visual/state gaps                               |
| **P** | Partial: a mounted component exists, but T3's composition or state family is materially deeper |
| **H** | Hidden/foundation: code exists outside the current mounted React product path                  |
| **M** | Missing from the current mounted React product path                                            |
| **D** | Deliberate OpenAgents difference that should be preserved                                      |

## The 2026-07-17 convergence sequence

Three commits are the concrete precedent for this program.

### `922fce1a25` — port T3 user-message chrome

This commit replaced the generic shared message row for user messages with the
T3 composition from `MessagesTimeline.tsx` at T3 revision `fdca1547`:

- user messages align to the right;
- the bubble caps at 80% width;
- the bubble uses a 16px radius and 12px padding;
- timestamp and actions live in a separate row below the bubble;
- the action row appears on hover or focus-within; and
- copy has a temporary success state.

The deliberate substitution was color: OpenAgents kept its Khala raised-blue
surface instead of T3's neutral secondary surface.

### `f8538cd550` — restore exact bubble geometry

The follow-up moved the geometry out of Tailwind utilities and into the shared
workbench stylesheet because the OpenAgents workbench resets its radius scale
to zero. It also normalized timestamps and hides epoch sentinels rather than
showing `1970` as user-visible metadata.

This is the strongest model for future ports: copy the semantic composition,
put stable geometry under the shared component stylesheet, retain OpenAgents
tokens, and add tests for the exact interaction and bad-data states.

### `e8a981c0ee` — align sidebar and settings with T3

This commit made several useful shell-level changes:

- the ordinary rail is bounded to recent chats rather than mixing in workspace
  administration and pagination controls;
- the settings rail swaps recent chats for `General`, `Codex CLI`, optional
  `Privacy`, and `Account` destinations;
- the settings footer becomes `Back`;
- section selection scrolls the settings page, respecting reduced motion;
- the sidebar footer shows provider rate limits, not per-turn token counts;
  and
- background chat projection no longer pulls the user out of Settings.

That closes the first-order visual mismatch, not full sidebar/settings parity.
T3 settings has routed section identity, six top-level sections, sign-in/account
chrome, compact row primitives, reset affordances, conflicts/errors, skeletons,
and extensive per-section state. T3's ordinary sidebar is project-grouped and
status-rich rather than a flat recent-chat list.

## Top-level UI scorecard

| UI family           | T3 Code                                                                                                                | OpenAgents Desktop                                                                            | Status  | Material gap                                                                                          |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | :-----: | ----------------------------------------------------------------------------------------------------- |
| App frame           | Resizable/off-canvas sidebar, responsive main/right-panel layout, native titlebar accommodation                        | Fixed 248px rail, 760px minimum workbench, chat/settings mounted                              |  **P**  | Structural responsive behavior and pane composition remain different.                                 |
| Visual identity     | Neutral light/dark/system product theme, DM Sans, restrained blue, subtle grain                                        | Fixed dark Khala/Protoss blue, sans + mono instrumentation, square workbench grammar          |  **D**  | Preserve OA identity; port hierarchy and geometry rather than T3's palette wholesale.                 |
| Sidebar             | Projects → threads, nested status, sorting/grouping, drag reorder, multi-select, menus, updates                        | Recent chats, search, rename, settings mode, rate limits                                      |  **P**  | Recent-chat styling is closer, but the information architecture and row states are not.               |
| Chat header         | Thread title plus scripts, Open In, Git/commit/push actions, responsive action layout                                  | Title, lifecycle, optional repository label                                                   |  **P**  | No workbench action cluster or context-aware collapse.                                                |
| User message        | Rich bubble, attachments, contexts, collapse, copy, revert, timestamp                                                  | T3-style bubble, copy, timestamp, details                                                     |  **N**  | Geometry is aligned; message-content and action states are not.                                       |
| Assistant message   | Rich markdown, copy/meta, links/files/skills, turn grouping                                                            | Safe markdown plus details; strong typed cards around it                                      |  **P**  | OA lacks T3's content integrations and turn-level navigation.                                         |
| Transcript engine   | Virtualized rows, stable anchoring, turn folds, work folds, minimap                                                    | MessageScroller, paging, stable item keys, no equivalent virtualization/minimap               |  **P**  | Long-thread navigation and render-budget behavior remain large gaps.                                  |
| Work/tool activity  | Compact icon rows, foldable grouped work, in-place status, expandable output                                           | Typed command/file/tool/protocol/agent cards and work groups                                  | **N/P** | OA has broader typed variants; T3 has calmer default density and better transcript integration.       |
| Changed files       | Inline tree with stats, collapse-all, file navigation, rich diff                                                       | File-change cards and hidden read-only review foundation                                      | **P/H** | No mounted changed-file tree/right-panel flow in the React shell.                                     |
| Composer            | Lexical editor, slash/mention menus, many context types, images, modes, approvals, plan follow-up, responsive collapse | Lexical editor, images, provider/model/reasoning/permission, Full Auto, explicit Queue/Steer  | **P/D** | OA semantics are stronger; contextual nodes, menus, responsive treatment, and footer polish trail T3. |
| Decisions           | Inline composer panels for approval/input/plan plus stateful actions                                                   | Modal dialog with typed approval/question/plan cards                                          | **P/D** | OA is explicit but interrupts flow; progressive inline treatment is missing.                          |
| Right panel         | Tabbed diff/files/file/terminal/plan/browser surfaces, add/close/context menu, maximize/layout controls                | Not mounted in current React workbench                                                        |  **M**  | Largest visible product gap.                                                                          |
| Diff/review         | Worker-backed rich diffs, trees, annotations, per-turn/open-panel flow                                                 | Exact bounded read-only review code exists but is explicitly absent from core React workbench |  **H**  | Must be mounted and composed without weakening read-only authority.                                   |
| Terminal            | Persistent xterm surfaces, tabs/drawer, labels, resize, context insertion                                              | Typed terminal foundations outside mounted React shell                                        |  **H**  | No visible terminal workbench.                                                                        |
| Files/editor        | Project tree, file tabs, read/edit/save, annotations                                                                   | Typed browser/editor foundations outside mounted React shell                                  |  **H**  | No visible file panel or tab flow.                                                                    |
| Browser preview     | URL/local-server discovery, browser chrome, device sizing, automation cursor/recording                                 | No mounted equivalent                                                                         |  **M**  | Entire visual-development loop is absent.                                                             |
| Plan                | Proposed-plan transcript row plus right-panel plan and follow-up actions                                               | Typed plan card and modal accept/change/replan                                                |  **P**  | No persistent side-by-side plan surface.                                                              |
| Command palette     | Search, add/open project, local/remote sources, commands, navigation                                                   | Actions plus six recent sessions and keyboard footer                                          |  **P**  | OA is usable but not a whole-workbench launcher.                                                      |
| Settings            | Routed six-section IA with reusable rows, full state coverage                                                          | One scrolling page, four rail anchors, shallow Codex/privacy/account content                  |  **P**  | Layout has converged; component/state depth has not.                                                  |
| Overlays            | Broad Base UI/shadcn family: menus, popovers, selects, sheets, dialogs, toasts, tooltips                               | Fourteen local primitives and several one-off compositions                                    |  **P**  | Missing primitive coverage produces inconsistent local solutions.                                     |
| Empty/loading/error | Surface-specific empty states, skeletons, disabled explanations, banners, toasts                                       | Honest loading/error copy, sparse surface-specific treatment                                  |  **P**  | Needs a systematic state pass per component family.                                                   |
| Accessibility       | Broad keyboard/tooltip/ARIA work, but very large components still carry risk                                           | Strong explicit labels, focus, reduced motion in aligned slices                               |  **N**  | OA should preserve its stronger explicit semantics while adding T3's keyboard breadth.                |
| Narrow windows      | Off-canvas/sidebar sheets, media/container queries, collapsed composer, right-panel sheet                              | Overlay rail exists, but root minimum is 760px and most workbench layout is desktop-fixed     |  **P**  | Responsive behavior is not yet a complete product mode.                                               |
| Theme modes         | Light, dark, system                                                                                                    | One fixed dark theme by contract                                                              |  **D**  | Do not create light mode merely for parity.                                                           |

## 1. Design-system and visual-language differences

### 1.1 Palette: converge roles, not colors

T3's light theme is white/neutral with an OKLCH blue primary. Its dark theme is
near-neutral black with low-alpha white layers, a blue primary, and conventional
red/amber/emerald semantic colors. OpenAgents is intentionally deeper blue:
background `#05070d`, surface `#0b1220`, raised surface `#141f36`, and accent
`#3b82f6` are pinned by the Desktop UX contracts.

**Disposition: D.** Keep Khala. The T3 ports should answer questions such as
“which layer is raised?”, “how strong is selected?”, and “how does destructive
state read?” using OpenAgents semantic tokens. They should not replace the blue
surface hierarchy with neutral gray.

The gap is role consistency. T3's 42 local UI primitives all speak the same
background/card/popover/secondary/muted/accent/border/ring vocabulary.
OpenAgents has 14 local shadcn primitives plus 28 shared workbench component
files, but many workbench recipes live in one 2,289-line stylesheet. Before a
broad port, OpenAgents should make sure hover, selected, raised, overlay,
disabled, warning, error, success, and focus roles resolve the same way across
both primitive families.

### 1.2 Typography: hierarchy is the gap

T3 uses DM Sans for product UI and a conventional mono stack for code. It
mostly keeps labels between 11–14px, uses medium/semibold weights for titles,
and reserves uppercase tracking for quiet section labels.

OpenAgents combines a sans UI face with mono instrumentation and makes more
frequent use of uppercase, tracking, status tags, bracketed labels, and key/value
rows. That language suits runtime evidence, but applied everywhere it makes
ordinary navigation and chat feel like an operations console.

**Recommendation:** retain OpenAgents fonts, but port T3's hierarchy:

- ordinary navigation, chat prose, titles, settings rows, and controls use the
  sans voice;
- mono stays with code, paths, commands, exact values, and runtime evidence;
- uppercase/tracking is limited to section or state labels, not general body
  copy; and
- the default transcript is quiet enough that typed evidence cards can become
  prominent only when needed.

### 1.3 Radius and geometry are an unresolved deliberate difference

T3's base radius is 10px. It uses small rounded rows/controls, 16px user
messages and settings cards, and larger radii selectively for composer and
floating surfaces. OpenAgents resets every shared workbench radius token to
zero, then makes the T3 user bubble a deliberate 16px exception.

That exception proves exact T3 geometry cannot be obtained by component markup
alone; OpenAgents' root token override erases it. Repeating the user-bubble
approach one component at a time would create an undocumented patchwork.

**Decision needed before bulk implementation:** define a semantic radius
taxonomy. A reasonable target is:

- window edges, split panes, dense data tables, evidence ledgers: square;
- interactive rows, menus, inputs, buttons, tabs: small radius;
- message bubbles, composer, dialogs, setting groups: medium radius;
- pills only for true tags, short statuses, and binary modes.

This preserves OpenAgents' engineered character without forcing every product
control into the same square geometry.

### 1.4 Surface material and motion

T3 uses a very subtle baked-in grain and a blurred/translucent composer when
supported, with solid fallbacks. It also uses 150–200ms transitions,
AutoAnimate for project/thread lists, duty-cycled status motion, and explicit
reduced-motion fallbacks.

OpenAgents is mostly solid and border-led, with quantized block animation for
working state and reduced-motion coverage in the shared stylesheet.

**Recommendation:** port T3's state motion and list continuity, not decorative
material by default. The floating composer may justify a restrained translucent
layer, but grain and blur are not prerequisites for component parity. Measure
idle GPU cost before accepting either.

## 2. App shell and navigation

### 2.1 Root frame

T3's root shell supports:

- a sidebar whose width persists and can be resized while preserving a 640px
  minimum main content width;
- off-canvas behavior and a global sidebar toggle;
- macOS traffic-light and Window Controls Overlay insets;
- a responsive split between chat and a right-panel workbench;
- a right-panel sheet when inline width is unavailable; and
- safe-area and viewport handling shared by web and Electron.

OpenAgents has good macOS titlebar inset handling, a collapsible/overlay rail,
and a scrim, but `.oa-react-workbench` is a fixed `248px + 1fr` grid with a
760px minimum width. Its third column appears only for a review flag, while the
current React shell does not mount `ReviewSurface` at all.

**Gap:** shell collapse exists; responsive workbench reflow does not.

**Target:** a single frame contract with persisted sidebar width, compact rail,
inline right panel above a measured width, sheet right panel below it, and no
hard 760px overflow floor.

### 2.2 Ordinary sidebar

The recent OpenAgents simplification moves the visual rhythm toward T3, but the
products still encode different objects.

T3 sidebar rows model:

- logical projects grouped by repository/path policy;
- expandable thread children;
- project favicons and local/remote/container identity;
- working/completed/error/PR/terminal/preview status;
- user-selected thread preview count;
- project and thread sort policy;
- manual drag ordering;
- multi-selection and native context menus;
- inline rename/archive/delete confirmation;
- per-project new-thread entry; and
- update/provider notices in the footer.

OpenAgents rows model:

- a flat, bounded recent-chat list;
- selected and working state;
- relative time or shortcut hints;
- search across the history catalog;
- local chat rename; and
- rate-limit telemetry below Settings.

**Status: P.** The OpenAgents list is now calmer and should stay bounded, but it
does not yet communicate the project/worktree/session hierarchy that makes a
coding workbench understandable.

**Port target:** project headers with nested recent chats, compact activity
state, new chat per project, and a bounded overflow path. Do not import T3's
full menu breadth until the corresponding typed authority exists.

### 2.3 Settings-mode rail

The overall interaction is now aligned: entering Settings replaces chat
navigation and the footer becomes Back. Remaining gaps:

- T3 has routed, reload-stable section identity; OA keeps selection in local
  component state and scrolls anchors;
- T3 has General, Keybindings, Providers, Source Control, Connections, and
  Archive; OA has General, Codex CLI, optional Privacy, and Account;
- T3 closes its mobile sidebar after section selection; OA's rail behavior is
  not yet section-route aware;
- T3 footer carries sign-in/account state; OA account state lives in page
  content; and
- T3's current section and browser history are the same state, while OA's are
  only visually synchronized.

**Target:** keep the current labels appropriate to OpenAgents, but give each
section durable route/state identity and make narrow-window selection dismiss
the rail.

## 3. Conversation header and status chrome

T3's 52px header is a compact workbench toolbar. It contains the thread title,
project scripts, Open In editor choice, and Git actions such as commit/push.
Container queries adjust gaps and reserved native-control space. Additional
provider/error banners appear immediately below without changing the titlebar
contract.

OpenAgents' header is also 52px and correctly uses a draggable title region,
but it only presents title, lifecycle, and optional repository label. This
makes the main pane read as a chat viewer rather than an active coding
workspace.

**Status: P.** Geometry is close; capability density is not.

**Target order:**

1. project/worktree/branch disclosure;
2. one compact “Open” action for editor/file manager where supported;
3. a Review/right-panel toggle;
4. typed source-control actions only after mutation authority is admitted; and
5. an overflow menu at narrow widths rather than wrapping the header.

Provider availability, version mismatch, disconnect, and update state should
use a consistent banner/notice family rather than several independent alert
recipes.

## 4. Transcript and messages

### 4.1 User messages: near, not complete

The recent commits align the base row. T3 still supports additional content
inside and around that bubble:

- image grids with expandable previews;
- preview-annotation cards;
- DOM element-context chips;
- terminal-context inline chips;
- review-comment cards with embedded diff;
- skill-aware inline rendering;
- collapse/expand after eight lines or 600 characters;
- checkpoint revert with turn-count disclosure; and
- message metadata/copy behavior integrated with scroll anchoring.

OpenAgents supports image attachments at compose time and safe Markdown in the
message, plus Details and Copy. It does not project T3-equivalent contextual
objects inside the mounted user bubble.

**Status: N.** The visual primitive is aligned. The content grammar is the gap.

### 4.2 Assistant messages

T3 renders assistant prose as the quiet default layer and gives it skill/file
link awareness, bounded markdown styling, copy/meta actions, and turn-aware
changed-file sections. Tool activity is visually subordinate until expanded.

OpenAgents has safe Markdown and a useful typed family of command, file,
protocol, tool, approval, plan, notice, queue, reasoning, and subagent cards.
This is a real strength. The issue is that many variants use similarly strong
borders, uppercase tags, and dense instrumentation, so the transcript can lose
its reading hierarchy.

**Target:** keep OpenAgents' typed card breadth, but adopt T3's three-level
hierarchy:

1. assistant prose is primary;
2. compact work rows are secondary;
3. exact output, arguments, diffs, receipts, and protocol fields appear on
   disclosure or when user action is required.

### 4.3 Turn grouping and work folds

T3 derives stable rows from messages and activities, folds settled turns,
keeps an interrupted active turn expanded, groups work entries, and preserves
scroll position when a fold changes height. Its compact rows update in place
and disclose full output only when requested.

OpenAgents merges matching tool lifecycle records and has `DesktopWorkGroup`
and `DesktopWorkEntry`, but it lacks equivalent turn-level folding and
anchor-preserving expansion in the mounted timeline.

**Status: P.** Port T3's behavior, not necessarily its internal state stores:
stable turn IDs, one row per evolving activity, default collapse rules, and
height-change anchor compensation belong in the OpenAgents projection.

### 4.4 Long-thread navigation and virtualization

T3 uses `@legendapp/list`, stable-row reuse, explicit anchor bookkeeping, and a
keyboard-accessible transcript minimap that previews user and assistant text.
The minimap becomes persistent only when the viewport has room. T3 also uses
worker-backed diff parsing and bounded render caches.

OpenAgents pages history and uses a MessageScroller with stable keys and a
scroll-to-latest control. Its tests cover large synthetic corpora, but the
current renderer maps all projected records in the page and has no minimap or
equivalent turn navigator.

**Status: P; priority: high.** Before richer cards increase row weight, add
measured virtualization, stable anchoring, and a long-thread navigator. A
simple turn index could precede the full minimap, but it must remain keyboard
operable and must not obscure content at narrow widths.

### 4.5 Working, error, and completion state

T3's working row includes elapsed time and its activity rows resolve to clear
success, failure, warning, or neutral endings. OpenAgents' quantized Working
indicator is distinctive and should remain, but completion/errors can be split
between cards, notices, status banners, and Details affordances.

**Target:** preserve the quantized OA motion while standardizing one lifecycle
vocabulary across timeline rows, sidebar rows, header notices, and composer
state. A status color/icon/label must mean the same thing in each location.

## 5. Composer

### 5.1 What OpenAgents already does better

OpenAgents makes Queue and Steer explicit while a turn is running, exposes a
durable queued-message panel with edit/remove states, and gives Full Auto a
first-class toggle and running state. T3's composer supports drafting during a
turn but replaces its pointer primary action with Stop and historically exposes
a less explicit second-send seam.

**Disposition: D.** Do not trade explicit Queue/Steer for visual parity. T3's
composer should be a layout and interaction-quality reference around the
OpenAgents concurrency model.

### 5.2 Footer composition

Both products now use Lexical. T3's footer presents provider/model, effort,
Build/Plan, runtime access, plan-panel state, context meter, and primary action
as compact controls with tooltips and menus. It moves less-frequent controls
into a compact menu and collapses the entire composer at narrow widths.

OpenAgents presents image, provider, model, reasoning, permission, Full Auto,
Queue/Steer mode, availability, Stop, and Send in one horizontal bar. It cycles
several values on click rather than opening a discoverable menu.

**Status: P.** The current bar will crowd as capabilities grow.

**Target:**

- keep only provider/model summary, Full Auto, current Queue/Steer mode, and
  primary action visible at ordinary width;
- move image/context acquisition, reasoning, permission, and less-common
  actions into coherent popovers/menus;
- use real selection surfaces instead of click-to-cycle for model, provider,
  reasoning, and permission;
- show the consequence of permission modes in menu copy;
- add a compact/narrow composer mode; and
- maintain a stable primary-action position through state changes.

### 5.3 Context objects and command menus

T3's composer can carry images, files, folders, skills, slash commands,
terminal contexts, preview annotations, element contexts, review comments, and
pending user-input/approval/plan panels. It has dedicated chips/cards and an
empty-state message for each command-menu context.

OpenAgents currently mounts images and plain rich text. Its Lexical editor
explicitly leaves room for file, skill, and terminal DecoratorNodes, but those
are not present in the mounted composer.

**Status: P.** Add contexts in this order:

1. file/folder mentions tied to bounded workspace search;
2. skills and commands from the admitted Codex capability catalog;
3. review/diff context from the read-only review surface;
4. terminal context once the terminal is mounted; and
5. preview/element annotations after the browser surface exists.

Every chip needs source identity, remove behavior, overflow treatment, tooltip
detail, serialization, restart behavior, and a transcript representation.

### 5.4 Images

OpenAgents has good acquisition, count/size feedback, drag/drop, image-only
turns, thumbnails, and removal. T3 adds expanded image dialogs, persistence
warnings, preview-annotation linkage, and richer transcript rendering.

**Status: N.** Add click-to-expand and transcript parity first; preview
annotations depend on the browser workbench.

### 5.5 Approval, question, and plan decisions

T3 generally keeps pending decisions in the composer stack, close to where the
next user message will be written. OpenAgents uses a modal dialog and correctly
models tool approval, plan review, multi-select, submission, refusal, read-only
unavailability, and resolved states.

**Status: P/D.** Preserve OpenAgents' state semantics but reduce interruption:

- simple approvals and one-question prompts should be inline above the
  composer;
- modal treatment should be reserved for destructive/high-risk or lengthy
  review;
- closing an inline disclosure must not imply a decision; and
- unresolved decisions need a persistent attention marker in sidebar/header
  so they cannot disappear below the fold.

## 6. The missing right-panel workbench

This is the largest UI gap.

T3's right panel is not one “review drawer.” It is a reusable surface manager
with these tab types:

- browser preview;
- terminal;
- files root;
- individual file;
- diff; and
- plan.

It supports tab activation, pending indicators, close, middle-click close,
native context-menu actions, close others, close to right, close all, copy file
path, an add-surface menu, an explanatory empty state, layout controls,
maximize, inline mode, and sheet mode. Per-thread panel state survives changes
of active surface.

OpenAgents' current React workbench mounts none of that. `ReviewSurface`,
workspace browser/editor, terminal workspace, and older typed workspace views
exist, but the current React test suite explicitly asserts that repository
review is absent from the core workbench.

### 6.1 Recommended OpenAgents surface manager

Create a typed right-panel surface union owned by the existing application
state/intent boundary, initially:

```text
review-summary | diff-file | files | file | terminal | plan | preview
```

The first mounted wave should be read-only:

1. `review-summary` — current bounded status and changed files;
2. `diff-file` — exact bounded diff with secret/binary/oversize refusals;
3. `files` and `file` — bounded browser and editor/read view;
4. `plan` — the current proposed plan; and
5. `terminal` — only after PTY lifecycle/replay authority is ready.

Browser preview follows process/port and browser automation authority. Git
mutation actions follow their own admission; they are not implied by mounting
read-only review.

### 6.2 Visual target

Copy T3's pane mechanics closely:

- 52px tabbar aligned with the chat header;
- small icon + truncated label tabs;
- active background, hover background, pending marker, hover/focus close;
- scrollable tab strip;
- persistent active tab per chat;
- plus menu with disabled reasons;
- resizable split with reasonable min widths;
- inline above the wide breakpoint and right sheet below it; and
- keyboard shortcuts for panel toggle, next/previous tab, close tab, and
  maximize.

Use Khala surface/accent/focus roles and the approved icon catalog. Do not copy
T3's ambient filesystem/process capability into the renderer.

## 7. Diff, files, terminal, preview, and plan surfaces

### 7.1 Diff and changed files

T3 offers an inline changed-file tree with aggregate stats, directory folding,
Collapse/Expand all, and View diff. Its right-panel diff uses a worker pool and
`@pierre/diffs`; comments can be attached back to the composer.

OpenAgents has typed file-change cards and a bounded exact-diff review
foundation with unusually strong refusal copy for secret, binary, stale,
oversize, and unsafe states.

**Target:** combine them. T3 supplies the tree, navigation, rich rendering,
and annotation flow; OpenAgents supplies authority, caps, and honest absence.
Avoid presenting a raw `<pre>` as the final diff experience.

### 7.2 Files

T3 supports a project tree, file tabs, syntax-aware preview/edit, save
coordination, revision handling, annotations, and path-aware navigation.
OpenAgents has browser/editor/save modules but no mounted React panel.

**Target:** first ship read/search/open with virtualized directory rows and
clear dirty/read-only state. Editing and save must consume existing one-shot
authority and show conflict/refusal states in the same panel, not through
generic notices.

### 7.3 Terminal

T3 uses xterm-grade persistent sessions, thread-scoped labels, multiple
terminals, resize, running-state indicators, drawer/panel presentation, and
composer-context insertion. OpenAgents has typed terminal state and host
intents but no equivalent mounted surface.

**Target:** a tabbed terminal panel with session identity, replay boundary,
running/exited state, restart/close, copy, search, and explicit context
attachment. Do not make a generic terminal automatically part of every chat.

### 7.4 Browser preview

T3 provides local-server discovery, URL navigation, browser chrome, device
sizing, zoom, resize handles, multiple tabs, failure states, an automation
cursor, recording, and preview annotations.

OpenAgents has no mounted counterpart. This is not a component-skin port; it
requires process, port, webview, automation, evidence, and permission design.

**Status: M.** Treat the browser as the last right-panel surface in the first
parity program, not as a prerequisite for mounting review/files/plan.

### 7.5 Plan

OpenAgents' plan card and Accept/Request changes/Replan model are stronger than
a plain markdown view. T3 adds a persistent side panel and explicit follow-up
composition.

**Target:** mount the exact current plan in a panel while keeping its review
actions in the existing typed decision model. The transcript row should open
the panel, and plan revisions should update one tab rather than spawn new tabs.

## 8. Command palette, menus, dialogs, and toasts

### 8.1 Command palette

OpenAgents' palette is a sound first version: actions, recent sessions,
availability filtering, shortcuts, keyboard help, and live result count. T3's
palette is a workbench launcher with project addition, local browsing, remote
sources, repository providers, and deeper navigation.

**Status: P.** Expand only as mounted surfaces arrive:

- search all current project chats;
- open/switch right-panel surfaces;
- open file and symbol search;
- select project/worktree;
- run admitted skills/commands; and
- expose disabled results with reasons instead of silently filtering every
  unavailable action.

### 8.2 Primitive coverage

T3's primitive directory contains 42 components including autocomplete,
combobox, collapsible, field/fieldset/form, menu, popover, radio group, select,
skeleton, switch, table, toast, toggle group, and tooltip. OpenAgents' local
desktop primitive directory contains 14 components. The shared workbench adds
domain components, but it does not replace missing interaction primitives.

This gap already shows in local composition: the chat rename dialog is manually
constructed while the app also owns a Dialog primitive; model/provider choices
cycle on click instead of using Select/Popover; settings uses action buttons
where Switch/Radio/Field patterns would communicate state more naturally.

**Target:** add the smallest shared primitive set required by the planned
ports—menu, popover, select/combobox, field/fieldset, switch, collapsible,
skeleton, toast, tabs, and resizable panel—then use it consistently.

### 8.3 Dialogs and sheets

T3 uses responsive dialogs/sheets and supplies title, description, focus,
escape, overlay, and disabled-state explanations through shared primitives.
OpenAgents has good Dialog and Sheet components, but several features still
compose their own overlays or fixed alerts.

**Target:** one overlay stack and semantic z-index scale. Rename, updates,
decisions, image preview, right panel, and future connection flows should not
each invent backdrop, focus return, and width behavior.

### 8.4 Toasts and banners

T3 distinguishes transient toasts, persistent thread banners, sidebar update
pills, and inline composer banners. OpenAgents uses alerts/notices and typed
failures but has no equally coherent placement policy.

**Target placement policy:**

- toast: completed transient action with no remaining decision;
- banner: persistent connection/provider/thread condition;
- inline field/card error: action-specific failure;
- timeline notice: durable event worth retaining;
- modal: required high-attention decision only.

## 9. Settings

The recent rail port should be followed by the content primitives, not more
one-off sections.

T3's settings system has:

- a centered max-width page container;
- named `SettingsSection` cards;
- compact `SettingsRow` title/description/status/control composition;
- per-setting reset affordances;
- skeleton and empty states;
- responsive control stacking;
- theme, density, timestamp, confirmation, update, and behavior preferences;
- keybinding search/edit/conflict/when-expression UI;
- provider instance, model, authentication, environment, and accent controls;
- source-control discovery and refresh policy;
- local/remote connection and pairing management;
- diagnostics tables/charts/process actions; and
- archive management.

The mounted OpenAgents settings page has a branded frame, Refresh, Codex CLI
version/update status, optional usage sharing, and Codex account identity. It
does not mount the broader preference, keybinding, diagnostics, update, ACP
provider, or workspace controls that exist in older typed settings views and
contracts.

**Status: P.** Recommended content order:

1. General: density, font scale, reduced motion, notification/update
   preferences already owned by the preferences contract;
2. Codex CLI: current version, update, release notes, model/defaults, and
   capability status;
3. Providers: only admitted Codex/ACP peers, with experimental/matrix status
   stated honestly;
4. Keybindings: searchable bindings, conflicts, reset;
5. Privacy: local usage sharing and data-boundary explanations;
6. Account: local provider identity and optional OpenAgents link; and
7. Diagnostics: public-safe export, process/runtime health, and exact refresh
   state.

Use T3's row density and responsive control alignment, but keep OpenAgents'
privacy copy, redaction, and release-claim boundaries.

## 10. Empty, loading, unavailable, and error states

T3 has targeted empty states for no thread, no project, no right-panel surface,
no source-control discovery, no remote environments, preview unavailable, and
more. Disabled controls usually carry an explanatory tooltip. Large settings
areas use skeleton rows instead of a lone status sentence.

OpenAgents is honest but sparse: “Loading workbench…”, “The workbench stopped
updating”, “Checking provider accounts…”, and similar copy is correct but does
not always preserve the final layout or teach the next action.

For every ported component, require these states:

| State                | Required presentation                                              |
| -------------------- | ------------------------------------------------------------------ |
| Initial loading      | Skeleton matching the final geometry; no layout jump               |
| Empty                | Explain what belongs here and offer one safe next action           |
| Disabled             | Keep the control visible when discoverability matters; explain why |
| Offline/disconnected | Preserve local content, name stale state, offer retry              |
| Permission refused   | Name the refused capability and safe recovery                      |
| Partial data         | Render admitted data and label exact absence; never fabricate      |
| Error                | Localize to the failed action; preserve retry input/context        |
| Success              | Quiet confirmation; do not leave permanent celebratory chrome      |

## 11. Responsive behavior

T3 uses media and container queries throughout:

- off-canvas sidebar on small screens;
- narrow-window header action reduction;
- collapsed composer mode;
- right-panel inline/sheet switch;
- settings controls stack vertically;
- safe-area-aware composer/titlebar;
- hover-only behavior is guarded by pointer capability; and
- minimap persists only when width permits.

OpenAgents has an overlay rail and a few 720/900/1120px rules, but the fixed
root minimum and desktop-oriented component widths mean it is adaptive in
parts rather than responsive as a whole.

**Target breakpoint behavior:**

- wide: sidebar + chat + right panel;
- standard desktop: sidebar + chat, right panel toggle/sheet;
- narrow desktop: collapsed sidebar toggle + chat, compact composer/header;
- minimum supported window: no horizontal page overflow, all primary actions
  reachable, overlays constrained to viewport.

This is a Desktop requirement even if OpenAgents mobile remains a separate
renderer.

## 12. Accessibility and keyboard behavior

OpenAgents should copy T3's breadth while preserving its own stronger explicit
semantics.

Already strong in OpenAgents:

- accessible names on icon controls;
- hover actions also reveal on focus-within;
- focus return after rename;
- reduced-motion-aware settings scrolling;
- live regions on relevant status;
- read-only/refusal copy; and
- semantic dialogs for decisions and updates.

Remaining gaps or risks:

- hover-only metadata must remain discoverable on touch and keyboard;
- the flat recent list needs full arrow/selection/context-menu behavior before
  gaining project nesting;
- right-panel tabs need tab semantics and keyboard traversal;
- click-to-cycle controls are not discoverable selection controls;
- virtualized rows must retain screen-reader order and focus;
- minimap/turn navigation needs a non-pointer path;
- large user messages need collapse state and `aria-expanded`;
- every animated/pulsing status needs the explicit preference override as well
  as OS reduced motion; and
- the current manually composed rename dialog should be reconciled with the
  shared Dialog focus/overlay contract.

Accessibility parity is part of component parity, not a final polish phase.

## 13. Performance and implementation shape

T3's UI maturity comes with very large components: `Sidebar.tsx` is about
3,754 lines, `ChatComposer.tsx` about 2,565, `MessagesTimeline.tsx` about 2,057,
and `CommandPalette.tsx` about 1,888 at the audited snapshot. OpenAgents should
copy behavior, not those file boundaries.

OpenAgents should preserve:

- Effect Native/application state and intent authority;
- one typed projection into React;
- bounded native data and explicit refusal states;
- shared `packages/ui` domain components; and
- deterministic fixture coverage.

For each T3-derived component family:

1. define the typed view state and intents;
2. build the shared visual component with no ambient host authority;
3. adapt current Desktop state into that component;
4. add every visible state to the component fixture catalog;
5. test keyboard, focus, reduced motion, overflow, and narrow width;
6. measure long-list and streaming behavior; and
7. only then replace the existing mounted composition.

Do not add Zustand, TanStack Router, or Effect Atom merely because T3 uses
them. They solve T3's ownership problem. OpenAgents already has a different
authority boundary.

## 14. Component-by-component adoption ledger

| Priority | Component                 | Current OA state           | T3 behavior to adopt                                          | OpenAgents behavior to preserve                  | Closure proof                                                          |
| -------: | ------------------------- | -------------------------- | ------------------------------------------------------------- | ------------------------------------------------ | ---------------------------------------------------------------------- |
|       P0 | Root split-pane frame     | Fixed rail + chat/settings | Resizable sidebar; inline/sheet right panel; persisted layout | Closed typed intents; Khala theme                | Wide/standard/narrow visual fixtures and keyboard resize/toggle tests  |
|       P0 | Right-panel tab strip     | Missing                    | Tabs, active/pending, add/close/context menu, maximize        | No ambient renderer capability                   | Fixture for every surface/state plus focus/close behavior              |
|       P0 | Review summary            | Hidden foundation          | Changed-file tree, stats, file navigation                     | Exact caps/refusals; read-only boundary          | Mounted review from real bounded fixture; secret/binary/oversize cases |
|       P0 | Diff file                 | Hidden/raw foundation      | Rich worker-backed diff, navigation, annotations              | Exact source digest and bounded output           | Large diff perf, keyboard navigation, refusal fixtures                 |
|       P0 | Transcript engine         | Paged non-virtual list     | Virtualization, stable anchors, turn folds, minimap/index     | Loss-accounted history and stable refs           | Huge-thread scroll/anchor/perf receipt                                 |
|       P0 | Composer footer           | Dense flat row             | Menus, responsive collapse, stable primary action             | Queue/Steer, Full Auto, typed admission          | All mode combinations at 3 widths                                      |
|       P0 | File/skill command menu   | Missing                    | Slash/mention search, grouped results, empty states           | Semantic capability selection; no string routing | Keyboard-first selection, no-results, unavailable cases                |
|       P1 | Project/thread sidebar    | Flat recent chats          | Nested projects/threads, status, sort/group, per-project new  | Bounded recents; no unauthorized menu actions    | Multi-project fixture with working/error/selected states               |
|       P1 | Chat header               | Title/lifecycle            | Project/worktree/branch, Open, Review, overflow               | Honest capability disclosure                     | Container-query fixtures and title truncation tooltip                  |
|       P1 | User message content      | Aligned bubble             | Context chips/cards, image grid, collapse, revert             | OA color, copy, Details, exact timestamps        | Long/image/context/revert visual states                                |
|       P1 | Assistant prose/actions   | Basic markdown + Details   | Quiet prose hierarchy, copy/meta, file/skill links            | Safe Markdown and typed event refs               | Markdown corpus and link/action tests                                  |
|       P1 | Work rows/groups          | Rich typed cards           | Compact default rows, in-place updates, anchor-safe folds     | Protocol/tool/agent specificity                  | Streaming → complete → failed fixtures without duplicate rows          |
|       P1 | Changed-files inline card | File cards                 | Tree, aggregate stats, collapse all, open diff                | Bounded exact data                               | Nested paths and large-tree perf fixture                               |
|       P1 | Files panel               | Hidden foundation          | Tree, file tabs, search, preview/edit state                   | One-shot filesystem authority                    | Read-only first, then save/conflict receipts                           |
|       P1 | Plan panel                | Card/modal only            | Persistent plan tab and transcript link                       | Accept/change/replan authority                   | Revision update and unresolved-decision tests                          |
|       P1 | Inline decision stack     | Modal                      | Composer-adjacent pending input/approval/plan                 | Explicit pending/refused/resolved states         | No lost decision after close/navigation/restart                        |
|       P1 | Settings primitives       | One-off sections           | Page/section/row/reset/skeleton/control patterns              | Privacy/redaction and honest release state       | Every setting state at desktop/narrow widths                           |
|       P1 | Settings routing          | Local scroll state         | Durable routed section identity                               | Current OA information architecture              | Back/forward/reload/anchor tests                                       |
|       P1 | Menus/popovers/selects    | Sparse                     | Shared menu, popover, select, combobox                        | Effect-owned value/intent state                  | Keyboard and focus conformance suite                                   |
|       P1 | Toast/banner policy       | Mixed alerts/notices       | Placement-specific feedback taxonomy                          | Durable timeline evidence when warranted         | Action matrix proving one correct feedback location                    |
|       P2 | Terminal panel            | Hidden foundation          | Persistent xterm tabs, state, resize, context attach          | Generation-owned bounded PTY authority           | Replay/resize/exit/restart/close fixtures and smoke                    |
|       P2 | Browser preview           | Missing                    | Tabs, navigation, devices, local servers, failures            | Scoped process/webview/automation evidence       | Local-app preview and permission/refusal proof                         |
|       P2 | Preview annotations       | Missing                    | Screenshot/element annotations into composer/timeline         | Typed public-safe evidence                       | Round-trip from preview selection to transcript                        |
|       P2 | Command palette breadth   | Actions + six recents      | Projects/files/surfaces/remote sources                        | Availability filtering and typed intents         | Search ranking, disabled reason, keyboard suite                        |
|       P2 | Update chrome             | Notice/dialog              | Sidebar pill, changelog tooltip, channel state                | Fail-closed release/update semantics             | Available/downloading/ready/error/restart states                       |
|       P2 | Empty/skeleton family     | Sparse                     | Surface-specific teaching states and skeletons                | Honest absence                                   | Fixture catalog coverage for every mounted surface                     |
|       P2 | Sidebar drag/multi-select | Missing                    | Manual ordering and multi-selection                           | Collision-safe project/worktree ownership        | Keyboard alternative and persisted order tests                         |
|       P3 | Light/system theme        | Missing                    | T3 theme modes                                                | Fixed Khala theme contract                       | **Do not implement for parity alone**                                  |
|       P3 | T3 grain/glass material   | Missing/limited            | Subtle grain and composer blur                                | Performance and restrained Khala identity        | Optional measured experiment, not closure requirement                  |

## 15. Ordered implementation program

### Wave 1 — finish the chat column

1. Define the semantic radius/control/state vocabulary needed for ports.
2. Recompose assistant prose and work groups into T3-like reading hierarchy.
3. Add long-user-message collapse, image expansion, and context slots.
4. Add virtualized/anchored long-thread rendering and a turn navigator.
5. Recompose the composer footer into menus while preserving Queue/Steer and
   Full Auto.
6. Add file/folder/skill/command contexts and inline decision presentation.
7. Expand the header with project/worktree/branch and right-panel entry.

### Wave 2 — mount the right-panel workbench

1. Add the typed surface manager and tab strip.
2. Mount read-only review summary and rich exact diff.
3. Mount files and file views.
4. Mount the current plan.
5. Add inline/sheet responsive modes, resize, maximize, and shortcuts.
6. Mount terminal only after its lifecycle/replay proof is current.

### Wave 3 — finish shell and settings

1. Move from flat chats to project-grouped recent threads.
2. Add compact activity/attention status and per-project new chat.
3. Add shared menu/popover/select/skeleton/toast primitives.
4. Convert settings anchors to durable section state and reusable rows.
5. Mount existing preferences, keybindings, provider, update, privacy,
   account, and diagnostics truth through the new settings grammar.
6. Complete narrow-window behavior across shell, header, composer, settings,
   and panels.

### Wave 4 — visual development loop

1. Add terminal/file context round trips.
2. Add local server/process discovery.
3. Mount browser preview with strict webview/process authority.
4. Add device controls, failure states, automation cursor/evidence, and
   annotations.
5. Feed preview annotations into composer and transcript.

## 16. Definition of UI parity

A component is not “in line with T3” merely because its screenshot has similar
padding or radius. For this program, parity means:

- **Mounted:** present in the current React product path.
- **Composed:** occupies the same understandable place in the workbench.
- **Complete:** default, hover, focus, active, selected, disabled, loading,
  empty, error, and success states exist where relevant.
- **Responsive:** works in wide, standard, and minimum supported windows.
- **Keyboard-complete:** every pointer action has an operable keyboard path.
- **Accessible:** labels, focus order, live state, contrast, and reduced motion
  are correct.
- **Performant:** long lists, large diffs, streaming rows, and many tabs stay
  within measured budgets.
- **Persistent:** selection, draft, layout, and unresolved attention survive
  the transitions users expect.
- **Authorized:** visible controls correspond to typed capabilities and honest
  refusals.
- **Branded:** OpenAgents tokens and product semantics remain recognizable.
- **Fixture-backed:** every meaningful visual state is in the component catalog
  and regression suite.

By that definition, the user-message bubble is near parity, the settings rail
is partial parity, and the sidebar is visual-direction parity only. The rest of
the program should use those commits as the method precedent while broadening
the denominator from isolated components to the entire coding workbench.

## Primary source map

### Prior OpenAgents analysis

- [T3 Code teardown](./2026-07-13-t3-code-teardown.md)
- [T3 Code/OpenAgents full gap analysis](./2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md)
- [T3 Code ACP implementation teardown](./2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md)
- [OpenAgents Desktop product architecture](../sol/2026-07-10-openagents-desktop-product-architecture.md)

### T3 Code UI

- `projects/repos/t3code/apps/web/src/components/AppSidebarLayout.tsx`
- `projects/repos/t3code/apps/web/src/components/Sidebar.tsx`
- `projects/repos/t3code/apps/web/src/components/ChatView.tsx`
- `projects/repos/t3code/apps/web/src/components/chat/ChatHeader.tsx`
- `projects/repos/t3code/apps/web/src/components/chat/MessagesTimeline.tsx`
- `projects/repos/t3code/apps/web/src/components/chat/ChatComposer.tsx`
- `projects/repos/t3code/apps/web/src/components/RightPanelTabs.tsx`
- `projects/repos/t3code/apps/web/src/components/CommandPalette.tsx`
- `projects/repos/t3code/apps/web/src/components/settings/`
- `projects/repos/t3code/apps/web/src/components/ui/`
- `projects/repos/t3code/apps/web/src/index.css`

### OpenAgents Desktop UI

- `apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx`
- `apps/openagents-desktop/src/renderer/react-timeline.tsx`
- `apps/openagents-desktop/src/renderer/react-composer.tsx`
- `apps/openagents-desktop/src/renderer/react-review.tsx`
- `apps/openagents-desktop/src/renderer/lexical-composer-editor.tsx`
- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/src/renderer/settings.ts`
- `packages/ui/src/workbench/`
- `packages/ui/src/desktop-workbench.css`
