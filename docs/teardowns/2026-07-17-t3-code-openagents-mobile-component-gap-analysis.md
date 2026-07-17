# T3 Code vs OpenAgents Mobile — Remaining Component Gap Analysis

**Date:** 2026-07-17  
**T3 Code baseline:** `8b5469863ae1dd696e696de30240ec3da607962d`  
**OpenAgents baseline:** `6a9599f986a0372bd5da95e652d0ce3a1e8d4368`  
**Scope:** implemented mobile UI first, functional depth second; designed-only
OpenAgents capabilities are called out separately from rendered components.

## Implementation update — compact T3-style header navigation

OpenAgents mobile now replaces the full-width pill toolbar and oversized
in-content thread heading with T3's compact navigation grammar: a standalone
glass thread-list control, centered bold thread identity, optional confirmed
repository/worktree subtitle, and a shared glass compose/settings action
cluster. Surface switching remains available in the drawer instead of turning
the thread title into a mode button. Truncated-history accounting appears only
when it is actionable rather than occupying every ordinary thread header.

## Executive conclusion

OpenAgents has closed the most visible first-order gap: its ordinary transcript
rows and collapsed/expanded input now use T3's message and composer grammar.
That makes the center of a selected chat recognizably competitive.

The app is not yet at component parity. T3's advantage is the connected system
around the chat: a dense project-aware sidebar, structured work-log rows,
approval and input cards, model and command controls in the composer, native
files/diff/review/Git/terminal surfaces, adaptive panes, and platform-specific
navigation chrome. OpenAgents already has stronger typed authority, exact refs,
portable-session controls, attention projections, accessibility contracts, and
receipt-oriented designs, but much of that capability is rendered as generic
buttons, captions, and vertically stacked panels. The remaining UI problem is
therefore one of component architecture and information hierarchy, not another
cosmetic pass over the chat bubbles.

The next parity epic should be **Thread Surface V2**: rich assistant content,
grouped runtime activity, compact approval/input cards, message actions, and an
attachment viewer. Composer intelligence should follow immediately, before the
larger Files/Changes/Terminal workbench surfaces.

## How to read this analysis

Status labels describe the shipped component, not an underlying schema or a
planned service:

- **Aligned** — the primary interaction and visual grammar are substantially
  present, even if polish remains.
- **Shallow** — a working component exists but exposes materially less state or
  fewer actions than T3.
- **Primitive** — the capability is rendered through generic primitives rather
  than a purpose-built mobile component.
- **Designed** — contracts or roadmap exist, but there is no equivalent mobile
  surface.
- **Missing** — no current mobile component was found.

Priorities are based on the mobile-controller journey: **P1** blocks a credible
daily workflow, **P2** materially reduces speed or legibility, and **P3** is
polish after the workflow is whole.

## Component gap matrix

### 1. App shell and navigation

| Component                 | T3 mobile                                                                                         | OpenAgents mobile now                                                                      | Remaining gap                                                                                                                                  | Status    | Priority |
| ------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------- | --------- | -------- |
| Top navigation            | Native header items, contextual thread actions, and direct Files/Git/Terminal entry points        | Compact T3-style glass thread-list control, centered title/context, and grouped compose/settings actions | Add Files/Git/Terminal items only as their authoritative routes land; promote the current in-flow header to native-stack chrome | Aligned/Shallow | P2       |
| Phone navigation          | Sidebar route with header actions, filters, project/thread rows, and platform-specific treatment  | Full-screen drawer built from generic buttons and captions                                 | Add a true navigation list with search/filter, section headers, status metadata, row menus, swipe actions, and stable back/close behavior      | Shallow   | P1       |
| Tablet layout             | `AdaptiveWorkspaceLayout` supports sidebar, detail, inspector panes, dividers, and pane animation | Same single-column tree at all widths                                                      | Add compact/regular breakpoints and persistent sidebar/detail/inspector composition                                                            | Missing   | P1       |
| Empty and loading routes  | Purpose-built home, empty detail, connection, placeholder, and retry states                       | Text-led empty, refresh, unavailable, and cache-withheld states                            | Preserve OpenAgents' honest authority copy but give each state a concise illustration/icon, primary action, skeleton, and retry placement      | Primitive | P2       |
| Hardware and system entry | Hardware keyboard commands, app shortcuts, share intake, notification navigation                  | Attention deep-link consumption and typed action dispatch; no comparable shortcut/share UI | Add keyboard shortcuts on tablets, app shortcuts, and share-to-new-task intake                                                                 | Shallow   | P2       |

