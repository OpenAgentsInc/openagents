# Room overload, first part of #12841

An admitted seat has two outstanding cast reservations, covering waiting,
bar and flight phases together. The 32-slot room covers all 16 seats. A
third cast gets the existing `r SEQ busy` before it enters the queue; the
refusal spends neither cast order nor cooldown. A completed or stopped cast
frees the reservation. Idle reservations are not borrowed. This intentionally
bounds one client's queued work even while other clients are idle.

`cast-budget.coax` is the authority; `room-casts.coax rc_hear` counts the
seat's active casts and enforces it. The queue remains local to each room.
Existing input time/bank limits remain in force; sending more inputs cannot
earn more movement. Waiting casts retain the existing half-second maximum
wait for preceding inputs; ordinary cast bars and flight times retain the
spell's rules.

| Work | Current overload outcome | Qualification still needed |
| --- | --- | --- |
| New player cast | Two outstanding per seat; excess refused as busy | Native latency regression and workload capacity |
| Already admitted cast | Retains its slot through bar/flight | Every downstream world-strike/outcome cap |
| World hurt | Existing 32-entry deferred queue | Full-queue ordering: current fallback applies immediately, before heals |
| Lifecycle/status | Existing reserved budgets and catch-up snapshots; native socket throttling verified in #12903 | Actual browser and integrated saturation |
| Disposable cues | Existing per-seat allowance, drops counted | Saturation across gameplay workloads |
| Authoritative outcomes | Existing reservation and loss counts; town resnapshot after missed outcomes | Exhaustion with active fights and destruction |
| Durable operations | Existing receipt outboxes; checkpoint-v1 receiver is inactive | Explicit full-queue refusal/expiry and dependency outage policies |

Two deterministic room checks cover a sender flooding before a tick, all 16
seats taking both reservations, refusal without cooldown/order changes,
released-slot reuse, and admission in another room while the first is full.
They prove queue separation, not a bound on the other room's CPU tick time.

This does not close #12841. Remaining: hurt and world-strike semantics at
capacity; durable/control retry admission and queue ages; room/seat CPU work
budgets and early seat refusal; storage/API/model outage behavior; measured
new-room/content-warm cost; simultaneous area/pet/destruction stress and a
slow consumer beside healthy rooms. Existing event/freshness work from
#12836/#12837 remains its own evidence, not a substitute for these gates.

Already landed: `d7404bff3d` (#12902) recovers missed town outcomes;
`0d0504aca9` (#12903) measures the native socket/proxy path at 64 kbit/s
beside a healthy page and eight bots, recovering within 67 ms after release.
Reuse that unchanged evidence. Those consumers were headless socket mirrors;
the actual browser/proxy path remains unqualified. The later #12903 comment
corrects the negative test claim: only the HEARD-lag rule was disabled and
observed failing, not the held-snapshot rule.
