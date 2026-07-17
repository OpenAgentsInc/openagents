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

### CLAIM-RELEASE

- landed: `9cc111a8ee5a07a369c24752411d9828e181d9d3` on `main`
- verification: post-rebase 81 focused composer-toolbar, authoritative Home,
  accessibility, local-first registry, behavior-contract, and Sol tests;
  mobile typecheck; pre-rebase `pnpm run check` and manifest generation
- residual: `T3M-B1` is complete; `T3M-B2` slash commands, `@` context,
  attachment editing, and active-run queue/stop is the next ordered packet;
  full mobile parity remains open through `T3M-F2`

## Active packet — T3M-B2.1

Outcome: make draft attachments first-class composer content before adding the
command/context discovery layer: compact image/file previews, exact per-item
state, removal, and bounded retry without weakening device-local byte
verification.

Owned paths:

- `apps/openagents-mobile/src/coding/mobile-coding-composer.ts`
- `apps/openagents-mobile/src/coding/mobile-coding-attachment-picker.ts`
- `apps/openagents-mobile/src/coding/expo-mobile-coding-attachment-picker.ts`
- `apps/openagents-mobile/src/screens/mobile-composer-attachments.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/src/app.tsx`
- `apps/openagents-mobile/tests/mobile-composer-attachments.test.ts`
- `apps/openagents-mobile/tests/mobile-coding-composer.test.ts`
- `apps/openagents-mobile/tests/mobile-coding-attachment-picker.test.ts`
- `apps/openagents-mobile/tests/authoritative-home.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- `packages/khala-sync-client/src/index.ts`
- this ledger and `docs/sol/document-manifest.json`

Hot contracts: canonical composer transactions, persisted private drafts,
device-local attachment bytes, exact draft identity, Dynamic Type controls,
behavior registry, and Sol manifest.

Required behavior:

- every selected attachment renders in a bounded composer strip as an image
  preview or file card with filename, size, type, and exact staged/uploading/
  ready/error state;
- remove applies the canonical `RemoveAttachment` transaction to the exact
  active draft, persists it, and cannot remove a stale or foreign attachment;
- retry is shown only for a failed item and routes through a host verifier that
  must re-read and re-hash the original device-local bytes before the item can
  return to ready;
- pending mutation disables only the affected item and preserves the text,
  target, other attachments, and transcript;
- limits and delivery failures remain explicit; no binary-delivery or remote-
  upload support is implied.

Proof: canonical composer transaction tests, attachment projection/intent
journeys, authoritative submit regression, accessibility, behavior contracts,
mobile typecheck, and repository checks.

Close rule: this closes attachment preview/removal/retry only. Typed slash
commands and repository-backed `@` context remain `T3M-B2.2`; active-run
queue/stop remains `T3M-B2.3`.

### CLAIM

- actor/session: `codex-t3-mobile-parity-b2-1-20260717`
- base: `eae0c55d660812bdb630017bae5599c08a09ce0d`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-B2.1` draft attachment preview, exact removal, and verified retry
- paths: the `T3M-B2.1` owned paths above
- hot files: canonical composer exports, Home intent registry, Khala composer
  presentation, native binding, behavior registry, Sol manifest
- verification: focused mobile/composer suites, typecheck, behavior/Sol checks,
  and `pnpm run check`
- claimed_at: `2026-07-17T21:45:27Z`

### CLAIM-STATUS

- status: implementation complete and release gates green
- verification: 71 focused composer attachment, canonical draft, native
  picker/delivery, authoritative Home, accessibility, local-first registry,
  and behavior-contract tests; Khala Sync client and mobile typechecks;
  `pnpm run check`; `git diff --check`
- authority note: preview URIs name only the app-managed persistent copy;
  remove uses canonical composer transactions; retry requires exact failed
  attachment identity plus matching size and SHA-256 from re-read local bytes
- close boundary: no binary runtime delivery or remote upload claim; slash and
  `@` discovery remain `T3M-B2.2`; queue/stop remains `T3M-B2.3`
- verified_at: `2026-07-17T21:50:50Z`

### CLAIM-RELEASE

- landed: `baec3e49b1d3b61171a8c8d3780f20b2523ef791` on `main`
- verification: post-rebase 90 focused composer attachment, canonical draft,
  picker/delivery, authoritative Home, accessibility, local-first registry,
  behavior-contract, and Sol tests; Khala Sync client and mobile typechecks;
  pre-rebase `pnpm run check`
- residual: `T3M-B2.1` is complete; `T3M-B2.2` typed slash-command and
  repository-backed `@` context discovery is next; full mobile parity remains
  open through `T3M-F2`

## Active packet — T3M-B2.2a

Outcome: add the closed typed slash-command discovery and dispatch half of
composer discovery before binding repository-backed `@` results in B2.2b.

Owned paths:

- `apps/openagents-mobile/src/screens/mobile-composer-discovery.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/tests/mobile-composer-discovery.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger and `docs/sol/document-manifest.json`

Hot contracts: explicit composer trigger parsing, closed command registry,
exact intent dispatch, active-turn authority, accessibility, behavior registry,
and Sol manifest.

Required behavior:

- a leading slash token opens composer-local autocomplete with bounded built-in
  commands, typed labels/descriptions, filtered results, and an honest empty
  state;
- selection accepts only a closed command id and dispatches its exact existing
  Home intent/authority path; arbitrary command text never becomes tool or
  runtime routing;
- target and attachment commands preserve the remaining draft, while new-chat
  and stop use their existing authority checks and clear command text only
  after admission;
- unavailable commands remain visible with a reason and cannot dispatch;
- touch, screen-reader, and keyboard selection use the Effect Native Combobox
  contract rather than a bespoke overlay.

Proof: command projection/filtering, selection/refusal journeys, existing Home
authority regressions, accessibility, RN Composer/Combobox tests, behavior
contracts, mobile typecheck, and repository checks.

Close rule: this closes slash-command discovery only. Repository-backed `@`
context remains `T3M-B2.2b`; active-run queue/stop presentation remains
`T3M-B2.3` (the slash Stop command reuses current exact cancel authority).

### CLAIM

- actor/session: `codex-t3-mobile-parity-b2-2a-20260717`
- base: `74e308ba17041f97a81dedf78eeaea34132dcd21`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-B2.2a` typed composer slash-command discovery and dispatch
- paths: the `T3M-B2.2a` owned paths above
- verification: focused discovery/Home/renderer/accessibility suites,
  typecheck, behavior/Sol checks, and `pnpm run check`
