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