### 2. Home, projects, and thread navigation

| Component            | T3 mobile                                                                                                  | OpenAgents mobile now                                                                                    | Remaining gap                                                                                                                                                       | Status           | Priority |
| -------------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------- | -------- |
| Thread list rows     | Project-aware rows with activity state, pending-task treatment, filtering, list options, and swipe actions | Chat title buttons; coding sessions are separate repository captions plus state-only buttons             | Create one row model that shows project, title, branch/worktree, recency, run state, attention, sync state, and selected state without exposing raw refs by default | Primitive        | P1       |
| Project hierarchy    | Add-project flows and project-scoped thread creation                                                       | Confirmed repository/session directory and explicit execution targets, but no project-management surface | Add repository onboarding, grouped/collapsible project sections, project actions, and destination selection                                                         | Designed/Shallow | P1       |
| Search and filters   | Home and sidebar filtering/list options                                                                    | No rendered thread search or filter                                                                      | Add local projection search, status/project filters, and predictable empty results                                                                                  | Missing          | P1       |
| Thread lifecycle     | Archive screen plus thread list actions and swipe affordances                                              | Rename, archive, restore, and delete are implemented inside the drawer                                   | Move lifecycle actions into row context/swipe menus and dedicated archived navigation; use sheets for rename/delete confirmation                                    | Shallow          | P2       |
| Attention            | Pending tasks and notification-aware navigation                                                            | Strong confirmed personal inbox plus exact pending approval/question selection                           | Merge attention into the same visual row system, with semantic badges and direct jump to the causal card                                                            | Shallow          | P1       |
| Controller directory | Environment/project/session navigation is integrated into the workbench                                    | Recent/Repositories/Attention segmented directory and portable-session details exist                     | Replace diagnostic-looking fact blocks with compact session cards and progressive disclosure; hide refs until inspection                                            | Primitive        | P2       |

### 3. Thread header and transcript

| Component          | T3 mobile                                                                                                 | OpenAgents mobile now                                                                                             | Remaining gap                                                                                                                                   | Status          | Priority |
| ------------------ | --------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | --------------- | -------- |
| Thread identity    | Contextual navigation title plus repository/Git/worktree actions                                          | Centered compact title with confirmed repository/worktree context; ordinary message/event counts removed from the feed | Add repository/Git/worktree actions and an authority info sheet as their routes land                                                            | Aligned/Shallow | P2       |
| User messages      | Blue rounded bubble, constrained width, timestamp, image attachments, and message affordances             | T3-like blue bubble, constrained width, and timestamp                                                             | Add long-press/copy actions, attachment viewer, delivery/queue state, and accessible action announcements                                       | Aligned/Shallow | P2       |
| Assistant messages | Full-width rich Markdown with links, highlighted code, review comments, attachments, and copy affordances | Full-width assistant row, but body content is primarily plain text lowered through the shared transcript renderer | Add native Markdown typography, selectable text, links, syntax-highlighted code blocks, copy buttons, file references, and review-comment cards | Shallow         | P1       |
| Runtime activity   | `ThreadWorkLog` groups and collapses tool activity separately from conversational prose                   | Runtime timeline events become system-like transcript entries or separate generic controls                        | Introduce typed work groups with running/success/failure states, elapsed time, summaries, expandable details, and causal agent identity         | Primitive       | P1       |
| Streaming          | Feed-specific placeholders and presentation state around active work                                      | A thinking row and confirmed updates; no rich token/tool transition treatment                                     | Add stable in-place streaming, active-work indicator, auto-scroll suspension/resume, and reduced-motion-safe transitions                        | Shallow         | P2       |
| Message actions    | Content-specific copy/open/review interactions                                                            | No consistent per-message action surface                                                                          | Add long-press menu and visible-on-selection actions: copy, open attachment/file, inspect refs, and retry where authoritative                   | Missing         | P2       |
| Attachments        | Inline image sizing, preview viewer, and content-aware presentation                                       | Confirmed images render inline; draft attachments are filename chips                                              | Add aspect-ratio-aware thumbnails, full-screen zoom, loading/error states, removal before send, and non-image file cards                        | Shallow         | P1       |
| Long histories     | Purpose-built feed presentation around thread state                                                       | FlatList-backed shared transcript and up to 500 retained messages                                                 | Add pagination/earlier-history affordance, scroll-to-bottom control, unread boundary, and deterministic anchor retention                        | Shallow         | P2       |