- claimed_at: `2026-07-17T21:53:30Z`

### CLAIM-STATUS

- status: implementation complete and release gates green
- verification: 84 focused slash discovery, authoritative Home, composer
  toolbar/attachments, accessibility, local-first registry, RN Composer/
  Combobox, and behavior-contract tests; mobile typecheck; `pnpm run check`;
  `git diff --check`
- routing note: deterministic parsing starts only at the explicit trailing
  slash token; selection is a closed schema literal and dispatches existing
  Home authority paths, never free-form tool selection
- close boundary: repository-backed `@` context remains `T3M-B2.2b`;
  active-run queue/stop presentation remains `T3M-B2.3`
- verified_at: `2026-07-17T21:56:45Z`

### CLAIM-RELEASE

- landed: `14b41ed2b25f0ce5f24f73ade8a911c815b9aa2e` on `main`
- verification: post-rebase 103 focused slash discovery, authoritative Home,
  composer toolbar/attachments, accessibility, local-first registry, RN
  Composer/Combobox, behavior-contract, and Sol tests; mobile typecheck;
  pre-rebase `pnpm run check`
- residual: `T3M-B2.2a` is complete; repository-backed `@` context remains
  `T3M-B2.2b`; full mobile parity remains open through `T3M-F2`

## Active packet — T3M-B2.2b

Outcome: add the exact repository/worktree-scoped `@` path-search contract,
autocomplete states, and mention insertion boundary. This slice must expose
missing environment transport honestly; it cannot manufacture paths from the
coding catalog's display labels.

Owned paths:

- `apps/openagents-mobile/src/coding/mobile-composer-path-context.ts`
- `apps/openagents-mobile/src/screens/mobile-composer-discovery.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/tests/mobile-composer-path-context.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger and `docs/sol/document-manifest.json`

Hot contracts: exact repository/worktree identity, bounded environment query,
stale-result rejection, safe relative paths, canonical draft text, Effect
Native mention autocomplete, behavior registry, and Sol manifest.

Required behavior:

- an explicit trailing `@` token queries at most twenty entries against the
  exact repository and worktree refs already bound to the composer;
- results carry safe relative path, kind, and revision identity; stale,
  foreign, traversal, oversized, or duplicate results are rejected;
- loading, empty, unavailable, and failed states render honestly in the native
  composer autocomplete;
- selecting an exact current result replaces only the trigger token with a
  mention and persists the resulting draft text; arbitrary path text cannot
  bypass the result set;
- when no environment search transport is connected, the UI says so and keeps
  the draft. Full transport closure is a dependency shared with `T3M-D1` and
  `T3M-F1`, not permission to synthesize repository contents.

Proof: decoder/path safety tests, stale query races, Home selection/refusal,
autocomplete states, authoritative draft regressions, accessibility, behavior
contracts, mobile typecheck, and repository checks.

Close rule: this closes the mobile query/presentation/admission boundary. A
real paired environment provider remains a named `T3M-D1`/`T3M-F1` release
dependency; active-run queue/stop remains `T3M-B2.3`.

### CLAIM

- actor/session: `codex-t3-mobile-parity-b2-2b-20260717`
- base: `c6abfbf69e67f37b643dbee996e228b6aebe7200`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-B2.2b` exact path-search, autocomplete, and mention admission
- paths: the `T3M-B2.2b` owned paths above
- verification: focused path/Home/accessibility suites, behavior/Sol checks,
  mobile typecheck, and `pnpm run check`
- claimed_at: `2026-07-17T21:59:09Z`

### CLAIM-STATUS

- status: implementation complete and release gates green
- verification: 87 focused repository-path, slash discovery, composer toolbar/
  attachments, authoritative Home, accessibility, local-first registry, RN
  renderer, and behavior-contract tests; mobile typecheck; `pnpm run check`;
  `git diff --check`
- authority note: queries and decoded pages must match the exact composer-bound
  repository/worktree/query; only a current decoded result can be inserted, and
  stale, foreign, traversal, oversized, or duplicate results are refused
- close boundary: the current mobile composition reports an unavailable search
  transport honestly; connecting a real paired environment provider remains a
  `T3M-D1`/`T3M-F1` release dependency; queue/stop remains `T3M-B2.3`
- verified_at: `2026-07-17T22:04:49Z`

### CLAIM-RELEASE

- landed: `31f2aa4683055c9e75ac911a53a070e4fecf44c3` on `main`
- verification: post-rebase 106 repository-path, slash discovery, composer
  toolbar/attachments, authoritative Home, accessibility, local-first registry,
  RN renderer, behavior-contract, and Sol tests; mobile typecheck; pre-rebase
  `pnpm run check`
- residual: `T3M-B2.2b` query/presentation/admission is complete; the real paired
  environment provider remains a `T3M-D1`/`T3M-F1` dependency; active-run
  queue/stop presentation is next in `T3M-B2.3`; full mobile parity remains open
  through `T3M-F2`

## Active packet — T3M-B2.3a

