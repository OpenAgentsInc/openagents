---
id: method.as-of-event-replay
version: 1
kind: method
title: Replay event histories with explicit ordering and cutoffs
summary: >-
  For stateful evaluators, replay events using the contract’s ordering and
  as-of rules instead of deriving final state from the latest row or current
  scorer output. Test cutoff boundaries, ties, and reversible transitions
  independently.
tags: [event-replay, timestamps, state-machines, as-of]
applies_when: >-
  Code folds timestamped events into decisions, locks, overrides, or other
  state that must reflect a specified point in time.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - risk-scorer-replay-1790394263
  cites:
    - Martin Fowler, “Event Sourcing,” “How It Works.”
evidence: []
---

## Details

Treat replay as a fold over an explicitly ordered event history. Use the timestamp field and tie-break rule specified by the contract; do not substitute ingestion time, file order, or display order without evidence. Apply the contract’s as-of cutoff consistently, including its exact-boundary behavior. A transition such as a freeze or reopen should be handled according to the state machine, not assumed to be terminal or reversible.

Keep event replay separate from request scoring: a replayed event may determine final state even when the current scorer would produce a different base result. Fowler describes event sourcing as deriving application state from an event sequence; the allowed transitions and ordering policy still come from the application contract. (Martin Fowler, “Event Sourcing,” “How It Works.”)

## How to check

Create a small synthetic history and verify: an event before the cutoff applies; one after it does not; an event exactly at the cutoff follows the documented rule; same-time events follow the specified tie-break even if input order is reversed; and a later transition behaves as documented.

```python
def replay(events, cutoff, apply, initial):
    ordered = sorted(events, key=lambda e: (e["event_time"], e["tie_break"]))
    state = initial
    for event in ordered:
        if event["event_time"] <= cutoff:  # Change only if the contract specifies otherwise.
            state = apply(state, event)
    return state
```