### 4. Runtime interaction cards and agent supervision

| Component          | T3 mobile                                                                 | OpenAgents mobile now                                                                                      | Remaining gap                                                                                                                                        | Status    | Priority |
| ------------------ | ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | --------- | -------- |
| Approval request   | Dedicated `PendingApprovalCard` with bounded choices and pending state    | Tool approvals render from typed interactions with authoritative submit handling                           | Create a compact card with operation summary, risk/context disclosure, approve/deny hierarchy, expiry/stale state, and post-decision receipt         | Primitive | P1       |
| User input request | Dedicated `PendingUserInputCard`                                          | Typed provider questions and selections are functional                                                     | Add question grouping, single/multi-select affordances, optional free text when allowed, validation, and a resolved summary                          | Primitive | P1       |
| Plan review        | T3 interaction flow is integrated with thread work                        | Accept/request-changes/replan authority exists                                                             | Give plans a readable document surface, sticky decision controls, and a resolved audit summary                                                       | Primitive | P1       |
| Run controls       | Thread-aware run controls and terminal/Git context                        | Retry/resume/cancel/close controls are functional generic buttons                                          | Consolidate into a status bar near the composer, with destructive confirmation and clear local/remote pending states                                 | Shallow   | P2       |
| Agent graph        | T3 emphasizes the thread work stream rather than a large diagnostic graph | OpenAgents projects a bounded live/historical hierarchy with exact provider/runtime/session/worktree facts | Retain the stronger model, but present it as compact avatars/rows and a dedicated inspector sheet instead of a bordered block above every transcript | Primitive | P2       |

### 5. Composer

| Component             | T3 mobile                                                                                 | OpenAgents mobile now                                                                                                     | Remaining gap                                                                                                                                      | Status    | Priority |
| --------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | --------- | -------- |
| Collapsed shell       | Pill-shaped glass input with circular send action                                         | Closely matched native/fallback shell and dimensions                                                                      | Tune keyboard animation, haptics, disabled/queued states, and Android material fallback on physical devices                                        | Aligned   | P3       |
| Expanded shell        | Focus morph reveals multiline editor, attachment action, model/options controls, and send | Focus expansion and attachment request are present; target facts/options sit as separate text and buttons above the shell | Move model, mode, target, and attachment controls into a compact composer toolbar; remove the diagnostic block from the primary flow               | Shallow   | P1       |
| Model/provider picker | Grouped model actions by provider and current selection                                   | Explicit execution targets can be selected; provider/model/account may display as raw labels                              | Add searchable grouped picker, readiness/error explanation, current selection label, and recent/default behavior while preserving target authority | Primitive | P1       |
| Commands              | Slash-command popover, built-ins, provider commands, and interaction-mode changes         | No rendered slash-command or command autocomplete                                                                         | Add typed slash-command results, keyboard navigation, empty/error states, and semantic command routing                                             | Missing   | P1       |
| File/path context     | Composer trigger system and rich attachments                                              | Typed attachment picker exists; no `@` path/file search                                                                   | Add repository-backed `@` file/path autocomplete after the coding target is selected                                                               | Missing   | P1       |
| Attachment editing    | Thumbnail strip, preview, removal, and image viewer                                       | Filename/mime/size chips and picker status                                                                                | Render thumbnails/file cards in the composer, allow remove/retry, and show upload/confirmation state per item                                      | Shallow   | P1       |
| Active-run behavior   | Composer and work state coordinate around the running thread                              | Draft remains available and separate runtime buttons can cancel/resume                                                    | Add queue/send-later policy, replace send with stop when appropriate, and make admission state explicit without discarding the draft               | Shallow   | P2       |

### 6. Coding workbench surfaces

