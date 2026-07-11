# P0 PROOF D1-G: predictable Codex trace workspace acceptance

- Issue: #8675
- Parents: #8574, #8566
- Priority: immediate; complete before the next recorded Desktop trace demo
- Depends on: closed #8674 at `c83f5faac9`
- Product context: [`../../transcripts/248.md`](../../transcripts/248.md)
- Architecture evidence:
  [`../../teardowns/2026-07-10-codex-subagents-rendering-analysis.md`](../../teardowns/2026-07-10-codex-subagents-rendering-analysis.md)

## Outcome

The loss-accounted Codex history workspace that landed in #8674 is visibly
predictable in the real Electron app on current `main`, not only green against
fixtures. An owner can open Desktop, immediately find recent named top-level
Codex conversations, select a nested parent/child trace, inspect agents and
tools, close/reopen, and record the flow without blank startup, stuck loading,
misleading titles, hidden descendants, layout breakage, or private-content
leakage.

This is a bounded product-acceptance and defect-hardening issue. It does not
rebuild #8674, add provider execution, or widen renderer authority.

## UX promise

Contract: `openagents_desktop.seam.codex_loss_accounted_history.v2`.

When local Codex history exists, opening OpenAgents Desktop:

1. paints the shell and metadata-first conversation list without waiting for
   selected-thread hydration;
2. shows stable real conversation names and compact relative timestamps,
   newest activity first, with no permanent loading copy or decorative row
   clutter;
3. keeps child and grandchild sessions attached to the selected top-level
   conversation instead of listing them as unrelated chats;
4. shows every supported trace item once or reports an explicit redaction/gap;
5. makes the agent tree, selected transcript, tool/item inspector, paging,
   resizing, collapse/drawer, and keyboard navigation visibly usable; and
6. restores bounded ref-only selection state after restart without blocking
   first paint or uploading local history.

## Scope

- Launch the actual `oa`/Electron path from clean current `main` against the
  owner-local history root; do not substitute the synthetic corpus for the
  visible acceptance pass.
- Exercise one recent named top-level conversation with at least two sibling
  children, one nested grandchild, and tool activity. Publish only counts,
  hashes, timings, and redacted screenshots/video metadata.
- Add or tighten one executable UX/behavior contract and a real-Electron
  journey for the promise above. The programmatic oracle must fail on blank
  shell, stale spinner/loading text, untitled fallback when a title is known,
  child leakage into the top-level list, selection loss, silent gaps, or
  inspector inaccessibility.
- Fix only defects exposed by that journey. Prefer the smallest correction in
  the existing Runtime Gateway v4 / Effect Native workspace; do not introduce
  a demo-only state path.
- Record first-shell, catalog, first-page, and selection/inspector readiness
  timings so future releases can detect regression without promising a
  hardware-independent absolute latency.

## Acceptance

1. Real Electron opens from current `main`; the shell is interactive before
   history page hydration and no beachball/blank-window/stuck-loading defect is
   observed.
2. Known titles, relative timestamps, ordering, descendant counts, and selected
   trace remain stable through refresh and one restart.
3. The selected real nested trace exposes parent, both sibling children, the
   grandchild, typed tool rows/details, lifecycle, and completeness state with
   zero unsupported gaps for the pinned source version.
4. Keyboard-only traversal covers the conversation list, agent tree,
   transcript items, inspector switch/back, paging, collapse, and narrow
   drawer affordance. Status is not color-only.
5. The automated contract runs against built Electron and fails on the named
   regressions. Existing Desktop `verify`, package build, and focused Effect
   Native/behavior-contract tests pass.
6. The acceptance receipt contains no conversation text, local paths, provider
   credentials, raw JSONL, or stable private refs.

## Non-goals

- launching or steering a provider runtime;
- uploading local Codex history to Khala Sync;
- pixel-cloning the closed Codex desktop app;
- weakening pagination, redaction, completeness, preload, or renderer laws;
- general visual polish unrelated to the recorded trace flow.

## Close

Close after the automated UX promise and one owner-visible real-Electron trace
receipt both pass. Any remaining provider-stream or cross-device work belongs
to the next D1 vertical slice, not this proof.

## Acceptance receipt — 2026-07-10

`bun run --cwd apps/openagents-desktop verify` passed on the owner Mac against
the real local Codex history root. The built Electron journey observed 1,231
top-level conversations and selected a 131-agent trace containing 127 direct
children, a nested grandchild, 72 tool items, and zero unsupported gaps. It
proved the catalog, nested workspace, keyboard binding, structured tool
inspector, and a real renderer reload.

Public-safe readiness measurements from that run were: shell 0 ms after the
journey began (already painted before catalog inspection), catalog 2,111 ms,
selected page 0 ms (already projected after the explicit row selection), tool
inspector 147 ms, and reload restoration 3,295 ms on the first pre-fix run.
The final gate records fresh values on every verification rather than treating
these machine-local observations as hardware-independent budgets.

The journey exposed and fixed one real state defect: conversation, agent,
item, paging, and inspector actions could overwrite the ref-only restoration
record while dropping expanded branches or forcing the inspector open. The
handlers now preserve those fields, while explicit top-level selection expands
the complete selected topology. The receipt contains no titles, message text,
paths, credentials, raw records, or stable private refs.

### Post-acceptance viewport and selection hardening

Owner review of the real app exposed four presentation defects after close:
an unnecessary top/left inset, horizontal pane overflow, missing scroll
ownership, and nearly one-second selection latency on the 29,262-item trace.
The follow-up removes the outer inset, constrains every split descendant,
assigns independent virtualized scroll owners to the 1,231-row sidebar and
timeline, and visibly marks the pending/active sidebar row. Reusing the
worker's metadata graph and projecting only the requested page reduced the
same trace's direct cold page time from about 1,406 ms to 182 ms; built
Electron measured 96–97 ms warm without changing the complete
`source = rendered + redactions + gaps` accounting.

A second owner review proved that the apparent center scroll region still had
`clientHeight = 0` while 3,061px of rows overflowed it. The defect was the
shared DOM SplitPane wrapper: it was not a flex formatting context, so its
child's `flex: 1` had no definite viewport. The shared renderer now gives pane
content a bounded flex viewport; the real-Electron oracle rejects a center
viewport below 100px and proves `scrollTop` changes. The accepted run measured
546px client height over 3,061px scroll height. Trace pages now render 50
compact, variable-height previews (full bounded detail remains in the
inspector), and the same nested selection committed in 89–91ms.
