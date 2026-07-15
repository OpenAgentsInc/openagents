---
spec_format_version: "0.1"
title: "OpenAgents Desktop MVP Phase 2 — React-backed Codex Workbench"
artifact_type: "prd"
spec_revision: 3
author: "OpenAgents"
created_at: "2026-07-14T00:00:00Z"
updated_at: "2026-07-14T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "apps/openagents-desktop/"
  - path: "apps/openagents.com/packages/effect-native-render-dom/"
  - component: "desktop-codex-workbench"
custom_sections:
  - id: "custom-design-direction"
    label: "Design Direction"
    after: "solution"
  - id: "custom-architecture-boundary"
    label: "Architecture Boundary"
    after: "custom-design-direction"
  - id: "custom-reference-code"
    label: "Reference Code"
    after: "custom-architecture-boundary"
  - id: "custom-success-metric-context"
    label: "Success Metric Context"
    after: "success_metrics"
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "rollout"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
  - id: "custom-decision-trace"
    label: "Decision Trace"
    after: "custom-promise-links"
tool_metadata:
  openagents_phase: "desktop-mvp-phase-2"
  openagents_phase_status: "proposed; no dispatch, release, or public-claim authority"
  openagents_parent_spec: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md@6"
  openagents_architecture_audit: "docs/effect-native/2026-07-14-react-web-renderer-harmonization-gap-analysis.md"
  openagents_reference_code: "projects/repos/t3code@c1ec1915fc16f3dc1ec5d47d9a97f6210a574526"
  openagents_supersedes: "docs/mvp/openagents-desktop-mvp-phase-2-react-codex-workbench.product-spec.md@de1180b2da937922c2a8724915cf761f8fb78617"
---

## Problem

The first OpenAgents Desktop MVP proved a signed, local-first Codex workroom
with durable identity, typed timelines, controlled intents, restart recovery,
and strict renderer authority. Its visible implementation is still expensive
to evolve as a polished desktop product. The current React root hosts the
existing direct-DOM Effect Native renderer, so React owns lifecycle but does
not yet render the workbench's components, overlays, focus behavior, or
incremental updates.

That gap is most noticeable in the ordinary Codex loop. Starting a chat,
finding or resuming a session, reading a live turn, sending or steering a
prompt, resolving a blocker, and reviewing a change should feel immediate and
familiar. ProductSpec, child topology, Fleet, terminal, editor, and broader
platform machinery must not crowd that primary path or become prerequisites
for a useful session.

Replacing the entire application with an independent React app would gain UI
velocity by creating a second component, state, command, and token system.
Finishing every generic Effect Native React-DOM lowering before improving the
workbench would delay the user-visible result. Phase 2 needs a bounded middle
path: React components for the core desktop experience, with Effect Native and
Effect continuing to own the portable contract and application truth.

## Hypothesis

If OpenAgents renders the basic Codex-management workbench with a focused set
of React 19 component lowerings—using Tailwind for renderer-private styling
and reviewed accessible primitives for overlays—while preserving the existing
Effect services, Effect Native view contract, typed projections, intent
registry, tokens, and host security boundary, then developers will be able to
start, manage, and resume Codex work smoothly inside OpenAgents and the team
will be able to iterate on that experience without creating a second app
architecture.

## Scope