| Component          | T3 mobile                                                                                            | OpenAgents mobile now                                                                    | Remaining gap                                                                                                                                     | Status   | Priority |
| ------------------ | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | -------- |
| Files              | File tree, source viewer, Markdown preview, image preview, and web preview                           | No mobile Files route                                                                    | Build a repository file navigator, source/Markdown/image renderers, path copy/open actions, and safe size/loading limits                          | Missing  | P1       |
| Changes and review | Native diff surface, syntax/word highlighting, file sections, selection, and review comment composer | Runtime/controller contracts exist, but no mobile diff/review component                  | Add changes overview, per-file diff, inline selection/comment draft, and authoritative writeback/receipt state                                    | Designed | P1       |
| Git                | Branch, commit, confirm, overview sheets and thread header quick actions                             | Repository/worktree identity is projected; no equivalent Git UI                          | Add status summary, branch picker, commit sheet, push/confirm flow, conflict/error states, and evidence links                                     | Missing  | P1       |
| Terminal           | Native terminal surface, session list, replay, preferences, and route bootstrap                      | Portable runtime control exists, but no PTY component                                    | Add terminal session picker and native terminal host with reconnect/replay, keyboard accessory, size negotiation, and bounded transcript behavior | Designed | P1       |
| Preview/artifacts  | File-backed Markdown/image/web previews and inspector integration                                    | Preview, artifacts, and receipts are planned but not rendered as a coherent mobile route | Add an inspector destination for preview, artifacts, verification, and receipts; keep active content bound to exact session refs                  | Designed | P2       |

### 7. Settings, connections, notifications, and platform finish

| Component                | T3 mobile                                                                                       | OpenAgents mobile now                                                                                              | Remaining gap                                                                                                                                                                 | Status           | Priority |
| ------------------------ | ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------- | -------- |
| Settings                 | Dedicated appearance, auth, storage, environment, legal, and cloud routes                       | Settings destination exists, but the visible shell is primarily sync/account/controller content                    | Create a native settings hierarchy with account, appearance, accessibility, notifications, storage/cache, diagnostics, and legal sections                                     | Primitive        | P2       |
| Connections/environments | Pairing, cloud/local environment rows, status dots, connection notices, and add flows           | Sync phases and execution-target readiness are typed; no comparable environment-management component               | Add environment list/detail, pairing flow, health/status affordances, reconnect, and scoped access explanation                                                                | Designed/Shallow | P1       |
| Notifications            | Permission, registration, response consumption, live-activity preferences, and navigation       | Attention target projection and navigation exist; end-to-end physical push registration/delivery is not yet proven | Add permission education, notification preferences, registration health, and signed physical-device proof                                                                     | Designed/Shallow | P1       |
| Motion and feedback      | Native pane transitions, platform header treatment, swipe actions, and focused component motion | Reduced-motion is respected; most UI changes are immediate generic-tree replacements                               | Add restrained route, row insertion, composer morph, sheet, and success feedback with reduced-motion alternatives                                                             | Shallow          | P2       |
| Accessibility            | Native controls benefit from platform behavior                                                  | Explicit minimum targets, text scaling, labels, reduced-motion profile, and bounded hierarchies                    | Preserve this OpenAgents strength; add rotor/heading semantics, focus restoration after sheets/actions, code/terminal accessibility, and physical VoiceOver/TalkBack evidence | Aligned/Shallow  | P2       |
| Visual system            | Dense native workbench with intentional component-specific hierarchy                            | Strong shared tokens, but generic Button/Text/Stack patterns remain visually repetitive                            | Add domain components and semantic density tiers; do not solve every state with another bordered card or full-width button                                                    | Primitive        | P2       |

## The five largest gaps

### 1. Work is not yet a first-class visual object

OpenAgents has richer runtime and agent data than the screen communicates. Tool
activity, approvals, questions, plans, controls, and agent facts compete with
conversation content as generic rows or panels. T3's work log and dedicated
cards make the agent's activity scannable without turning the transcript into a
diagnostic console.

The parity move is a typed `WorkGroup`/`WorkItem` presentation family with
collapsed summaries, stable status icons, progress and elapsed time, causal
agent identity, and expandable details. Raw refs belong in an inspector, not in
the default reading path.

### 2. The composer looks close but does less

The shipped shell now matches T3's geometry and focus behavior, but the controls
around it remain fragmented. Target, provider, model, account, readiness, and
attachments are shown above the composer instead of forming one coherent input
instrument. Commands and file mentions are absent.

Parity requires a composer toolbar and popover system, not more labels. It must
remain driven by the typed execution-target catalog so visual similarity never
creates a second source of authority.

### 3. Navigation does not yet describe a coding workspace

The drawer can select chats and sessions, and its lifecycle operations work,
but it reads like an administrative menu. It lacks the row density, grouping,
search, status, gestures, and contextual actions that let a user supervise many
projects quickly.

The next navigation component should unify conversations, coding sessions, and
attention under a project-aware row grammar while keeping confirmed authority
and cached-withheld accounting intact.

