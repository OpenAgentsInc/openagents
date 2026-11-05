# Cast reservations, first part of #12841

Source: `cast-budget.coax`, `room-casts.coax`; contract and remaining scope:
[`room-overload.md`](../verse/2026-11-05-room-overload.md). This qualifies
per-seat cast queue reservations, not complete room overload isolation.

Local isolated worktree, builds at nice 15, one Coax check/build worker.
The measured native/worker artifacts use `2f88ce4f12` plus the cast reservation
patch. Rebase subsequently includes `2f95eb8979` (rig tables as literals);
these measurements do not claim to rebuild that later asset-source change.
The room/cast sources and their 543 passing checks are unchanged by it.

- Full room suite: **543/543 passed**, including a flood before a tick,
  all 16 seats taking two reservations, explicit busy refusal without spending
  order/cooldown, slot reuse and another room admitting a cast beside a full
  room. The queue-separation test does not measure CPU isolation.
- Authority: `coax/bin/coax mutate coax/web/room/cast-budget.coax --library
  --root-only --jobs 1`: **5 mutants, 5 killed, zero survivors**.
- Native `coax/web/room/build.sh /tmp/mmo-codex-qual/12841/room` completes
  within 15 minutes; startup check reports **zero fields off their defaults**.
- Release-mode `coax/web/tool.sh coax/web/room/worker.coax
  /tmp/mmo-codex-qual/12841/everglade-room.wasm`: **1,004,003 bytes**, below
  the committed **1,060,000-byte** budget. No budget was raised.
- API sources are unchanged from the checkpoint receiver qualification;
  that evidence is not repeated here. `tools-check` still reports the same
  five pre-existing missing Python headers documented in that record.
- Native proxy cast regression: **10/10 answered** (8 taken, 2 refused),
  slowest **799 ms**, none beyond the 2000 ms bound; two expected gone-target
  interruptions and none for another reason; 1720 state frames heard;
  **`casts hold`**, exit zero. Four socket consumers, two rounds, 250 ms
  round trip, 30 ms jitter and 2% loss/retry.

The cast regression command uses its own ports, local development room/key,
headless socket consumers and deterministic network proxy:

```
CAST_PROOF_LOG=/tmp/cast-room.log coax/web/tool.sh coax/web/room/room-proof.coax cast /tmp/mmo-codex-qual/12841/room 8951 250 30 20 2
```

No production resources or visible browsers are used. Existing missed-town
recovery (`d7404bff3d`, #12902) and native socket throttling (`0d0504aca9`,
#12903) retain their own unchanged evidence. The current part does not claim
actual-browser throttling, hurt-overflow ordering, retry-queue/outage policy,
CPU isolation or fleet capacity.
