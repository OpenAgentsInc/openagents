# T3 Code mobile full-parity program — accepted plan and packet ledger

- Class: accepted plan and implementation admission
- Date: 2026-07-17
- Owner authority: current owner conversation
- Status: active — full mobile parity
- Base: `eb15ce99c54af497874a998192b1afbb2fa8268b`
- Reference: T3 Code `8b5469863ae1dd696e696de30240ec3da607962d`
- Gap authority: `docs/teardowns/2026-07-17-t3-code-openagents-mobile-component-gap-analysis.md`

## Owner direction

> i want full mobile parity, do the breakdown then start churning thru it

This direction admits the complete mobile parity program. “Parity” covers the
component, state-machine, interaction, accessibility, and platform behavior of
T3 Code's mobile workbench, adapted to OpenAgents' existing tokens and product
identity. It does not authorize a second application authority: the Effect
Native tree, exact confirmed refs, fail-closed execution targets, Sync
boundaries, portable-session receipts, and local credential custody remain
binding.

The repository records this accepted plan instead of creating a feature issue
because issues are reserved for reproducible bugs. Work lands as ordered,
bounded packets. No packet may claim full parity until the final component
census and signed physical-device matrix close.

## Ordered program

### Epic A — Thread Surface V2

1. `T3M-A1` — selectable rich assistant content, safe links, fenced code, and
   copy actions.
2. `T3M-A2` — grouped work log with running/success/failure summaries,
   progressive disclosure, elapsed time, and causal agent identity.
3. `T3M-A3` — compact approval, provider-input, and plan-review cards with
   complete pending/resolved/expired/revoked states.
4. `T3M-A4` — attachment cards/viewer plus streaming, pagination, unread,
   scroll-to-bottom, and deterministic anchor retention.

### Epic B — Composer Intelligence

5. `T3M-B1` — composer-local target/model/mode toolbar and grouped picker.
6. `T3M-B2` — typed slash commands, repository-backed `@` file context,
   attachment preview/removal/retry, and active-run queue/stop behavior.

### Epic C — Workspace Navigation

7. `T3M-C1` — project-aware conversation/session rows, search, filters,
   attention, lifecycle actions, and causal jumps.
8. `T3M-C2` — phone drawer, tablet split panes, route-aware native headers,
   sheets, gestures, keyboard commands, and focus restoration.

### Epic D — Files and Changes

9. `T3M-D1` — bounded repository tree and source/Markdown/image previews.
10. `T3M-D2` — changed-files summary, native diff, inline review selection,
    comment submission, authoritative writeback, and receipts.

### Epic E — Git and Terminal

11. `T3M-E1` — exact-worktree Git status, branch, commit, push, confirmation,
    conflict, failure, and evidence surfaces.
12. `T3M-E2` — native terminal sessions, reconnect/replay, keyboard accessory,
    size negotiation, bounded history, and background recovery.

### Epic F — Connections and native finish

13. `T3M-F1` — environment pairing/health, settings hierarchy, notification
    education/preferences/registration health, share intake, and inspectors.
14. `T3M-F2` — complete T3 component census, compact/regular layout matrix,
    motion/haptic polish, VoiceOver/TalkBack traversal, physical iOS/Android
    journeys, signed distribution evidence, and owner acceptance.

## Active packet — T3M-A1

Outcome: make assistant messages first-class readable mobile content without
changing transcript authority or ordering.

Owned paths:

