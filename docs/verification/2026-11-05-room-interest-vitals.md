# Actual interest selection gauges, first part of #12840

Source: `ac5a09eac2` plus this room-stats patch. The existing relevance gauge
already used the production interest picker; this part exposes its four
categories rather than introducing another estimate. There is no change to
selection policy, wire format, API storage or the existing seat report shape.

Verified locally, at nice 15 with one Coax check/build worker:

- Full room suite: **545/545 pass**. New checks cover exact categories,
  process sums/maxima, the named worst seat, excluding inactive/unplaced seats,
  short output buffers, and counts fed by the production replication path.
- Native build/startup defaults pass. A C clock forward declaration fixes
  the declaration-order error caught by the first native compile. The passing
  room suite and release worker were not repeated for that SDK-only fix.
- Release worker: **1,004,062 / 1,060,000 bytes**, no budget increase.
- Native two-consumer transport run: room/proxy ports **8953/8954**, 20 s,
  profile **0/0/0**, driven by the existing Coax room-proof/load tools.
  The native vitals endpoint reports **2 seats**, **53 whole picks** total,
  **27** at the largest seat, zero health-only/blocked/hidden picks in this
  ordinary workload, and identifies `pair-load-51024-1`, seat 0, as the worst.
  The sampled aggregation call takes **1 microsecond**. This one small sample
  is not a bound on whole instrumentation overhead or maximum-load cost.

Rerun after building the native room:

```
coax/web/tool.sh coax/web/room/room-proof.coax transport /path/to/room 8953 2 20 0/0/0
```

During the run, `/api/room/vitals` on port 8953 exposes the `interest` lines.
They are gauges of each live placed seat's most recent pick. Totals and maxima
are process-wide; the named worst room/seat maximizes whole plus health-only,
with first seat winning ties. Picks do not prove rows sent or received.

Remaining for #12840: complete sent/deferred row and message-byte accounting,
accepted revision/age and owner/retry/drain reporting, injected fault coverage,
client presentation and clock uncertainty, dashboard integration and thresholds,
and representative instrumentation overhead. No production, visible browser
or deployment script is used. The existing tools-check missing-header finding
from the preceding two records remains outside this part.