### 4. The workbench is mostly a roadmap, not a set of routes

Files, changes, review, Git, and terminal are where T3 becomes a mobile coding
controller rather than a chat client. OpenAgents has sound target and portable
session foundations, but those foundations cannot yet be used to inspect a
file, review a diff, commit a change, or operate a PTY from mobile.

These should be independent inspector destinations sharing exact session,
repository, and worktree identity—not miscellaneous cards added to the chat.

### 5. Adaptive and native presentation trails the data model

OpenAgents' Effect Native boundary is strategically stronger than duplicating
application state inside React Native. The current renderer, however, still
produces one phone-shaped generic tree. T3 proves that the same controller can
feel native on phone and tablet through platform headers, sheets, panes,
dividers, swipe actions, keyboard commands, and inspector placement.

Effect Native needs those presentation primitives so application features can
stay typed without accepting lowest-common-denominator UI.

## Recommended parity sequence

### Epic A — Thread Surface V2

Ship rich Markdown, selectable/copyable content, code blocks, attachment cards
and viewer, grouped runtime work, compact approval/input/plan cards, message
actions, and stable streaming/scroll behavior.

**Parity proof:** a long real coding thread remains readable while tools run;
the user can inspect and act on pending work without leaving the causal point in
the transcript; VoiceOver and TalkBack can traverse messages and work groups in
order.

### Epic B — Composer Intelligence

Move target/model/mode controls into the expanded composer, add grouped picker
sheets, slash commands, `@` file context, attachment preview/removal, and
active-run send/stop/queue behavior.

**Parity proof:** a user can choose a ready target and model, attach or mention
a file, issue a command, and submit or stop a turn without touching diagnostic
UI outside the composer.

### Epic C — Workspace Navigation

Build project-aware thread/session rows, search and filters, swipe/context
actions, attention badges, archived navigation, route-aware headers, and
compact/regular adaptive shell composition.

**Parity proof:** a user with at least five repositories and fifty threads can
locate an active or attention-requiring session in a few seconds on phone and
keep the sidebar visible on tablet.

### Epic D — Files and Changes

Add file tree, source/Markdown/image views, changed-files summary, native diff,
review selection/comments, and safe authoritative writeback with receipts.

**Parity proof:** from a thread, the user can open a referenced file, inspect
the relevant diff, leave a review instruction, and return to the same transcript
anchor.

### Epic E — Git and Terminal

Add Git status/branch/commit/push sheets and the native PTY host with session
selection, replay, reconnect, keyboard tooling, and failure recovery.

**Parity proof:** branch and commit state remain tied to the exact worktree, and
a terminal survives background/reconnect without inventing output or command
success.

### Epic F — Connections, notifications, and finish

Add environment management, pairing/health, physical push registration and
preferences, settings hierarchy, adaptive inspector, shortcuts/share intake,
motion/haptics, and a signed physical-device accessibility/dogfood matrix.

**Parity proof:** local and hosted targets can be paired and diagnosed from the
app; attention arrives and deep-links correctly on physical iOS and Android;
all primary routes pass compact/regular layout and accessibility checks.

## What to copy and what to keep

Copy T3's interaction grammar: compact native navigation, information-dense
rows, progressive disclosure, work-log grouping, dedicated decision cards,
composer-local controls, inspectors, sheets, and adaptive panes.

Keep OpenAgents' stronger foundations:

- one typed Effect Native application tree instead of parallel React Native
  authority;
- confirmed exact refs for session, thread, repository, worktree, run, and
  interaction identity;
- fail-closed target readiness and cached-but-withheld accounting;
- portable-session movement and evidence/receipt semantics;
- centralized tokens, minimum target sizing, text scaling, reduced motion, and
  explicit accessibility labels.

Component parity is reached only when the visual component covers its complete
state machine: loading, empty, live, pending, offline, stale, failed, resolved,
disabled, destructive confirmation, keyboard/focus behavior, accessibility,
and physical-device evidence. A screenshot match of the happy state is useful,
but it is not parity.

## Final recommendation

Start the next implementation epic with **Thread Surface V2**, not Files and
not another shell restyle. It improves every existing conversation immediately,
gives the already-implemented approvals, questions, plans, runtime events, and
agent graph an appropriate visual home, and establishes the component grammar
that Files, Changes, Git, Terminal, and receipts can later reuse. Then complete
Composer Intelligence before widening the workbench.