- `apps/openagents-mobile/src/screens/mobile-transcript-content.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/src/effect-native/effect-native-host.tsx`
- `apps/openagents-mobile/tests/mobile-transcript-content.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- `apps/openagents-mobile/package.json`
- `apps/openagents.com/packages/effect-native-render-rn/src/index.ts`
- `apps/openagents.com/packages/effect-native-render-rn/src/index.test.ts`
- `pnpm-lock.yaml`
- this ledger, `docs/sol/document-manifest-policy.json`, and
  `docs/sol/document-manifest.json`

Hot contracts: Effect Native Markdown/Transcript renderer behavior, mobile
behavior-contract registry, package dependency graph, and Sol manifest. This
packet does not version a wire schema or alter conversation authority.

Required behavior:

- assistant bodies are bounded and deterministically parsed into the existing
  typed Markdown and CodeBlock catalog; arbitrary HTML never renders;
- headings, paragraphs, emphasis, inline code, safe HTTP(S) links, lists,
  blockquotes, and fenced code remain selectable and accessible;
- external links open only through the native host's safe-link capability;
- assistant prose and code expose real clipboard actions through the injected
  native clipboard driver;
- user bubble geometry, exact message keys, status, ordering, attachments,
  runtime interactions, and confirmed transcript authority remain unchanged;
- malformed or oversized markup degrades to bounded readable text.

Proof: focused parser, mobile Home/Khala, RN renderer, accessibility, behavior
contract, mobile typecheck, and repository-required `pnpm run check`.

Close rule: this closes only `T3M-A1`. Work groups, decision-card polish,
attachments/viewer, and scroll/history state remain `T3M-A2`–`T3M-A4`; full
mobile parity remains open through `T3M-F2`.

### CLAIM

- actor/session: `codex-t3-mobile-parity-20260717`
- base: `eb15ce99c54af497874a998192b1afbb2fa8268b`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-A1` rich assistant transcript content and native actions
- paths: the `T3M-A1` owned paths above
- hot files: mobile behavior registry, Effect Native RN renderer, Sol manifest policy/manifest, and lockfile
- hot contracts: Effect Native Markdown/Transcript rendering and native clipboard/link capabilities
- verification: focused packet suites, mobile typecheck, behavior/Sol checks, and `pnpm run check`
- claimed_at: `2026-07-17T18:00:00Z`

### CLAIM-STATUS

- implementation: bounded assistant Markdown and fenced code now lower into
  typed Effect Native content; native text/code selection, safe external-link
  opening, and clipboard-backed message/code actions are wired without changing
  message identity, order, authority, or user-bubble geometry
- focused proof: 76 tests passed across the new mobile transcript oracle,
  authoritative Home, accessibility, RN renderer, and behavior contracts
- type proof: OpenAgents mobile TypeScript passed
- repository gate: `pnpm run check` passed
- residual: `T3M-A2` through `T3M-F2`; this is not a full-parity claim

### CLAIM-RELEASE

- landed: `72a841ea4ef7ce5f1b4289d3b5a2b1a29e11063d` on `main`
- verification: post-rebase mobile typecheck; 76 focused mobile, renderer,
  accessibility, and behavior-contract tests; `pnpm run check`; pre-push fast
  policy, generated-contract, retired-surface, and Cloud-authority checks
- residual: `T3M-A2` grouped runtime work is the next ordered packet; full
  mobile parity remains open through `T3M-F2`

## Active packet — T3M-A2

Outcome: replace diagnostic runtime-event transcript rows with one compact,
causal work-log grammar that stays readable while a turn runs and after it
settles.

Owned paths:

- `apps/openagents-mobile/src/screens/mobile-work-log.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/tests/mobile-work-log.test.ts`
- `apps/openagents-mobile/tests/authoritative-home.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger and `docs/sol/document-manifest.json`

Hot contracts: mobile transcript entry projection, Effect Native intent
registry, mobile behavior-contract registry, and Sol manifest. This packet
does not alter runtime-event wire schemas, Sync authority, or execution
control.

Required behavior:

- confirmed reasoning, connection, tool, non-interactive plan, usage,
  reconnect, stale, interruption, terminal, and error events group by exact
  run into a bounded work log rather than competing as generic system messages;
- the group names running/success/failure/canceled state, elapsed time when
  derivable, and causal runtime/backend identity without displaying raw refs;
- the collapsed state keeps the latest five useful items and names the exact
  hidden remainder; typed disclosure reveals all bounded items;
- rows carry stable status icons/labels, concise detail, selectable full detail,
  and independent typed disclosure without changing runtime authority;
- assistant text and interaction cards retain their causal transcript order,
  and malformed timestamps/details degrade truthfully without invented time;
- accessibility exposes group/item status and expanded state with minimum
  mobile targets; disclosure remains local view state.

Proof: focused projection, Effect Native intent, authoritative Home,
accessibility, behavior-contract, mobile typecheck, and repository-required
`pnpm run check`.

Close rule: this closes grouped runtime work only. Decision-card visual parity
remains `T3M-A3`; attachment and scroll/history parity remains `T3M-A4`.

### CLAIM

- actor/session: `codex-t3-mobile-parity-a2-20260717`
- base: `4ef8dc7858aad3e07c81d4c2707257ecb28c5076`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-A2` grouped runtime work log and typed disclosure
- paths: the `T3M-A2` owned paths above
- hot files: mobile transcript projection, Home intent registry, behavior registry, and Sol manifest
- hot contracts: confirmed runtime-event presentation and Effect Native disclosure intents
- verification: focused packet suites, mobile typecheck, behavior/Sol checks, and `pnpm run check`
- claimed_at: `2026-07-17T21:03:24Z`