Outcome: make active-run composer admission and Stop behavior explicit before
adding a real durable queue-next transport: exact same-run steering for every
steerable confirmed state, a compact composer-local status, and confirmed Stop
with destructive confirmation.

Owned paths:

- `apps/openagents.com/packages/effect-native-core/src/index.ts`
- `apps/openagents.com/packages/effect-native-render-rn/src/index.ts`
- `apps/openagents.com/packages/effect-native-render-rn/src/index.test.ts`
- `apps/openagents-mobile/src/screens/mobile-composer-run-control.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/src/conversation/mobile-conversation.ts`
- `apps/openagents-mobile/tests/mobile-composer-run-control.test.ts`
- `apps/openagents-mobile/tests/mobile-conversation.test.ts`
- `apps/openagents-mobile/tests/authoritative-home.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger and `docs/sol/document-manifest.json`

Hot contracts: exact active thread/run identity, provider-neutral follow-up vs
new-turn distinction, confirmed runtime control, Effect Native Composer action,
destructive confirmation, behavior registry, and Sol manifest.

Required behavior:

- queued, running, waiting-for-input, and stop-pending states are named beside
  the composer; the placeholder and action copy state whether text will steer
  the exact current run, wait, or start a new turn;
- running and waiting-for-input follow-ups use the exact active run and never
  fall through to start a concurrent turn;
- when an active draft is empty, the composer action becomes Stop; a first tap
  requests confirmation and only the exact current run can be confirmed;
- Stop pending preserves the editable draft, disables duplicate control, and
  remains pending until a confirmed runtime update or typed failure;
- terminal and replaced runs clear stale confirmation; unavailable control
  authority remains visibly disabled;
- this slice does not call same-run steering a durable queue. A provider-neutral
  `turn.queue` adapter remains the immediately following `T3M-B2.3b` slice.

Proof: admission projection, renderer action, Home confirmation/refusal,
same-run conversation dispatch, authoritative transcript/composer,
accessibility, behavior contracts, mobile/package typechecks, and repository
checks.

Close rule: this closes exact active-run admission, steering, and composer Stop.
It does not close B2.3 queue parity; `T3M-B2.3b` must connect a real
provider-neutral queue-next transport and render its durable outcome.

### CLAIM

- actor/session: `codex-t3-mobile-parity-b2-3a-20260717`
- base: `6d73faee9ec885784531a36f7f3da7b85aa1db1c`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-B2.3a` exact active-run admission, steering, and confirmed Stop
- paths: the `T3M-B2.3a` owned paths above
- verification: focused run-control/Home/conversation/renderer/accessibility
  suites, behavior/Sol checks, package/mobile typechecks, and `pnpm run check`
- claimed_at: `2026-07-17T22:07:36Z`

### CLAIM-STATUS

- status: implementation complete and release gates green
- verification: 109 focused run-control, mobile conversation, authoritative
  Home, composer discovery/toolbar/attachments, accessibility, local-first,
  Effect Native core/native renderer, and behavior-contract tests; Effect
  Native core, native renderer, and mobile typechecks; `pnpm run check`;
  `git diff --check`
- authority note: running and waiting follow-ups bind to the exact confirmed
  run; empty composer Stop first creates an exact-run confirmation and dispatch
  remains pending until confirmed runtime replacement
- close boundary: same-run steering and confirmed composer Stop are complete;
  no durable queue-next claim is made, and provider-neutral `turn.queue`
  transport remains the immediately following `T3M-B2.3b` slice
- verified_at: `2026-07-17T22:13:41Z`

### CLAIM-RELEASE

- landed: `05d6bf4d0d8ab92000063c471156bb484dfc7f7f` on `main`
- verification: post-rebase 128 active-run control, conversation, authoritative
  Home, composer discovery/toolbar/attachments, accessibility, local-first,
  Effect Native core/native renderer, behavior-contract, and Sol tests; Effect
  Native core, native renderer, and mobile typechecks; pre-rebase `pnpm run check`
- residual: `T3M-B2.3a` exact-run steering and confirmed composer Stop are
  complete; provider-neutral durable queue-next remains `T3M-B2.3b`; full
  mobile parity remains open through `T3M-F2`

## Active packet — T3M-B2.3b

Outcome: expose the real Pylon queue-until-idle behavior through an explicit
provider-neutral `turn.queue` mobile adapter, settle the composer on confirmed
admission rather than terminal completion, and render the queued follow-up's
separate admission/delivery/promotion state.

Owned paths:

