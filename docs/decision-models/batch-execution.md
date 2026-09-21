# Bounded batch execution for classify

`POST /v1/classify` answers a multi-input request by forwarding one
native `POST /v1/systemone` call per input through the same backend the
door is bound to. This document is the scheduling contract for that
fan-out — what bounds it, what a queued or stopped item reports, and
what the fixture measurement showed. It is the gateway's half of #9483.

This is HTTP scheduling, not packed inference: every forward is an
ordinary native call with that input's own questions, and nothing here
claims a model-quality or production-throughput result.

## The bound layers

An in-flight forward holds three slots, and the tightest one wins:

- The call's fan-out bound — the door's configured
  `classify_item_concurrency`, clamped by the binding's declared
  `capacity.concurrency`, the process's `max_in_flight`, and the
  request's own input count.
- The binding's `capacity.concurrency` pool, shared with every other
  call on the door — `systemone` calls and other tenants' classify
  calls included.
- The process's `max_in_flight` forward bound.

Because each layer is a real permit a forward holds for its duration, a
configured item concurrency can never multiply backend capacity
invisibly: a declaration above the binding's concurrency simply queues,
and the pool — not the config — decides how many forwards run.

`classify_item_concurrency` defaults to 1, which keeps the call serial.
A declaration is refused at config load when it is zero, when it sits on
a door that declares no `classify` bounds at all, or when it exceeds the
process's own `max_in_flight` — a bound that can never be reached is a
misconfiguration, not a cap.

## Queueing, deadlines, and halts

Every slot wait and forward uses the same absolute deadline. An item that
cannot take a slot waits — bounded by the call's execution
deadline, which is the configured `forward_timeout_ms` measured from
when the call was admitted. The queue is the call's own pending items,
so its depth never exceeds the request's input count. Global and per-tenant
admission limits additionally bound the sum of admitted input counts. Saturation is
typed: an item still waiting when the deadline passes reports
`unattempted` with a cause naming the bound that stopped it (a full fan
out, the door's slots, or the gateway's slots), and an item dispatched
but cut off by the deadline reports `unavailable`. Neither is charged
for work it did not dispatch, and neither reads as success.

A forward that comes back `unavailable` — a transport failure, a 5xx, a
deadline — halts the call: the fan-out pool closes, items already in
flight finish and report what they got, and items still queued report
`unattempted`. Backend death cannot turn potentially attempted work
into zero usage or a pass, and a call dropped mid-flight leaves its
reservation unsettled — recovery orphans it as `unknown`, which is
charged rather than freed.

Results reassemble in input order by index, so reordered completions
never reorder the response. Settlement counts the forwards that were
dispatched — `questions` and `options` scale by dispatched inputs — and
per-item usage is retained where the door reported it, with aggregate
counters flagged incomplete rather than zero-filled.

## The measured fixture

`classify_serial_and_concurrent_batches_are_measured` runs the same
eight-input single-label call twice against a deterministic stub that
holds each forward 60 ms — once at the default bound, once at a
configured bound of four on a door whose binding declares concurrency
two. A run reports completed items, batch elapsed, per-item latencies,
incomplete coverage, and the stub's observed peak in flight:

```json
{"serial":     {"completed_items": 8, "batch_ms": 504, "peak_in_flight": 1,
                "per_item_ms": [62, 61, 63, 63, 62, 63, 60, 61], "incomplete": 0},
 "concurrent": {"completed_items": 8, "batch_ms": 253, "peak_in_flight": 2,
                "per_item_ms": [61, 61, 62, 62, 61, 61, 62, 62], "incomplete": 0}}
```

Both runs completed every item in input order. The concurrent run's
peak was two, not the configured four — the binding's declared
concurrency clamped it, which is the coverage invariant working. The
figures are a local fixture's record of what the scheduler did, not a
latency promise for a real backend.

## Limits and unmet acceptance

- Item concurrency is HTTP-level scheduling; the door still answers one
  `systemone` call per input. No packing, no cross-input sharing, no
  model-side batching.
- Admission reserves the entire request's input count against
  `max_classify_inputs` and `max_classify_inputs_per_tenant` before quota
  reservation or task creation. Both default to 1,024. Waiting and running
  items retain these permits until the call ends. Requests that cannot fit
  receive `classification_queue_full` with HTTP 429 and `Retry-After: 1`.
  Different keys for one tenant share the same allowance; anonymous calls
  share a separate allowance. These are admission ceilings, not weighted
  fairness, priority scheduling, token/memory estimates, or dedicated allocations.
- A call's deadline is `forward_timeout_ms`, shared with plain
  forwards; there is no separate classification deadline yet.
- Receipts still record the attempt, not each item — per-item receipt
  identities remain integration work.
- Cancellation of the HTTP call relies on the ledger's orphan path; the
  gateway does not yet observe a disconnect to stop in-flight items
  early.
- The durable-job surface stays with #9484, and semantic review of
  flagged inputs stays with the review items the facade documents.