### CLAIM-STATUS

- implementation: confirmed runtime activity now compacts into one exact-run
  work group with causal runtime/backend identity, truthful running/settled
  summary, derivable elapsed time, five-row collapsed density, exact bounded
  remainder accounting, and typed group/item disclosure with selectable copyable
  detail; generic runtime diagnostic messages are removed
- focused proof: 60 tests passed across work-log projection/disclosure,
  authoritative Home, accessibility, and behavior contracts
- type proof: OpenAgents mobile TypeScript passed
- repository gate: `pnpm run check` passed
- residual: `T3M-A3` decision-card parity through `T3M-F2`; this is not a
  full-parity claim

### CLAIM-RELEASE

- landed: `61f8b0ad4b1f5884dccfd19aca75b034addcd927` on `main`
- verification: post-rebase mobile typecheck; 60 focused work-log,
  authoritative Home, accessibility, and behavior-contract tests; `pnpm run
  check`
- residual: `T3M-A3` compact approval/input/plan cards is the next ordered
  packet; full mobile parity remains open through `T3M-F2`

## Active packet — T3M-A3

Outcome: give each authoritative runtime interaction a compact, readable,
kind-specific transcript card without changing decision authority.

Owned paths:

- `apps/openagents-mobile/src/screens/mobile-interaction-card.ts`
- `apps/openagents-mobile/src/screens/mobile-transcript-content.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/tests/mobile-interaction-card.test.ts`
- `apps/openagents-mobile/tests/authoritative-home.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger and `docs/sol/document-manifest.json`

Hot contracts: runtime-interaction presentation, existing typed decision
intents, mobile behavior-contract registry, and Sol manifest. This packet does
not version the runtime-interaction schema or add a new decision outcome.

Required behavior:

- tool approval, provider input, and plan review use distinct compact card
  hierarchy, status tone, disclosure copy, and action emphasis;
- approvals summarize the bounded requested operation, state that allowance is
  request-scoped, and distinguish allow-once from deny without inventing a
  session-wide grant;
- provider questions group prompts, distinguish single from multiple choice,
  show option descriptions, selected state, validation, and submitting state;
  free text appears only when future authority explicitly allows it;
- plans render as readable bounded Markdown with anchored accept,
  request-changes, and replan actions;
- pending, submitting, resolved, expired, and revoked states are complete;
  terminal states are read-only and show a confirmed audit summary without
  displaying raw refs;
- all actions retain exact interaction/thread/turn/kind/idempotency authority,
  minimum touch targets, labels, and disabled/loading semantics.

Proof: focused interaction-card state matrix, existing authoritative decision
journeys, accessibility, behavior contracts, mobile typecheck, and
repository-required `pnpm run check`.

Close rule: this closes interaction-card presentation only. Attachments,
viewer, streaming, and scroll/history parity remain `T3M-A4`.

### CLAIM

- actor/session: `codex-t3-mobile-parity-a3-20260717`
- base: `f80b770d815154d33c6bf09649671e653ae3b293`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-A3` compact authoritative approval/input/plan cards
- paths: the `T3M-A3` owned paths above
- hot files: interaction renderer, behavior registry, and Sol manifest
- hot contracts: existing runtime-interaction decision intent and terminal-state presentation
- verification: focused packet suites, mobile typecheck, behavior/Sol checks, and `pnpm run check`
- claimed_at: `2026-07-17T21:11:39Z`

### CLAIM-STATUS

- status: implementation complete; ready to commit and land
- completed_at: `2026-07-17T21:15:38Z`
- delivered: kind-specific compact provider-question, request-scoped tool
  approval, and plan-review cards; bounded plan Markdown and native copy;
  complete selected/validation/submitting and terminal state presentation;
  accessible 44pt minimum actions; no invented session grant or free-text
  authority
- verification: mobile typecheck passed; 63 focused interaction-card,
  authoritative Home, accessibility, and behavior-contract tests passed;
  `pnpm run check` passed