- `apps/openagents-mobile/src/conversation/mobile-runtime-queue.ts`
- `apps/openagents-mobile/src/conversation/mobile-conversation.ts`
- `apps/openagents-mobile/src/screens/mobile-composer-run-control.ts`
- `apps/openagents-mobile/src/screens/khala-core.ts`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/tests/mobile-runtime-queue.test.ts`
- `apps/openagents-mobile/tests/mobile-conversation.test.ts`
- `apps/openagents-mobile/tests/mobile-composer-run-control.test.ts`
- `apps/openagents-mobile/tests/authoritative-home.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.runtime_control_intent.v2`, exact thread/run/message
identity, legacy Pylon queue-only lowering, distinct admission/delivery/terminal
axes, confirmed transcript state, behavior registry, and Sol manifest.

Required behavior:

- text submitted during a confirmed running or waiting turn mints exact
  `turn.queue` identity and lowers only through the proven queue-until-idle
  `message.append` adapter; it is never labeled or dispatched as steer;
- queue identity binds the exact thread, current run generation, durable chat
  message ref, ordering key, origin, idempotency key, and deadline;
- the composer clears only after the chat message and legacy queue mutation are
  confirmed admitted; pending, expired, rejected, or mismatched outcomes retain
  the draft with explicit copy;
- a compact queued-follow-up receipt names accepted admission separately from
  pending delivery/promotion and remains until a replacement run proves
  promotion or a typed failure replaces it;
- Stop continues to target the active run and never cancels or silently consumes
  the queued follow-up;
- this packet corrects B2.3a presentation language: current Pylon
  `message.append` is queue-until-idle, not literal mid-stream steering.

Proof: queue intent decoder/replay, exact adapter lowering, pending/rejection,
Home draft/receipt journey, run replacement, Stop coexistence, accessibility,
behavior contracts, mobile typecheck, and repository checks.

Close rule: this closes B2.3 mobile queue/Stop product semantics for the current
Pylon runtime adapter. Cross-restart delivery proof and physical-device evidence
remain release gates in `T3M-F1`/`T3M-F2`; genuine provider steering remains a
separate capability and is not claimed.

### CLAIM

- actor/session: `codex-t3-mobile-parity-b2-3b-20260717`
- base: `ecb39424af6798d40a427b99f8882eedfc8f4a63`
- worktree/branch: `openagents-t3-mobile-20260717` / detached `origin/main`
- scope: `T3M-B2.3b` provider-neutral queue adapter and mobile admission receipt
- paths: the `T3M-B2.3b` owned paths above
- verification: focused queue/conversation/Home/run-control/accessibility
  suites, behavior/Sol checks, mobile typecheck, and `pnpm run check`
- claimed_at: `2026-07-17T22:18:07Z`

### CLAIM-STATUS

- status: implementation complete and release gates green
- verification: 118 focused provider-neutral queue, mobile conversation,
  run-control, authoritative Home, composer discovery/toolbar/attachments,
  accessibility, local-first, native renderer, agent-runtime-schema, and
  behavior-contract tests; mobile typecheck; `pnpm run check`;
  `git diff --check`
- authority note: active text mints `turn.queue` bound to exact confirmed
  thread/run generation/message identity and lowers only to Pylon's existing
  queue-until-idle adapter; admission, delivery, and promotion remain distinct
- correction: B2.3a's interim "steer" presentation is superseded for the
  current Pylon adapter; `message.append` is queue-until-idle and the product
  now labels it Queue without claiming genuine mid-stream steering
- close boundary: B2.3 mobile queue/Stop semantics are complete for the current
  adapter; cross-restart promotion proof remains `T3M-F1` and physical-device
  evidence remains `T3M-F2`
- verified_at: `2026-07-17T22:23:13Z`

### CLAIM-RELEASE

- landed: `9c380f28430e697e431f5c32d8b3efe273473bf6` on `main`
- verification: post-first-rebase 137 provider-neutral queue, conversation,
  run-control, authoritative Home, composer, accessibility, local-first, native
  renderer, schema, behavior-contract, and Sol tests plus mobile typecheck;
  post-final-rebase 62 queue/conversation/Home/local-first/Sol tests plus mobile
  typecheck; pre-rebase `pnpm run check`
- residual: `T3M-B2.3` mobile queue/Stop semantics are complete for the current
  Pylon adapter; transcript/composer group B is complete; ordered parity work
  advances to `T3M-C1`; full mobile parity remains open through `T3M-F2`

## Active packet — T3M-C1

Outcome: replace the administrative mobile drawer with one bounded,
project-aware workspace navigation grammar spanning confirmed conversations,
coding sessions, attention, and archived threads.

Owned paths:

- `apps/openagents-mobile/src/screens/mobile-workspace-navigation.ts`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/tests/mobile-workspace-navigation.test.ts`
- `apps/openagents-mobile/tests/authoritative-home.test.ts`
- `apps/openagents-mobile/tests/mobile-accessibility.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger, `docs/sol/document-manifest-policy.json`, and
  `docs/sol/document-manifest.json`

Hot contracts: confirmed conversation and coding-directory projections,
attention-target identity, Effect Native intent dispatch, thread lifecycle
writeback, accessibility, and Sol manifest. This packet does not add a remote
query, hydrate withheld cache rows, or change conversation authority.

Required behavior:

- one bounded row model joins coding sessions to repository/project,
  branch/worktree identity, recency, run/readiness state, attention, selected
  state, and exact navigation intent while ordinary chat rows retain confirmed
  thread identity and sync state;
- local search and status/project filters are deterministic, bounded, and
  operate only on already-authorized projections, with predictable empty
  results and a clear-filters path;
- pending attention appears in the same row grammar and dispatches the exact
  confirmed attention target so selection lands on the causal transcript card;
- conversation lifecycle controls are row-local, archived threads have a
  dedicated filtered destination, and rename/delete confirmation remains
  explicit and server-authoritative;
- raw refs remain out of the primary row label, withheld cached rows remain
  counted but hidden, and all controls preserve Dynamic Type touch targets and
  accessible labels.

Proof: focused workspace-navigation projection, authoritative Home, lifecycle,
attention, accessibility, behavior-contract, mobile typecheck, and
repository-required checks.

Close rule: this closes only `T3M-C1`. Adaptive phone/tablet shell composition,
native headers/sheets/gestures/keyboard focus, and all files/change/Git/terminal
surfaces remain `T3M-C2` through `T3M-F2`; this is not a full-parity claim.

### CLAIM

- packet: `T3M-C1`
- base: `1b8a738b7df2a8c614b148476b8004719f11b6a4`
- claimed_at: `2026-07-17T22:27:38Z`
- scope: bounded project-aware workspace row projection; drawer search,
  filters, attention causal jumps, row-local lifecycle actions, archived
  destination, tests, behavior contract, and Sol receipt
- exclusions: adaptive shell/native navigation (`T3M-C2`), repository files and
  changes (`T3M-D1`/`T3M-D2`), Git/terminal (`T3M-E1`/`T3M-E2`), and physical
  device/distribution evidence (`T3M-F1`/`T3M-F2`)

### CLAIM-RELEASE

- landed: `159f6dff775b35cf19e183e9a58997d10468d413` on `main`
- verification: post-rebase 195 mobile tests, mobile typecheck, and 19 Sol
  policy tests; pre-rebase repository lint/format and the same mobile/Sol
  suites; `git diff --check`
- authority note: rows project only confirmed conversation, coding-directory,
  and personal-attention state; exact refs stay in typed actions, invalid or
  withheld authority stays hidden, and lifecycle effects remain confirmed by
  the existing server writeback path
- close boundary: `T3M-C1` project-aware rows, bounded local search/status and
  project filters, causal attention jumps, row-local lifecycle actions, and
  archived navigation are complete; adaptive/native workspace composition is
  `T3M-C2`, and full mobile parity remains open through `T3M-F2`
- verified_at: `2026-07-17T22:34:08Z`

## Active packet — T3M-C2.1

Outcome: make the workspace shell explicitly compact/regular so phones retain
a single-task drawer while tablets keep navigation and transcript visible in a
stable split workspace with route-aware chrome and focus return.

Owned paths:

- `apps/openagents-mobile/src/screens/mobile-adaptive-workspace.ts`
- `apps/openagents-mobile/src/screens/home-screen.tsx`
- `apps/openagents-mobile/src/screens/home-core.ts`
- `apps/openagents-mobile/tests/mobile-adaptive-workspace.test.ts`
- `apps/openagents-mobile/tests/authoritative-home.test.ts`
- `apps/openagents-mobile/tests/mobile-accessibility.test.ts`
- `apps/openagents-mobile/src/contracts/ux-contracts.ts`
- this ledger and Sol manifest files

Required behavior: width classification is deterministic and bounded; compact
mode keeps the current full-screen drawer/content exclusivity; regular mode
uses the typed Effect Native split-pane contract with persistent navigation and
detail, a bounded adjustable sidebar, no duplicate transcript authority, and a
route-aware header/navigation affordance; transitions restore focus to the
selected row or main transcript through typed serializable focus identity.

Close rule: this closes only adaptive composition and route/focus semantics.
Native sheet presentation, swipe actions, hardware keyboard commands, and
their physical-device evidence remain `T3M-C2.2`; files/change/Git/terminal and
release finish remain `T3M-D1` through `T3M-F2`.

### CLAIM

- packet: `T3M-C2.1`
- base: `70bd452ded74a7d12aee5becee718bef7add2aaa`
- claimed_at: `2026-07-17T22:35:01Z`
- scope: compact/regular layout projection, phone exclusivity, tablet split
  navigation/detail, typed resize/collapse state, route-aware header behavior,
  focus restoration identity, tests, behavior contract, and Sol receipt
- exclusions: sheets/swipes/hardware shortcuts (`T3M-C2.2`), workbench feature
  routes (`T3M-D1`–`T3M-E2`), and physical distribution evidence (`T3M-F2`)

### CLAIM-RELEASE

- landed: `420fa1b5ff43ce065f1b52b9b45f1f843ce62212` on `main`
- verification: pre-rebase 200 mobile tests, mobile and RN-renderer typechecks,
  repository lint/format, and 19 Sol tests; post-rebase 47 adaptive/Home/
  accessibility/renderer/behavior tests, mobile typecheck, and 19 Sol tests;
  `git diff --check`
- authority note: viewport changes update only layout state in the live Home
  program, preserving draft/transcript authority; regular mode mounts the sole
  detail tree once beside confirmed navigation through typed SplitPane data
- close boundary: `T3M-C2.1` compact/regular composition, bounded sidebar,
  route-aware navigation copy, rotation continuity, and focus-return identity
  are complete; native sheet/swipe/keyboard focus behavior remains `T3M-C2.2`
  and full mobile parity remains open through `T3M-F2`
- verified_at: `2026-07-17T22:39:47Z`

## Active packet — T3M-C2.2a

Outcome: move compact lifecycle/context controls into a native-lowered sheet
and express row archive/restore/delete affordances through the typed swipe-item
contract with an accessible press fallback.

Required behavior: compact More opens one dismissable bottom sheet bound to the
exact thread; regular layout retains its in-pane actions; active and archived
rows expose only valid lifecycle actions; full-swipe defaults only to reversible
archive/restore, never delete; delete remains a second explicit confirmation;
stale/foreign action IDs and dismissal during confirmed writeback are refused.

Close rule: this closes sheet and row-action composition only. A genuine native
gesture driver plus hardware keyboard commands and host focus application remain
`T3M-C2.2b`; physical gesture/focus evidence remains `T3M-F2`.

### CLAIM

- packet: `T3M-C2.2a`
- base: `c2a6e286497ab8a6899fb0f25bfc43848260afc4`
- claimed_at: `2026-07-17T22:40:50Z`
- scope: typed compact lifecycle sheet, action dismissal, swipe-item action
  grammar, reversible full-swipe policy, explicit destructive confirmation,
  renderer/accessibility tests, behavior contract, and Sol receipt
- exclusions: genuine gesture driver, hardware shortcuts, and host-applied
  focus (`T3M-C2.2b`); physical evidence (`T3M-F2`)

### CLAIM-RELEASE

- landed: `b4dc67bcb97ae04a3da3af2589dbe78c3852548d` on `main`
- verification: pre-push 223 mobile/RN-renderer tests, mobile typecheck,
  repository lint/format, and 19 Sol tests; post-commit 22 workspace-action/
  authoritative-Home/behavior tests and mobile typecheck; `git diff --check`
- authority note: swipe-item action IDs are accepted only after exact current
  active/archived membership and valid state/action checks; Delete never owns
  full swipe and remains explicit confirmed writeback
- close boundary: `T3M-C2.2a` compact native-lowered lifecycle sheet and typed
  row-action grammar are complete; genuine native gesture/keyboard/focus host
  behavior remains `T3M-C2.2b`, and full parity remains open through `T3M-F2`
- verified_at: `2026-07-17T22:43:45Z`

## Active packet — T3M-C2.2b

Outcome: close the remaining navigation host boundary with a real native pan
driver for typed row actions and a bounded hardware-key command adapter, while
keeping host-applied accessibility focus evidence honest.

Required behavior: horizontal pan must win only after an axis/threshold check,
dispatch at most the declared reversible full-swipe action, and always settle;
press actions remain available to switch/assistive users. Hardware commands are
closed and layout-aware (new task, navigation, detail, dismiss); unknown or
unmodified keys do nothing. Focus announcements/targets never select authority.

### CLAIM

- packet: `T3M-C2.2b`
- base: `89d63e4ab9977975ba0a6fdf369e0f279c22da09`
- claimed_at: `2026-07-17T22:44:52Z`
- scope: RN PanResponder full-swipe driver with accessible fallback, typed
  hardware-key parsing/dispatch, layout-aware dismissal/focus target, tests,
  behavior contract, and Sol receipt
- exclusions: physical keyboard/VoiceOver/TalkBack receipts (`T3M-F2`)

### CLAIM-RELEASE

- landed: `6eb235ebb85810903a17bd35dac11779ec695297` on `main`
- verification: pre-rebase 226 mobile/RN-renderer tests, mobile and RN
  typechecks, repository lint/format, and 19 Sol tests; post-rebase 26 native-
  input/workspace/behavior tests, mobile typecheck, and 19 Sol tests
- authority note: PanResponder dispatch is axis-, threshold-, side-, and
  declaration-gated; hardware keys map through one closed typed command set and
  layout/focus state only
- close boundary: `T3M-C2.2b` native gesture and keyboard host semantics are
  complete in code; physical keyboard and screen-reader receipts remain
  `T3M-F2`. Epic C is complete and ordered work advances to `T3M-D1`; full
  mobile parity remains open through `T3M-F2`
- verified_at: `2026-07-17T22:48:42Z`

## Active packet — T3M-D1.1

Outcome: establish the exact mobile paired-worktree tree/read contract and a
real Files workbench route with bounded source, Markdown, and image preview
states, without manufacturing repository data while transport is absent.

Required behavior:

- every tree/read request is bound to the selected session, repository,
  worktree, directory/path, revision/cursor, and a bounded limit;
- decoders reject foreign scope, traversal, duplicate/oversized pages,
  unsupported media, unsafe image URLs, stale revisions, malformed UTF-8, and
  content beyond the mobile preview limits;
- the Files route preserves the transcript and its anchor, exposes loading,
  empty, unavailable, failed, and stale states, and supports deterministic
  folder expansion, selection, refresh, path copy, and return-to-conversation;
- source uses the typed code/line presentation, Markdown uses the safe existing
  parser/renderer, and images use only an authenticated or short-lived HTTPS
  content URL carrying exact revision/digest identity;
- a missing provider is visible and cannot be counted as D1 completion.

Close rule: this closes only the mobile contract and route. `T3M-D1.2` must
connect and prove a real authenticated paired-environment provider before D1
closes; D2 and later epics remain open.

### CLAIM

- packet: `T3M-D1.1`
- base: `3297bcb0cd24785095c474d3c36abdf8a6e94951`
- claimed_at: `2026-07-17T22:50:53Z`
- scope: bounded tree/read schemas and decoder, Files route/state machine,
  source/Markdown/image previews, navigation/copy/refresh actions, tests,
  behavior contract, and Sol receipt
- exclusions: real paired provider (`T3M-D1.2`), changes/review (`T3M-D2`),
  Git/terminal/connections/release (`T3M-E1`–`T3M-F2`)

### CLAIM-RELEASE

- landed: `812ecf10a3ea3af37081ce2570e9981e902e0ed4` on `main`
- verification: pre-rebase 211 mobile tests, 36 behavior-contract tests,
  mobile typecheck, repository lint/format, and 19 Sol tests; post-rebase 60
  repository-files/behavior/Sol tests and mobile typecheck
- authority note: Files navigation binds every request and response to the
  exact session/repository/worktree/path/revision and request epoch; invalid,
  stale, foreign, unsafe, and oversized content fails closed, while opening and
  closing the route preserves transcript state and its scroll target
- close boundary: `T3M-D1.1` contract, state machine, and source/Markdown/image
  workbench are complete; the real authenticated paired-environment provider
  remains `T3M-D1.2`, so Epic D1 and full parity remain open
- verified_at: `2026-07-17T22:59:22Z`

## Active packet — T3M-D1.2

Outcome: replace the test-only repository port boundary with a native-host-
owned authenticated environment client for tree, read, and path-search
operations, without moving bearer custody into the Effect Native view tree.

Required behavior: only a server-verified native session may construct the
client; requests use exact HTTPS POST endpoints, bearer headers, omitted ambient
credentials, disabled redirects, and exact session/repository/worktree payloads.
Responses must be successful JSON within operation-specific byte bounds before
the D1.1 decoders see them. Invalid base URLs, malformed tokens, redirects,
non-JSON, failures, and oversized responses fail closed.

Close rule: this closes the D1 application/provider seam in code. Environment
registration, pairing/health UI, endpoint availability on a paired installation,
and installed physical-device proof remain `T3M-F1`/`T3M-F2`; D1 does not claim
those release proofs early.

### CLAIM

- packet: `T3M-D1.2`
- base: `13df87b53361d0f4dfaf8640b246a1fb92852142`
- claimed_at: `2026-07-17T23:03:55Z`
- scope: authenticated mobile repository environment client, verified-session
  host construction, Files and `@` search provider wiring, tests, behavior
  contract, and Sol receipt
- exclusions: environment pairing/health and endpoint provisioning (`T3M-F1`),
  installed iOS/Android/live paired proof (`T3M-F2`), changes/review (`T3M-D2`)

### CLAIM-RELEASE

- landed: `7ef1cfe155bd84db19791ea9768485c8b178c432` on `main`
- verification: pre-rebase 213 mobile tests, 36 behavior-contract tests,
  mobile typecheck, repository lint/format, and 19 Sol tests; post-rebase 26
  environment/files/Sol tests and mobile typecheck
- authority note: the verified native session host alone constructs the
  authenticated client; bearer material stays in headers, redirects and ambient
  credentials are disabled, and response bytes are bounded before exact D1.1
  decoding
- close boundary: `T3M-D1` is complete in application code. Environment
  registration/pairing/health and installed live endpoint evidence remain
  explicitly `T3M-F1`/`T3M-F2`; ordered work advances to `T3M-D2`
- verified_at: `2026-07-17T23:08:02Z`

## Active packet — T3M-D2

Outcome: add a transcript-preserving Changes route that projects the exact
selected worktree status, renders bounded native diffs, and records a review
instruction against one current diff row through authoritative writeback.

Required behavior: status, diff, and review traffic is bound to the exact
session/repository/worktree/status/path/source/revision/row identity and request
epoch. Bounded decoders reject foreign, duplicate, traversal, stale, malformed,
binary, unmerged, unsupported, and oversized data. Only a current selectable
row may open the instruction editor; only a nonempty bounded instruction may be
submitted; and success appears only from an exact recorded receipt. Opening,
closing, refreshing, and hardware dismissal preserve the conversation state and
invalidate stale workbench requests.

Close rule: this closes the D2 mobile contract, Changes state machine, native
diff/comment affordance, and authenticated provider seam in application code.
Live paired endpoint provisioning and installed iOS/Android writeback evidence
remain `T3M-F1`/`T3M-F2`; Git, terminal, connections, and release remain open.

### CLAIM

- packet: `T3M-D2`
- base: `7dbc81dc1761e4a1d225fa69fbcb0c62111681b9`
- claimed_at: `2026-07-17T23:14:19Z`
- scope: exact changed-files/diff/review contracts, Changes route and state
  machine, native row-comment selection, authenticated writeback, visible
  receipts, navigation fencing, tests, behavior contract, and Sol receipt
- exclusions: live paired endpoint and installed-device evidence (`T3M-F1/F2`),
  Git/terminal/connections/release (`T3M-E1`–`T3M-F2`)

### CLAIM-RELEASE

- landed: `dadc663647aaab6f88a39794353e50af6dac2f19` on `main`
- verification: pre-rebase 243 mobile tests, 21 RN renderer tests, mobile and
  RN typechecks, 36 behavior-contract tests, repository lint/format, and 19 Sol
  tests; post-rebase 45 review/environment/RN/Sol tests, mobile typecheck, and
  the 141-document Sol manifest check
- authority note: every status, diff, and review operation is exact-scope,
  bounded, revision- and epoch-fenced; native row selection mints no authority,
  and only an exact recorded receipt becomes visible. Late receipts after route
  dismissal are ignored while transcript state remains unchanged
- close boundary: `T3M-D2` is complete in application code. Live paired
  endpoint provisioning and installed writeback remain explicit `T3M-F1/F2`
  evidence; ordered work advances to `T3M-E1`
- verified_at: `2026-07-17T23:17:18Z`

## Active packet — T3M-E1

Outcome: add a transcript-preserving Git workbench whose status, branch,
commit, and push operations remain bound to the exact selected worktree and
become successful only through authoritative receipts.

Required behavior: status responses are bounded and carry exact
session/repository/worktree/status/HEAD identity, changed files, current branch,
upstream, ahead/behind counts, default-branch state, and a bounded branch list.
Checkout, commit, and push use a closed typed operation set, validate selection
against the current snapshot, require explicit confirmation carrying exact
status and HEAD fences, and include idempotency identity. Stale status, dirty
tree, conflict, non-fast-forward, auth, hook, detached, missing-upstream, and
generic failures are typed, visible, and cannot mint success. Only a decoded
exact receipt may replace status or display success; route dismissal invalidates
late responses and preserves the conversation state.

Close rule: this closes the E1 mobile contract, Git route/state machine,
authenticated environment seam, confirmation flow, failures, and receipts in
application code. Live endpoint provisioning and installed iOS/Android Git
mutation evidence remain `T3M-F1`/`T3M-F2`; terminal and native finish remain
open.

### CLAIM

- packet: `T3M-E1`
- base: `13393b62306325dd1576c31ea8e29731461f6ab6`
- claimed_at: `2026-07-17T23:23:26Z`
- scope: exact-worktree Git status/branch/file contracts, Git workbench route,
  confirmation-fenced checkout/commit/push, typed conflicts/failures,
  authenticated provider seam, receipts, tests, behavior contract, and Sol
  receipt
- exclusions: live paired endpoint and installed mutation evidence
  (`T3M-F1/F2`), terminal/connections/release (`T3M-E2`–`T3M-F2`)

### CLAIM-RELEASE

- landed: `ea8c6e2d5e16f9da3ad69fe13fc808c4bdff4fe5` on `main`
- verification: pre-rebase 246 mobile tests, 36 behavior-contract tests,
  mobile typecheck, repository lint/format, and 19 Sol tests; post-rebase 24
  Git/environment/Sol tests, mobile typecheck, and the 141-document Sol
  manifest check
- authority note: status and all mutations are exact session/repository/
  worktree/status/HEAD fenced. Checkout, commit, and push each require explicit
  confirmation and idempotency identity; typed failures mint no receipt, and
  only decoded receipts replace visible status while the transcript remains
  unchanged
- close boundary: `T3M-E1` is complete in application code. Live paired Git
  endpoint provisioning and installed mutation evidence remain explicit
  `T3M-F1/F2`; ordered work advances to `T3M-E2`
- verified_at: `2026-07-17T23:24:58Z`

## Active packet — T3M-E2

Outcome: add a transcript-preserving terminal workbench using Effect Native's
typed terminal host, exact paired-worktree session contracts, bounded replay,
and foreground recovery without granting mobile shell/cwd/process authority.

Required behavior: snapshot, create, replay, and command operations bind to the
exact session/repository/worktree and host-minted terminal/version. The host
alone chooses shell, cwd, and environment. Mobile may send only bounded stdin,
bounded geometry, interrupt, restart, and close operations; each requires an
exact receipt. Replay is contiguous, monotonically sequenced, version-fenced,
event- and byte-bounded, and names gaps/truncation instead of inventing output.
The native host emits only typed data/resize events, provides a bounded
scrollback and keyboard accessory, and background-to-active lifecycle refreshes
only when Terminal is the open workbench.

Close rule: this closes the E2 mobile terminal contract, registered React Native
host driver, session route/state machine, reconnect/replay, keyboard accessory,
geometry, bounded history, background recovery hook, authenticated environment
seam, and receipts in application code. Live PTY endpoint provisioning,
packaged native-emulator evidence, and physical iOS/Android background proof
remain `T3M-F1`/`T3M-F2`.

### CLAIM

- packet: `T3M-E2`
- base: `06eef70690869769afcbfbdf446c6a408c13f607`
- claimed_at: `2026-07-17T23:33:29Z`
- scope: exact terminal snapshot/create/replay/command contracts, native Effect
  Native terminal driver, session route and keyboard accessory, contiguous
  replay/gap accounting, geometry negotiation, foreground recovery, receipts,
  tests, behavior contract, and Sol receipt
- exclusions: live paired PTY endpoint (`T3M-F1`), packaged native emulator and
  physical-device background/distribution evidence (`T3M-F2`)

### CLAIM-RELEASE

- landed: `b20873eab90c3dd2d4f19b79edf46cad18e5001c` on `main`
- verification: pre-rebase 249 mobile tests, 36 behavior-contract tests,
  mobile typecheck, repository lint/format, and 19 Sol tests; post-rebase 24
  terminal/environment/Sol tests, mobile typecheck, and the 141-document Sol
  manifest check
- authority note: the paired host alone chooses shell/cwd/environment; mobile
  can send only bounded typed terminal operations against exact host-minted
  terminal/version identity. Output replay is contiguous and bounded, gaps are
  explicit, command success requires a receipt, and foreground recovery only
  refreshes an already-open Terminal route
- close boundary: `T3M-E2` is complete in application code. Live paired PTY
  endpoint provisioning, packaged native-emulator evidence, and physical
  background journeys remain explicit `T3M-F1/F2`; ordered work advances to
  `T3M-F1`
- verified_at: `2026-07-17T23:34:56Z`

## Active packet — T3M-F1

Outcome: make mobile connections and native settings coherent and usable while
keeping credential, permission, registration, and transport authority in the
native host and keeping the transcript workbench primary.

Required behavior: Settings is a first-class typed route with account,
environments, notifications, appearance, accessibility, storage/cache,
diagnostics, legal, and pending-share destinations. Environment directory,
pair, and reconnect traffic uses the verified-session authenticated client,
bounded public-safe projections, explicit health/capabilities, idempotency
identity, and exact receipts. Notification permission is requested only after
an explicit tap; native token material never enters view state; preferences are
persisted in native secure storage. Initial and live share links accept only
bounded text and safe HTTP(S) URLs, require review, and insert into the composer
without auto-submit or transcript mutation. Existing controller, environment,
notification, Git, terminal, and agent views remain the inspectors for their
respective authorities.

Close rule: this closes the F1 application contract and host composition for
settings, connection pairing/health, notification education/preferences/
registration health, share intake, and inspectors. It does not claim that the
currently fixed authenticated OpenAgents endpoint is provisioned with a live
owner environment, that a native device token is available on a simulator, or
that physical/signed builds passed. Those installed runtime and distribution
receipts remain `T3M-F2`.

### CLAIM

- packet: `T3M-F1`
- base: `50486fabbdc2e9d6d5fb30a5c824ecebe1674ddc`
- claimed_at: `2026-07-18T00:42:00Z`
- scope: first-class settings hierarchy, authenticated environment directory/
  pair/reconnect client and receipts, notification education/preferences/
  registration-health host, safe initial/live share intake, public-safe
  inspectors, tests, behavior contract, and Sol receipt
- exclusions: live owner-environment endpoint observation, physical iOS and
  Android journeys, packaged native emulator, signed distribution, and owner
  acceptance (`T3M-F2`)

### CLAIM-STATUS

- implementation: Settings is a transcript-preserving workbench route; account
  state and actions reuse the existing session authority; connection health,
  pairing, and reconnect are bounded and receipt-driven over exact authenticated
  endpoints; permission prompting is explicit-only; notification preferences
  stay in native secure storage; native registration material never crosses
  into Effect Native; and safe shares require review before composer insertion
- focused proof: 254 mobile tests passed, including the new settings,
  environment transport, permission, preference, share-delivery, and transcript
  preservation journey; 36 behavior-contract tests and mobile typecheck passed
- repository gate: `pnpm run check` passed
- residual: `T3M-F2` complete component census, layout/motion/haptic and
  screen-reader finish, installed iOS/Android journeys, signed distribution,
  and owner acceptance; this is not a full-parity claim