```productspec-scope
in:
  - one React 19 root for the OpenAgents Desktop renderer
  - renderer-private React component lowerings for the exact Effect Native nodes used by the core Codex workbench
  - Tailwind CSS 4 utilities compiled from canonical Effect Native token values and semantic state variables
  - owner-selected shadcn preset `b3Zg9L0M8A` (`base-vega`, zinc/blue, Oxanium body, Geist headings, small radius, Lucide) as the preferred renderer-private React source-component layer, lifted into the Khala theme
  - reviewed Base UI primitives supplied by that preset, or an equivalently accessible React primitive, behind typed renderer-owned menu, dialog, popover, tooltip, and command-palette lowerings
  - a familiar desktop shell with session rail, conversation header, typed timeline, composer, command palette, blocker and approval surfaces, read-only review drawer, status, and settings entry
  - local-first new chat, session search, session switch, resume, archive, and delete flows already authorized by the Phase 1 contracts
  - metadata-first session rendering before transcript hydration, stable selection, and restart restoration
  - typed text, reasoning summary, plan, tool, patch or file-change, usage, blocker, error, interruption, gap, and terminal timeline items
  - send, stop, steer-current-turn, queue-next-turn, question, approval, and plan-review actions through the existing registered command identities
  - ordinary Codex use without requiring ProductSpec authoring, a ProductSpec plan, an OpenAgents account, Fleet, or another product surface
  - repository context summary plus bounded read-only status and exact diff review reachable without turning the workbench into an editor
  - explicit empty, loading, hydrating, streaming, blocked, offline, incompatible, signed-out, quota, rate-limit, policy-denied, revoked, interrupted, and failed states
  - keyboard, pointer, screen-reader, reduced-motion, narrow-window, renderer-reload, and app-restart behavior for the core workbench
  - compatibility mounting for non-converted Effect Native surfaces while the focused React lowering set is completed and proven
out:
  - a full editor, interactive terminal, preview server, browser, voice, or computer-use surface
  - Git discard, commit, branch, push, pull request, merge, or arbitrary command execution
  - ProductSpec authoring, criterion boards, AssuranceSpec inspection, child-agent topology, Fleet, multi-account controls, or remote workrooms as Phase 2 completion requirements
  - mobile, web, React Native, server rendering, or hydration completion as a condition for this Desktop phase
  - completion of every Effect Native DOM catalog lowering or removal of the direct-DOM compatibility backend
  - adoption of TanStack Router, Zustand, Effect Atom React, Lexical, xterm, a diff editor, or a virtualizer without a measured need in the scoped journey
  - adoption of the Vercel AI SDK (`ai` or `@ai-sdk/*`) for the Desktop MVP; model streaming, tools, session continuation, and runtime events remain owned by the existing Codex Runtime Gateway and compatible app-server path
  - a new visual brand, token system, icon system, or component library that competes with the OpenAgents and Effect Native design contracts; the selected shadcn source components are permitted only as a Khala-themed renderer extension
cut:
  - a second durable thread, turn, command, approval, repository, or session store owned by React
  - React Query, Zustand, Context, hooks, URL state, or component-local state becoming domain or persistence authority
  - arbitrary ReactNode, JSX component, callback, className, credential, provider payload, absolute path, Node handle, generic IPC, process handle, or filesystem handle entering the portable Effect Native view contract
  - direct renderer parsing of raw Codex app-server events, rollout files, terminal text, credentials, or provider-specific payloads
  - duplicate subscriptions, duplicate command dispatch, optimistic terminal success, silent retry, silent session substitution, or state restoration that reruns work
  - a required ProductSpec ceremony before a user can start an ordinary Codex conversation
  - copying external reference source, importing it as a runtime dependency, or adopting its state model as OpenAgents authority
  - release, publication, or public product claims inferred from this intent document or component-level tests
```

## User Experience

A developer opens OpenAgents Desktop and sees a quiet, recognizable workbench.
The session rail appears immediately from bounded metadata. New Chat is
prominent; search and recent sessions are close at hand. Selecting a session
keeps the selection stable while its transcript hydrates. Empty and loading
states explain what is happening without covering the workspace in cards or
spinners.

The center column is the conversation. User and assistant content reads as a
continuous causal timeline, while plans, tools, changes, blockers, usage, and
terminal outcomes use compact, distinct components. Live content updates in
place without jumping the reader. Older content can load above the viewport
without moving the first visible item. A gap or interrupted stream is shown as
a gap or interruption, never smoothed over as complete.

The composer is always easy to reach. It captures the first keystroke, handles
IME composition, grows within a bounded height, and makes the current action
unambiguous: send while idle, stop while the agent is running, steer the
current turn where supported, or queue the next instruction. Keyboard and
pointer entry points dispatch the same registered intent. A question,
approval, or plan review appears as an accessible focused surface with clear
choices, escape behavior, and focus restoration.

Repository context remains adjacent and read-only. A compact change summary
can open an exact diff drawer correlated to the timeline. Settings and
diagnostics remain available but do not compete with the conversation. The
primary journey never asks the user to understand ProductSpec, Fleet, account
rotation, or internal renderer architecture.

On a narrow desktop window, the session rail becomes a dismissible overlay and
the review drawer becomes a full-height sheet; the conversation and composer
remain usable. Status changes are announced without stealing focus. Motion is
short and functional, and reduced-motion preference removes nonessential
transitions.

## Solution

Phase 2 converts the core Desktop workbench from a React-hosted direct-DOM
subtree to ordinary React elements produced by the shared Effect Native DOM
renderer. The portable screen program continues to emit the same schema-backed
view nodes and registered intent keys. A renderer adapter exposes the resolved
view snapshot through a synchronous `getSnapshot`/`subscribe` contract and the
React surface consumes it with `useSyncExternalStore` or an equivalent React
19-safe mechanism.

The first React lowering set covers only the nodes and behaviors needed by the
scoped workbench. Shared typed lowerings own semantic element selection,
stable keys, accessibility props, sanitized content, intent lookup, and token
resolution. Renderer-private React components own browser mechanics such as
focus, IME state, measured geometry, portal placement, pointer capture,
virtual window state if later justified, and short transition phase. Those
mechanics are memory-only and reset with the keyed renderer lifecycle.

Tailwind is a build-time styling tool, not the design source of truth. Its
theme maps to canonical `--en-*` variables and OpenAgents semantic state
tokens. The owner-selected shadcn preset supplies source-owned React component
recipes and is preferred whenever an installed component meets the need. Its
zinc/blue semantics are lifted into the one dark Khala theme: Oxanium, Geist,
small-radius geometry, Lucide, subtle menu behavior, and component composition
remain, while color roles alias Effect Native rather than forming another
palette. Base UI may implement difficult browser primitives behind those
renderer-private components; neither layer enters application state or portable
modules. Non-converted surfaces continue through the existing compatibility
backend until separately migrated.

The Phase 1 Runtime Gateway, app-server custody, persistence, WorkContext,
command registry, restart reconciliation, renderer boundary, and public-safety
rules remain unchanged. This ProductSpec narrows presentation and interaction
work; it does not reopen Phase 1 acceptance or relabel its evidence.

The Vercel AI SDK is deliberately not part of this MVP architecture. React is
the presentation host, not a new model/runtime client. Introducing an AI SDK
here would create a second streaming, tool-call, and session abstraction beside
the already accepted Codex Runtime Gateway. Any later AI SDK evaluation must be
a separate provider-neutral capability decision with its own authority and
migration analysis; it is not implied by adopting React.

## Design Direction

### Product character

- Familiar desktop controls, system typography, restrained color, and dense
  but breathable information hierarchy.
- Conversation first. Navigation and review support the active turn instead
  of presenting every platform capability at once.
- One strong primary action per state. Secondary actions use menus, quiet
  buttons, or the command palette rather than competing call-to-action styles.
- Semantic state is communicated by copy, icon, and structure as well as
  color. Decorative gradients, excessive cards, and ornamental motion are not
  part of the workbench.

### Core component set

| Component | Responsibility | Required states |
| --- | --- | --- |
| `WorkbenchShell` | desktop layout, pane ownership, narrow-window behavior | ready, narrow, offline, fatal |
| `SessionRail` | new, search, select, resume, archive, delete | empty, metadata-loading, ready, paging, error |
| `ConversationHeader` | session title, repository context, status, compact actions | idle, running, blocked, interrupted |
| `Timeline` | stable keyed causal items and older-page prepend | empty, hydrating, streaming, gap, terminal, error |
| `TimelineItem` family | text, plan, tool, change, usage, blocker, lifecycle | pending, active, succeeded, failed, interrupted |
| `Composer` | prompt entry and current command affordance | idle, composing, submitting, running, queued, disabled |
| `CommandPalette` | searchable registered commands | closed, searching, no-results, selection |
| `DecisionSurface` | question, approval, and plan review | open, submitting, stale, resolved, failed |
| `ReviewDrawer` | bounded status and exact correlated diff | closed, loading, ready, stale, unavailable |
| `StatusNotice` | signed-out, incompatible, quota, rate, policy, revocation | informative, blocking, recoverable, terminal |

### Interaction and accessibility

- Every action has a visible focus style and a stable accessible name.
- Menus, dialogs, popovers, tooltips, and the command palette follow the
  expected keyboard model, trap focus only when modal, restore focus on close,
  and close predictably on Escape.
- Timeline streaming uses bounded live-region announcements; it does not read
  every token or move focus.
- Pointer targets are at least 24 by 24 CSS pixels, with 44 by 44 preferred for
  isolated primary controls where density permits.
- Text and interactive states meet WCAG 2.2 AA contrast. Error, warning,
  running, and success states do not rely on color alone.
- State transitions normally complete in 150–250 ms. Reduced motion removes
  transforms and nonessential transitions while preserving state change.
- The supported minimum window is 760 by 520 CSS pixels. Below 980 pixels the
  session rail overlays; below 1120 pixels the review drawer overlays instead
  of compressing the conversation.

## Architecture Boundary

### Ownership ledger

| Concern | Authority | React responsibility |
| --- | --- | --- |
| Codex process and protocol | host-owned Runtime Gateway and compatible app-server | none |
| model streaming and tool execution | host-owned Runtime Gateway and compatible app-server; no Vercel AI SDK in this MVP | render the typed timeline and controls only |
| threads, turns, approvals, commands, WorkContext, recovery | existing Effect services and durable stores | render typed projections only |
| view structure and portable intent keys | Effect Native schema-backed view program | lower nodes to ordinary React elements |
| subscription snapshot | Effect-owned projection adapter | consume with one React-safe subscription |
| command execution | registered typed intent dispatcher | dispatch the exact resolved key once |
| theme and semantics | Effect Native/OpenAgents tokens | consume variables through Tailwind utilities |
| focus, IME, geometry, portals, transient animation | renderer lifecycle | memory-only local mechanics |
| persistence | existing host and Effect stores | none |

### Implementation rules

1. Keep one React root and one authoritative projection subscription per
   mounted workbench. Strict Mode, reload, remount, and error recovery must not
   duplicate host subscriptions or intent effects.
2. Lower portable view nodes to semantic React elements. Do not pass arbitrary
   components or callbacks through the schema to make a lowering convenient.
3. Resolve every event through the existing typed interaction registry. A
   React callback is an implementation detail created after resolution, never
   a new command identity.
4. React state may hold only ephemeral renderer mechanics. If state must
   survive remount, coordinate a command, affect another surface, or be
   reconciled after restart, it belongs in the existing Effect model.
5. Keep React, Base UI, Tailwind, DOM APIs, and browser-only libraries inside
   declared renderer or Desktop-host modules. Portable Effect Native modules
   remain React-free `.ts` and do not gain `ReactNode` or `className` fields.
   Generated shadcn source stays in the declared Desktop component directory;
   prefer it over bespoke DOM when it already provides the required control.
6. Sanitize Markdown, links, and any rendered code through the existing
   bounded content policy. React does not receive credentials, raw provider
   payloads, absolute roots, or general host authority.
7. Preserve the direct-DOM renderer only as a measured compatibility backend.
   A surface uses one backend at a time; nested roots may not render the same
   authoritative workbench state twice.
8. Do not block this product phase on the complete generic React-DOM renderer
   program. Catalog-wide coverage, web hydration, and final compatibility
   backend removal retain their own design and evidence gates.

The current invariant allows React to own renderer lifecycle while the Effect
Native application remains authoritative. If implementation needs a broader
renderer-private React component allowance than that invariant currently
states, the implementation change must update `INVARIANTS.md`, name the exact
exception, and add scanner or contract coverage in the same commit. This
proposed ProductSpec does not silently relax the live invariant.

## Reference Code

The read-only implementation reference is T3 Code at workspace path
`projects/repos/t3code`, pinned for this specification to commit
`c1ec1915fc16f3dc1ec5d47d9a97f6210a574526` from
`https://github.com/pingdotgg/t3code`. It demonstrates a current React 19,
Tailwind CSS 4, and Base UI desktop workbench. OpenAgents studies its component
composition and browser interaction mechanics; the reference is not a runtime
dependency, design authority, state authority, or source-copy license for this
phase. The pinned reference declares neither `ai` nor `@ai-sdk/*`; its Codex
path is built around Effect client-runtime/contracts and a generated
`effect-codex-app-server` package. That is evidence that the React presentation
choice is independent of a Vercel AI SDK choice, not a mandate to copy T3's
runtime architecture.

The most relevant files are:

- `apps/web/src/main.tsx` and `apps/web/src/AppRoot.tsx` for root composition,
  provider placement, and top-level error handling.
- `apps/web/src/components/AppSidebarLayout.tsx` and
  `apps/web/src/components/Sidebar.tsx` for session-navigation layout and
  collapsed/narrow behavior.
- `apps/web/src/components/ComposerPromptEditor.tsx` for prompt focus,
  composition, submit behavior, and dynamic editor sizing.
- `apps/web/src/components/CommandPalette.tsx` for searchable command overlay,
  keyboard navigation, and focus restoration.
- `apps/web/src/components/ChatMarkdown.tsx` for memoized rich-text rendering
  and bounded code presentation.
- `apps/web/src/components/DiffPanel.tsx` for read-only change-review layout.
- `apps/web/src/rpc/atomRegistry.ts` and `packages/client-runtime/src/state/`
  as comparison material for subscription granularity only; OpenAgents does
  not adopt those stores as application authority.

Terminal, editor, routing, and specialized state libraries present in the
reference remain outside this phase unless a scoped acceptance failure proves
the need and a separate dependency decision records the cost and boundary.

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: The shipped Desktop workbench mounts through one React 19 root and renders the scoped shell, session rail, conversation header, timeline, composer, command palette, decision surface, review drawer, and status notices as ordinary React elements produced by declared renderer-private Effect Native lowerings.
- id: AC-2
  criterion: The React surface consumes one synchronous Effect-owned snapshot subscription and dispatches only existing registered typed intent keys; Strict Mode, rerender, reload, remount, and recovery tests observe no duplicate host subscription, command, or terminal outcome.
- id: AC-3
  criterion: A supported user can launch locally, use the ordinary logged-in Codex session, grant one repository, and start an ordinary new chat without an OpenAgents account, ProductSpec artifact, accepted plan, Fleet setup, account selector, or provider selector.
- id: AC-4
  criterion: The React session rail paints bounded top-level metadata before transcript hydration and supports new, search, select, resume, archive, delete, and paging while preserving stable ordering, titles, status, attention, selection, and restart restoration.
- id: AC-5
  criterion: The React timeline renders stable keyed authored text plus compact plan, tool, change, blocker, error, interruption, gap, and terminal treatments in causal order; transport, metadata, context, and token-accounting events do not appear as conversation messages, settled consecutive work folds behind a bounded disclosure, active work and streaming state remain visible without duplicating or reordering items, and prepending older variable-height content preserves the first visible item without a stale intermediate frame.
- id: AC-6
  criterion: The React composer captures the first keystroke, preserves IME composition, grows within its bound, and exposes the correct send, stop, steer, or queue action for the current state; pointer, keyboard, command-palette, and native-menu entry points resolve to the same registered command identity.
- id: AC-7
  criterion: Question, approval, and plan-review requests open an accessible focused decision surface with complete choices, explicit pending, stale, failed, and resolved states, predictable Escape behavior, and focus restoration; prose or optimistic UI never records the decision.
- id: AC-8
  criterion: Repository context and read-only status or exact diff review remain bounded to the current WorkContext and correlated timeline refs; stale, revoked, secret-shaped, binary, oversized, or unavailable output fails visibly and no Git mutation or absolute path enters React props or state.
- id: AC-9
  criterion: The scoped workbench is fully operable by keyboard at 760 by 520 CSS pixels, restores focus across overlays, provides bounded screen-reader announcements, meets WCAG 2.2 AA contrast for text and controls, and honors reduced-motion preference.
- id: AC-10
  criterion: Empty, loading, hydrating, streaming, blocked, offline, incompatible, signed-out, quota, rate-limit, policy-denied, revoked, interrupted, and failed states are visually distinct, have actionable copy where recovery exists, and never masquerade as success or completion.
- id: AC-11
  criterion: React, Tailwind, Base UI, DOM APIs, hooks, and JSX remain in declared renderer or Desktop-host modules; portable Effect Native modules remain React-free and schema fields contain no arbitrary component, callback, ReactNode, className, credential, raw provider payload, absolute path, Node handle, generic IPC, process handle, or filesystem handle.
- id: AC-12
  criterion: One installed-app journey creates a chat, submits a real Codex turn, streams typed content, resolves one blocker or review request, opens one exact read-only change, stops or steers a running turn, switches away and back, reloads the renderer, restarts the app, and resumes the same durable session without duplicate execution or silent rerun.
- id: AC-13
  criterion: Non-converted surfaces use the existing compatibility backend without rendering the same workbench state through two backends, and the Phase 2 release does not require catalog-wide React-DOM coverage, web hydration, mobile changes, or compatibility-backend removal.
- id: AC-14
  criterion: The React workbench meets the existing signed artifact and security gates, has a median warm launch-to-interactive time no greater than 1500 ms and p95 no greater than 2500 ms on the admitted macOS ARM64 profile, and switches a metadata-ready session to its first stable transcript paint within 400 ms at p95 for the acceptance corpus.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: supported_first_launches_reaching_an_editable_codex_composer
  target: ">= 90% within 60 seconds"
  window: first 30 days of invited Phase 2 dogfood
- id: SM-2
  metric: qualifying_codex_management_journeys_completed_without_opening_another_codex_interface
  target: ">= 80%"
  window: first 30 days of invited Phase 2 dogfood
- id: SM-3
  metric: metadata_ready_session_switches_reaching_first_stable_transcript_paint
  target: ">= 95% within 400 ms"
  window: release acceptance corpus and first 30 days of opted-in dogfood
- id: SM-4
  metric: observed_duplicate_intent_or_duplicate_terminal_outcome_incidents_caused_by_react_lifecycle
  target: "0"
  window: implementation dogfood through release acceptance
- id: SM-5
  metric: required_keyboard_focus_and_screen_reader_workbench_checks_passing
  target: "100%"
  window: every Phase 2 release candidate
- id: SM-6
  metric: supported_warm_launches_reaching_interactive_workbench
  target: "median <= 1500 ms and p95 <= 2500 ms"
  window: every Phase 2 release candidate on the admitted macOS ARM64 profile
```

## Success Metric Context

- **SM-1** segment: supported macOS hosts with compatible ordinary Codex
  sessions; source: consented public-safe activation timing receipt.
- **SM-2** segment: opted-in sessions that start with compatible runtime and a
  valid repository grant; source: public-safe local journey counters with no
  prompt, transcript, path, account, or machine identifier.
- **SM-3** segment: acceptance corpus plus opted-in metadata-ready session
  switches; source: bounded renderer performance receipt.
- **SM-4** segment: all Phase 2 test and dogfood commands; source: private exact
  intent and terminal-ref reconciliation projected only as aggregate count.
- **SM-5** segment: the scoped component and installed-journey matrix; source:
  keyboard, accessibility-tree, focus, and native interaction receipts.
- **SM-6** segment: admitted macOS ARM64 release profile; source: existing
  startup benchmark harness with exact artifact binding.

Metrics remain absent when consent is off. They collect no prompt, transcript,
repository content, absolute path, credential, stable account, or stable
machine identity.

## Risks

- A quick React surface can become a second application architecture. The
  ownership ledger, import scanner, snapshot contract, and duplicate-intent
  tests are release gates, not cleanup work.
- A compatibility island can survive indefinitely and leave two browser
  lifecycles. Each converted surface needs an explicit backend disposition,
  and the same workbench may never mount through both.
- Component-local state can drift from Effect truth during streaming or
  restart. Durable or cross-surface state is forbidden in React.
- Tailwind utilities can silently fork tokens or accessibility states. Every
  color, spacing, type, radius, and state value must resolve through the
  canonical theme mapping or an explicitly reviewed renderer mechanic.
- A third-party overlay primitive can alter focus, portal, or dismissal
  behavior across versions. Pin the version, wrap it behind typed nodes, and
  keep interaction fixtures at the wrapper boundary.
- A polished transcript can conceal missing or reordered events. Stable key,
  gap, prepend, restart, and terminal-disposition oracles remain mandatory.
- Narrowing the primary journey can accidentally delete Phase 1 specialist
  capabilities. Phase 2 removes them from its completion path, not from stored
  state or historical proof; any removal requires separate intent and gates.
- The reference application may encourage uncritical dependency adoption.
  Every dependency remains excluded until a scoped failure and boundary review
  justify it.
- Performance work can trade away correctness by coalescing distinct typed
  events. Causal item identity and exact terminal state are never optimization
  variables.

## Open Questions

- Which existing Effect Native catalog nodes can be lowered directly for the
  vertical slice, and which need a renderer-private typed host driver?
- Does the current timeline corpus require a virtualizer for the first release,
  or do paging and stable keyed React updates meet the 400 ms target without
  one?
- Is Base UI the selected pinned overlay primitive, or does the existing
  renderer behavior plus a smaller focus/positioning dependency produce a
  safer first component set?
- Which Phase 1 specialist surfaces remain reachable from Settings or command
  palette during conversion, and which stay on the compatibility backend?
- What exact macOS hardware and corpus size define the final Phase 2
  performance profile beyond the existing ARM64 acceptance machine?
- Does read-only diff review belong in the first installed journey, or may the
  change summary satisfy the initial journey while the drawer completes in the
  next release candidate?

## Rollout

### Delivery order

1. **Boundary and adapter:** freeze the ownership ledger; add the synchronous
   snapshot adapter, one-subscription lifecycle tests, import scanner, error
   boundary, token-to-Tailwind mapping, and compatibility-backend selector.
2. **Shell and sessions:** lower the shell, session rail, header, narrow-window
   behavior, metadata-first loading, selection, search, and New Chat.
3. **Timeline:** lower typed timeline items, stable keys, live replacement,
   gap states, page prepend, Markdown, and terminal outcome.
4. **Composer and decisions:** lower the composer, command palette, send/stop/
   steer/queue controls, questions, approvals, plan review, overlay focus, and
   reduced-motion behavior.
5. **Review and recovery:** add bounded read-only change review, explicit
   failure notices, reload/restart restoration, and compatibility transitions.
6. **Installed acceptance:** run the exact real-Codex journey, accessibility
   matrix, boundary scans, startup/session-switch benchmarks, signed artifact
   gate, and independent review.

### Release boundary

Phase 2 is complete when the basic Codex-management journey is coherent in
React, the architecture boundary is enforced, and the exact installed receipt
passes. It is not held for a full editor, terminal, ProductSpec workbench
redesign, child/fleet UI, mobile work, server rendering, or the complete
catalog migration. Those capabilities keep their own intent and acceptance.

The ProductSpec is intent, not a work plan. Actual packets, claims, sequence,
and release-candidate admission remain in the owning roadmap and workroom
ledgers. A green component test cannot publish the app or revise the Phase 1
assurance record.

## Owner Gates

- Admit this exact Phase 2 ProductSpec identity before implementation packets
  are treated as accepted work.
- Approve any required change to the React/Effect Native invariant before a
  renderer-private component crosses the currently enforced boundary.
- Accept the exact installed basic Codex-management journey, including the
  decision to keep ProductSpec and broader platform capabilities outside the
  primary flow.
- Approve the pinned overlay dependency and its wrapper boundary if Base UI or
  another third-party primitive enters the release artifact.
- Approve consent copy before any dogfood metric collection.
- Authorize distribution and public language separately; this spec and its
  implementation receipts authorize neither.

## Receipts

- ProductSpec validation under both the OpenAgents and upstream profiles,
  exact document digest, revision, and admission record.
- React ownership receipt proving one root, one projection subscription,
  exact-once intent dispatch, Strict Mode safety, error recovery, teardown,
  and compatibility-backend exclusivity.
- Static boundary receipt proving React/Tailwind/Base UI imports stay inside
  declared renderer/host modules and portable view schemas contain no
  arbitrary component, callback, ReactNode, className, credential, raw
  provider payload, absolute path, or general host authority.
- Component-state matrix covering every scoped component's empty, loading,
  live, blocked, stale, failed, interrupted, and recovered states.
- Keyboard, screen-reader, focus, contrast, reduced-motion, pointer-target,
  narrow-window, overlay-dismissal, and focus-restoration receipt.
- Timeline receipt covering stable keys, typed causal order, live updates,
  gap visibility, variable-height prepend, exact terminal disposition, and
  sanitization.
- Composer receipt covering first keystroke, IME, dynamic height, send, stop,
  steer, queue, question, approval, plan review, stale decision, and retry.
- Performance receipt for bundle size, warm launch, session-switch paint,
  long-timeline update, input latency, memory, and teardown on the admitted
  artifact and corpus.
- Installed signed-artifact journey covering local-first launch, repository
  grant, new chat, real Codex stream, blocker or review, exact read-only change,
  stop or steer, session switch, renderer reload, app restart, and same-session
  resume without duplicate execution.
- Independent visual and interaction review using real content at standard,
  narrow, long-transcript, blocked, and failure states; screenshots are review
  artifacts, not the sole correctness oracle.

## Promise Links

No public promise becomes green by adopting this ProductSpec. Phase 1 remains
closed against its exact revision-6 ProductSpec and admitted assurance record;
Phase 2 creates a new intent and proof chain. Any claim about the React-backed
workbench, launch status, supported platform, performance, accessibility, or
Codex workflow must cite the exact accepted Phase 2 artifact and current
release receipts through the owning promise registry.

## Related Artifacts

```productspec-related-artifacts
- type: product_spec
  product_spec_path: "./openagents-codex-workroom-mvp.product-spec.md"
  product_spec_revision: 6
  relation: depends_on
  title: "OpenAgents Desktop Codex Workroom MVP"
- type: engineering_spec
  url: "../effect-native/2026-07-14-react-web-renderer-harmonization-gap-analysis.md"
  title: "Effect Native and React web renderer harmonization gap analysis"
- type: code
  url: "https://github.com/pingdotgg/t3code/tree/c1ec1915fc16f3dc1ec5d47d9a97f6210a574526"
  title: "T3 Code pinned React desktop reference"
```

## Decision Trace

- **2026-07-14 — Create a new Phase 2 intent instead of revising Phase 1.**
  Phase 1 is closed and its ProductSpec, AssuranceSpec, installed receipts, and
  owner acceptance remain byte-bound historical proof. This document depends
  on that product foundation and creates new acceptance criteria for the
  visible React-backed workbench; it does not rename or retarget old evidence.
- **2026-07-14 — Keep Effect Native above React.** React renders the scoped
  Desktop components and owns renderer mechanics. Effect Native owns the
  portable view and intent contract; Effect services own application and
  durable state. Completing the full generic renderer program is not a Phase 2
  prerequisite.
- **2026-07-14 — Make ordinary Codex management the primary journey.** New,
  find, resume, read, prompt, stop or steer, resolve, review, reload, and
  restart are the completion path. ProductSpec and broader platform machinery
  remain available under their existing contracts but are not required to
  begin useful work.
- **2026-07-14 — Reconcile the unversioned shadcn and AI SDK scope additions
  as proposed revision 2.** Revision 1 remains immutable at commit
  `de1180b2da937922c2a8724915cf761f8fb78617`. Later owner-directed component
  and runtime-boundary additions changed intent-bearing bytes without the
  required revision bump. This revision repairs that identity error; it does
  not transfer the separate revision-1 owner admission to revision 2.
- **2026-07-14 — Make the transcript a conversation in proposed revision 3.**
  The owner rejected the event-log presentation and directed the workbench to
  adapt the reference's authored-message, compact-work, disclosure, and live
  streaming hierarchy. Usage, session, context, and metadata records remain
  bounded host/history data but are no longer conversation rows. Consecutive
  settled work folds behind a `Worked` disclosure; active work stays visible.
  This is a material acceptance change from revision 2 and does not inherit
  revision 1 admission or admit revision 2.
- **2026-07-14 — Owner disposition for revision 3: accepted for candidate
  construction.** The owner explicitly directed OpenAgents to update assurance
  around this UI hierarchy, produce a passing release build, and close the
  transition issues. That is the current owner disposition for revision 3 and
  authorizes RC16 candidate construction and verification. It is not a public
  release, feed-publication, or public-claim authorization.