- close boundary: presentation only; exact confirmed decision settlement and
  runtime schemas remain unchanged; `T3M-A4` remains next

### CLAIM-RELEASE

- landed: `1a47f6252890eed3a30de86f59c1d601c1621b0f` on `main`
- verification: post-rebase mobile typecheck; 63 focused interaction-card,
  authoritative Home, accessibility, and behavior-contract tests; pre-rebase
  `pnpm run check`
- residual: `T3M-A4` attachments, viewer, streaming, scroll/history, and
  deep-transcript parity is the next ordered packet; full mobile parity remains
  open through `T3M-F2`

## Active packet — T3M-A4

Outcome: finish the mobile transcript's media and long-running-feed behavior:
honest attachment previews/viewer, stable in-place active work, user-controlled
auto-pin, unread recovery, and deterministic retained-history pagination.

Owned paths:

- `apps/openagents.com/packages/effect-native-core/src/index.ts`
- `apps/openagents.com/packages/effect-native-render-rn/src/index.ts`
- `apps/openagents.com/packages/effect-native-render-rn/src/index.test.ts`
- `apps/openagents-mobile/src/screens/mobile-transcript-attachment.ts`
- `apps/openagents-mobile/src/screens/mobile-transcript-history.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/tests/mobile-transcript-attachment.test.ts`
- `apps/openagents-mobile/tests/mobile-transcript-history.test.ts`
- `apps/openagents-mobile/tests/authoritative-home.test.ts`
- `apps/openagents-mobile/tests/mobile-accessibility.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger, `docs/sol/document-manifest-policy.json`, and
  `docs/sol/document-manifest.json`

Hot contracts: Effect Native Image and Transcript catalogs/renderers, exact
confirmed transcript ordering, retained-history accounting, device-local view
state, mobile behavior registry, and Sol manifest. The wire attachment schema
and Sync retention policy are not versioned here.

Required behavior:

- confirmed image attachments use content-aware thumbnail cards with accessible
  file metadata, native load/failure/retry state, and a dismissable full-screen
  contain-fit viewer; unsupported non-image payloads are never fabricated from
  the image-only authority schema;
- active reasoning/work/message rows retain stable keys and update in place;
  reduced motion never gates content or uses app-owned animation;
- auto-pin follows the latest row only while the user remains at the end; user
  scrolling suspends it, new rows increment an inline unread boundary, and a
  touch-safe jump-to-latest action clears the boundary and restores pinning;
- retained history opens newest-first in bounded pages, loads earlier retained
  rows through a typed local action, preserves the first visible keyed row when
  prepending, and names server-retained omissions exactly;
- list rendering remains virtualized and bounded, and stale/foreign image,
  pagination, scroll, or load callbacks fail closed.

Proof: focused attachment, history/scroll-state, RN renderer, authoritative
Home, accessibility, behavior-contract, mobile typecheck, and repository
checks.

Close rule: this closes Epic A's transcript surface only. Composer intelligence
begins at `T3M-B1`; physical image gestures and screen-reader/device evidence
remain `T3M-F2`.

### CLAIM

- actor/session: `codex-t3-mobile-parity-a4-20260717`
- base: `e39187ef97d2777ef21a609f3a0596cb607d34dd`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-A4` attachment viewer and deterministic transcript feed state
- paths: the `T3M-A4` owned paths above
- hot files: Effect Native Image/Transcript catalog and RN renderer, Home
  intent registry, Khala transcript state, behavior registry, Sol manifest
- hot contracts: typed native events, confirmed entry keys/order, retained
  omission accounting, request-local view authority
- verification: focused packet suites, renderer tests, mobile typecheck,
  behavior/Sol checks, and `pnpm run check`
- claimed_at: `2026-07-17T21:19:29Z`

### CLAIM-STATUS

- status: implementation complete; ready to commit and land
- completed_at: `2026-07-17T21:35:03Z`
- delivered: typed Effect Native image press/load/error lifecycle; confirmed
  attachment loading/failure/retry cards and ready-only contain-fit viewer;
  60-row retained-history pagination with exact non-retained accounting;
  keyed anchor preservation and targeted-scroll fallback; real end-pin
  detection, inline unread boundary, jump-to-latest, and stable-key active-row
  replacement; reduced-motion-safe and Dynamic-Type touch targets
