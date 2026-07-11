# CUT-10 live-event convergence receipt

- Date: 2026-07-11
- Issue: [#8690](https://github.com/OpenAgentsInc/openagents/issues/8690)
- Status: shared/mobile tranche complete; Desktop integration and physical
  receipt pending
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
- close removes state/content observers and unsubscribes the owned thread scope
  once.

Mobile create, append, runtime completion, expiry, and interrupt reconciliation
now use that subscription. The production mobile conversation adapter contains
no `for`-attempt polling loop, `await sleep(100)`, or conversation-state
`setInterval`; a one-shot bounded deadline preserves honest pending outcomes.

## Verification

- `@openagentsinc/khala-sync-client`: 169 pass, 3 explicitly gated live-smoke
  skips, 0 fail, 12,715 expectations; import coverage reports 22 source modules,
  21 test files, and no uncovered production module; typecheck passes.
- OpenAgents mobile: 66 pass, 0 fail, 287 expectations; typecheck passes.
- Focused live-subscription tests prove ordered delivery, resume, gap refetch,
  slow-consumer coalescing, future-cursor refusal, and idempotent disposal.
- Focused mobile tests prove asynchronous exact-message confirmation arrives
  through the change subscription and enforce the no-poll source boundary.

## Collision boundary and residual

Issue #8712 has an active claim over Desktop `runtime-gateway-contract.ts`,
`runtime-gateway.ts`, `main.ts`, `preload.cts`, renderer
`runtime-conversation.ts`, and adjacent composer/settings paths for its Episode
250 harness-selection slice. CUT-10 did not edit those paths or duplicate its
optional `conversation.start` lane field.

After #8712 releases or lands, CUT-10 must rebase and wire this shared envelope
through the Desktop Runtime Gateway subscribe/resume/unsubscribe protocol,
remove Desktop's two 100 ms confirmed-timeline loops, run transport and built
Electron tests, and attach the physical-mobile continuation receipt when the
phone is available. Until then #8690 remains open.
