# CUT-10 live-event convergence receipt

- Date: 2026-07-11
- Issue: [#8690](https://github.com/OpenAgentsInc/openagents/issues/8690)
- Status: shared/mobile and complete Desktop host/wire/renderer no-poll path
  landed; physical receipt pending
- Sequencing exception: the owner deferred CUT-09 physical acceptance while
  the paired phone records video, without waiving that acceptance gate

## Landed contract

The Khala Sync client now exposes direct content-change callbacks alongside its
Effect Stream and projects one cursor-aware native conversation subscription:

- every envelope carries an exact subscription ref, positive generation,
  ordered subscription-local sequence, thread ref, durable cursor, and bounded
  run/message/event correlation refs;
- delivery is explicitly `provisional`, `confirmed`, or `interrupted`;
- resume from a retained cursor is distinguished from a proven gap repaired by
  one bounded authoritative snapshot;
- a resume cursor ahead of durable state interrupts fail-closed;
- while a listener is slow, arbitrary source churn coalesces to one newest
  pending snapshot instead of growing an event queue;
- source signals, delivered updates, coalesced signals, the maximum pending
  snapshot count, and latest delivery latency are directly observable without
  transcript or payload content;
- close removes state/content observers and unsubscribes the owned thread scope
  once.

Mobile create, append, runtime completion, expiry, and interrupt reconciliation
now use that subscription. The production mobile conversation adapter contains
no `for`-attempt polling loop, `await sleep(100)`, or conversation-state
`setInterval`; a one-shot bounded deadline preserves honest pending outcomes.

A Desktop host registry now composes this contract through Runtime Gateway v7.
It serializes registry mutations, caps the host
at 64 live subscriptions, closes the prior generation before replacement,
fences stale subscribe/unsubscribe attempts, exposes metrics only for the exact
active generation, and disposes every slot once. The closed Gateway schema now
accepts typed subscribe/resume/unsubscribe commands and outcomes and decodes
the full bounded live update on its existing event channel. Main resets all
subscriptions before authenticated Sync replacement/sign-out; Gateway disposal
closes the registry.

The renderer-side typed adapter registers the decoded event listener before the
subscribe request so an immediate authoritative snapshot cannot race past,
buffers only the latest pre-ack update, fences foreign refs/generations, stale
sequences, and cursor regression, and sends one exact unsubscribe on disposal.
The active runtime chat consumer now opens that adapter before append, waits
for the exact generated message ref, keeps the same subscription through
streamed timeline updates and exact terminal run confirmation, then closes it
in `finally`. Harness selection still maps Fable to `claude_pylon` and Codex to
`codex_app_server`. Initial catalog/detail reads and final timeout diagnosis are
one-shot queries; the old recurring 100 ms loops are gone.

## Verification

- `@openagentsinc/khala-sync-client`: 169 pass, 3 explicitly gated live-smoke
  skips, 0 fail, 12,717 expectations; import coverage reports 22 source modules,
  21 test files, and no uncovered production module; typecheck passes.
- OpenAgents mobile: 66 pass, 0 fail, 287 expectations; typecheck passes.
- Focused live-subscription tests prove ordered delivery, resume, gap refetch,
  slow-consumer coalescing, future-cursor refusal, and idempotent disposal.
- Focused mobile tests prove asynchronous exact-message confirmation arrives
  through the change subscription and enforce the no-poll source boundary.
- Desktop registry: 6 focused tests / 26 expectations and Desktop typecheck
  pass for generation replacement, stale fencing, capacity, metrics, failed
  open, and dispose-all behavior.
- Renderer adapter: 4 focused tests / 17 expectations cover initial-event
  races, stale fencing, exact idempotent close, and unavailable cleanup.
- Runtime consumer: 11 focused tests / 35 expectations cover listener-before-
  append delivery, exact-message confirmation, streamed terminal projection,
  exact unsubscribe, preserved harness lanes, expired commands, and a source
  oracle forbidding `pollAttempts`, `sleep(100)`, and `setInterval`.
- Full Desktop verification passes on the integrated #8712 tree: 311 tests /
  1,581 expectations, typecheck, production bundle, every built Electron smoke
  stage (including Fable stream/markdown/metadata), renderer reload
  restoration, and lifecycle teardown with zero active host slots.

## Coordination boundary and residual

Issue #8712 explicitly released the Gateway wire after its optional
`conversation.start.lane` field landed, then its chat-UI landing satisfied the
stated renderer-consumer handoff condition. CUT-10 preserved that field,
protocol v7, and all landed chat UI. The later local Fable/Codex child lane
remains separate. This tranche did not change local execution, child activity,
usage, composer, provider-account, Pylon, package, or lockfile paths.

CUT-10 must still attach the physical-mobile continuation receipt when the
recording phone is available. Until then #8690 remains open; the deterministic
no-poll code and built-Electron receipt are complete.