- verification: 94 focused transcript, interaction, authoritative Home,
  accessibility, local-first registry, RN renderer, and behavior-contract tests
  passed; seven Effect Native vendor-guard tests passed; Effect Native core,
  RN renderer, and mobile typechecks passed; `pnpm run check` passed
- expanded-sweep note: the package-level `test` script ran the unscoped whole
  workspace (17,982 passed / 21 failed); failures were current-head/shared-host
  Postgres SHM exhaustion, Pylon/packaging timeouts, fixture drift, and Git
  `core.bare` mutation. Its one relevant registry failure exposed three
  presentation contracts carrying invalid seam metadata; that metadata was
  removed and the focused local-first registry oracle is green.
- close boundary: no attachment wire-schema widening or remote retention claim;
  physical zoom/VoiceOver/TalkBack evidence remains `T3M-F2`; `T3M-B1` is next

### CLAIM-RELEASE

- landed: `f337bbaf1bce8c40b9d22dfbe05c98f904856d66` on `main`
- verification: post-rebase 113 focused transcript, interaction,
  authoritative Home, accessibility, local-first registry, RN renderer,
  behavior-contract, and Sol tests; Effect Native core, RN renderer, and
  mobile typechecks; pre-rebase vendor guard and `pnpm run check`
- residual: Epic A is complete; `T3M-B1` composer-local target/model/mode
  toolbar and grouped picker is the next ordered packet; full mobile parity
  remains open through `T3M-F2`

## Active packet — T3M-B1

Outcome: turn the coding composer into one coherent input instrument by moving
confirmed repository, target, model, provider, readiness, and mode presentation
into a compact toolbar and grouped native picker.

Owned paths:

- `apps/openagents-mobile/src/screens/mobile-composer-toolbar.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/tests/mobile-composer-toolbar.test.ts`
- `apps/openagents-mobile/tests/authoritative-home.test.ts`
- `apps/openagents-mobile/tests/mobile-accessibility.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger and `docs/sol/document-manifest.json`

Hot contracts: authenticated execution-target catalog, persisted composer
selection, Effect Native sheet/text-field intents, accessibility, mobile
behavior registry, and Sol manifest. This packet does not split model from its
authoritative execution target or enable shell/command semantics reserved for
`T3M-B2`.

Required behavior:

- repository/worktree identity and the current target/model become one compact
  composer-local toolbar instead of a diagnostic block above the input;
- the target control opens a dismissable, searchable native sheet grouped by
  provider, with selected state, model label, account label, readiness, and
  exact unavailable/revoked/offline explanations;
- selection continues through the existing typed target intent and persisted
  draft mutation; stale, foreign, non-ready, or missing targets fail closed;
- current mode is visibly `Code`; no alternate/shell mode is presented as
  actionable until `T3M-B2` owns command admission and transport;
- catalog-unavailable and empty-search states preserve the draft and explain
  why target choice is unavailable;
- actions meet Dynamic Type touch targets and sheet dismissal restores the
  composer path without changing transcript authority.

Proof: focused toolbar/picker projection and intent journeys, existing
authoritative target persistence, accessibility, behavior contracts, mobile
typecheck, and `pnpm run check`.

Close rule: this closes composer target/model/mode presentation only. Slash
commands, `@` context, attachment editing, and active-run queue/stop remain
`T3M-B2`.

### CLAIM

- actor/session: `codex-t3-mobile-parity-b1-20260717`
- base: `55b81e1e7f504f002285f3dbce961f999d542f04`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-B1` composer toolbar and grouped authoritative target picker
- paths: the `T3M-B1` owned paths above
- hot files: Home state/intent registry, Khala composer presentation, behavior
  registry, Sol manifest
- hot contracts: exact target catalog/persistence and non-actionable Code mode
- verification: focused mobile suites, typecheck, behavior/Sol checks, and
  `pnpm run check`
- claimed_at: `2026-07-17T21:37:57Z`

### CLAIM-STATUS

- status: implementation complete and release gates green
- verification: 62 focused composer-toolbar, authoritative Home,
  accessibility, local-first registry, and behavior-contract tests; mobile
  typecheck; `pnpm run check`; `git diff --check`
- state-safety note: picker visibility and search reset on chat creation,
  conversation selection, coding-session selection, attention navigation,
  active-thread removal, and sync-authority loss
- close boundary: no slash-command, `@` context, attachment editing, or
  active-run queue/stop claim; those remain `T3M-B2`
- verified_at: `2026-07-17T21:42:38Z`
