# 74. A decision in the world

> **Status: partly implemented.** The `#authority` bounded world-actor decision
> gate ([`metaverse-engine/ai/decide.coax`](../../metaverse-engine/ai/decide.coax)),
> replay envelope ([`metaverse-engine/core/replay.coax`](../../metaverse-engine/core/replay.coax)),
> and decision trace ([`metaverse-engine/core/decision-trace.coax`](../../metaverse-engine/core/decision-trace.coax))
> run on `cpu`, grounded by
> [`openagents/examples/world-guard-decision.coax`](../examples/world-guard-decision.coax)
> and [`openagents/examples/world-guard-decision.out`](../examples/world-guard-decision.out),
> and are consumed by [`everglade/npc/guard-replay.coax`](../../everglade/npc/guard-replay.coax)
> and [`metaverse-engine/ai/pilot.coax`](../../metaverse-engine/ai/pilot.coax).
> Remaining: end-to-end capstone execution combining a live Pylon `SystemOne`
> recompute check, kind `3201` receipt, and Lightning payout inside Everglade.

## Premise

This chapter joins the book's parts in one scene: a guard in a town decides
what to do, a Pylon serves that decision, and the Pylon's owner is paid.

## Draft

In [Episode 290](../../docs/transcripts/290.md) the owner attacks a building
in Everglade and the town's guards chase him. They run on simple scripts. Then
he describes what he wants: characters that coordinate through real decisions
and are truly hard to beat. He says he would pay a great deal for a game like
that.

A guard's choice is a typed decision. The state is what the guard can see. The
question is a choice among a few actions: warn, chase, call for help, give up.
That is the same kind of question the front asks in chapter 15, and the same
kind of model answers it, in a fraction of a second.

So the scene uses everything. The world sends the decision through the gateway
(chapter 68). A Pylon serves it (chapter 70). A second machine can recompute
it, because the engine is deterministic. A receipt is written (chapter 57).
The Pylon's owner is paid (chapter 58). The realm's maker chose the guard's
questions, and published them as part of the realm (chapter 73).

This is why the OpenAgents Metaverse is in a book about agents. A world full of characters is
a steady buyer of small decisions, and that demand is what a compute network
has always lacked.

The books already share a design for this scene, the capstone called "A
guard's decision is paid for". It runs across Everglade, the OpenAgents Metaverse Engine, Psionic,
Nostr and Bitcoin. This chapter is where it lands in the OpenAgents book, and
it should run as one demonstration from end to end.

## What we build

A plan for what this chapter ships once the draft is approved:

- A guard whose choices are typed decisions sent through the gateway.
- A Pylon serving those decisions, with a recompute check and a receipt per
  decision.
- One demonstration from the guard's choice to the Pylon owner's payout.

It builds on what exists today:

- [`metaverse-engine/ai/decide.coax`](../../metaverse-engine/ai/decide.coax)
- [`metaverse-engine/core/decision-trace.coax`](../../metaverse-engine/core/decision-trace.coax)
- [`everglade`](../../everglade)
- [`psionic/lib/pylon`](../../psionic/lib/pylon)
- [`coax/lib/gateway/pylon.coax`](../../coax/lib/gateway/pylon.coax)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)
- [`docs/books/capstones/a-guard-decision-is-paid-for.md`](../../docs/books/capstones/a-guard-decision-is-paid-for.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The guard's decision | `openagents.decision.v1alpha1` `SystemOneRequest`, `SystemOneResponse` ([`systemone.proto`](../../proto/openagents/decision/v1alpha1/systemone.proto)) | HTTP; Nostr kind 25910 to a Pylon | In use on the server: the generated codec `psionic/lib/serve-proto.coax` |
| The Pylon's receipt | `openagents.pylon.v1alpha1` `ServiceReceipt` ([`mesh.proto`](../../proto/openagents/pylon/v1alpha1/mesh.proto)) | Nostr kind 3201 | Schema file: a projection of NIP-PYLON's JSON. The signed JSON stays the authority |
| A question to a character | Planned `openagents.verse.npc.v1alpha1` ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step E4) | HTTP | Four hand-written client encoders today |

### The chapter's check

- **Source:** [`metaverse-engine/ai/decide.coax`](../../metaverse-engine/ai/decide.coax),
  [`metaverse-engine/core/replay.coax`](../../metaverse-engine/core/replay.coax),
  [`openagents/examples/world-guard-decision.coax`](../examples/world-guard-decision.coax),
  and [`openagents/examples/world-guard-decision.out`](../examples/world-guard-decision.out)
  (remaining: full Pylon + kind `3201` receipt + payout capstone in
  [`a-guard-decision-is-paid-for.md`](../../docs/books/capstones/a-guard-decision-is-paid-for.md)).
- **Trace:** a town guard (`actor=2`) senses entity `40` at `(3, 0, 2)` on tick
  `10`, opens ticket `req=1` with deadline `+64` ticks, admits `DECIDE_CHASE`
  (`kind=3`, `target=40`, `open=0`), and on an expired second ticket
  (`expired_mask=4`) takes deterministic `decide_fallback` (`DECIDE_GO` to `(3,2)`,
  `digest=3803845797`).
- **Refusal:** replaying a closed ticket answer fails with `DECIDE_UNASKED`;
  chasing an unsensed entity (`99`) fails with `DECIDE_UNSENSED`; an
  out-of-bounds `DECIDE_GO` past `DECIDE_REACH` fails with `DECIDE_FORBIDDEN`;
  an answer past deadline fails with `DECIDE_LATE`; a context mismatch fails
  with `DECIDE_STALE`; and an answer after `decide_transfer` fails with
  `DECIDE_UNASKED`.
- **Independent check:** `coax/bin/coax run openagents/examples/world-guard-decision.coax`
  and `--release` match [`openagents/examples/world-guard-decision.out`](../examples/world-guard-decision.out),
  alongside `#authority` mutation checks in [`metaverse-engine/ai/decide.coax`](../../metaverse-engine/ai/decide.coax),
  Python protobuf decoding against [`systemone.proto`](../../proto/openagents/decision/v1alpha1/systemone.proto),
  and kind `25910` vectors in [`dec_eval_vectors.py`](../../coax/tests/nostr/dec_eval_vectors.py).

Planned paths: none. The decision runs in the engine's
[`metaverse-engine/ai/decide.coax`](../../metaverse-engine/ai/decide.coax).

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 1:30 and 26:00, guards and characters that decide
  - [Episode 284, *Gamifying Coder*](../../docs/transcripts/284.md): work and play in one world
  - [Episode 237, *You Must Construct Additional Pylons*](../../docs/transcripts/237.md): accepted outcomes as the unit of pay
- **Old code at `coder-v1.0.0`, crates:** `verse-game`, `verse-zone-everglade`, `pylon`, `psionic`, `jev`
- **NIPs:** [NIP-DEC](../../nostr/nips/openagents/NIP-DEC.md), [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md), [NIP-X402](../../nostr/nips/openagents/NIP-X402.md), [NIP-MV](../../nostr/nips/openagents/NIP-MV.md)
- **Also:**
  - [A guard's decision is paid for](../../docs/books/capstones/a-guard-decision-is-paid-for.md), the shared capstone
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P11, P12

## Open questions for the owner

1. Who pays for a guard's decisions: the realm's maker, the player, or us
   while the network starts?

## Source listing

Run the Chapter 74 example in [`openagents/examples/world-guard-decision.coax`](../examples/world-guard-decision.coax) (committed transcript [`openagents/examples/world-guard-decision.out`](../examples/world-guard-decision.out)):

<!-- source: openagents/examples/world-guard-decision.coax world-guard-decision -->
```jai
wgd_run :: () -> bool {
  // 1. Construct a recording replay session and decision table for a town guard (actor 2).
  ev_buf: [32] ReplayEvent;
  mark_buf: [32] int;
  ch_buf: [REPLAY_SOURCES] ReplayChannel;
  cnt_buf: [REPLAY_COUNTERS] int;
  sess := ReplaySession.{events = ev_buf[..], marks = mark_buf[..], channels = ch_buf[..], n = cnt_buf[..]};
  replay_record(sess, 1001, 42);

  t: [1] DecideTable;
  decide_table(t[..]);
  senses: Senses;
  mem := memory_note(Memory.{}, senses, Stimulus.{
    kind = StimulusKind.sight, source = 40, at = world_point(3, 0, 2), tick = 10
  }).memory;
  ctx := decide_context(mem);

  // 2. Ask for a bounded decision (wait <= 64 ticks) and admit a valid CHASE intent for sensed entity 40.
  ask := decide_ask(t[..], sess, 2, 10, 64, ctx);
  ans := decide_answer(ask, 2, 0, ctx, DECIDE_CHASE, 40, 0, 0);
  adm := decide_admit(t[..], ans, 22, mem, senses);
  print("guard ask=(status=%,req=%,live=%) admit=(status=%,kind=%,target=%) open=%\n",
    ask.status, ask.request, ask.live, adm.status, adm.intent.kind, adm.intent.target, decide_open(t[..]));

  // 3. Deadline expiry closes the ticket and falls back deterministically to the sensed location.
  ask2 := decide_ask(t[..], sess, 2, 30, 10, ctx);
  expired_mask := decide_expire(t[..], 41);
  fb := decide_fallback(2, mem);
  dig := decide_digest(2166136261, t[..]);
  print("fallback expired_mask=% kind=% at=(%,%) digest=%\n",
    expired_mask, fb.kind, cast(int) fb.x, cast(int) fb.z, dig);

  // 4. Fail-closed refusals: duplicate on closed ticket, unsensed target (hidden state),
  // forbidden out-of-bounds GO, late answer past deadline, and stale owner after transfer.
  ref_dup := decide_admit(t[..], ans, 23, mem, senses).status == DECIDE_UNASKED;

  a_uns := decide_ask(t[..], sess, 2, 50, 20, ctx);
  ref_unsensed := decide_admit(t[..], decide_answer(a_uns, 2, 0, ctx, DECIDE_CHASE, 99, 0, 0), 55, mem, senses).status == DECIDE_UNSENSED;

  a_forb := decide_ask(t[..], sess, 2, 60, 20, ctx);
  ref_forbidden := decide_admit(t[..], decide_answer(a_forb, 2, 0, ctx, DECIDE_GO, -1, DECIDE_REACH + 1, 0), 65, mem, senses).status == DECIDE_FORBIDDEN;

  a_late := decide_ask(t[..], sess, 2, 70, 10, ctx);
  ref_late := decide_admit(t[..], decide_answer(a_late, 2, 0, ctx, DECIDE_HOLD, -1, 0, 0), 81, mem, senses).status == DECIDE_LATE;

  a_stale := decide_ask(t[..], sess, 2, 90, 20, ctx);
  ref_stale := decide_admit(t[..], decide_answer(a_stale, 2, 0, ctx + 1, DECIDE_HOLD, -1, 0, 0), 92, mem, senses).status == DECIDE_STALE;
  decide_transfer(t[..], 2);
  ref_xfer := decide_admit(t[..], decide_answer(a_stale, 2, 0, ctx, DECIDE_HOLD, -1, 0, 0), 93, mem, senses).status == DECIDE_UNASKED;
  print("refuse dup=% unsensed=% forbidden=% late=% stale=% xfer=%\n",
    ref_dup, ref_unsensed, ref_forbidden, ref_late, ref_stale, ref_xfer);

  return ask.status == DECIDE_OK && ask.request == 1 && ask.live &&
         adm.status == DECIDE_OK && adm.intent.kind == DECIDE_CHASE && adm.intent.target == 40 &&
         ask2.status == DECIDE_OK && expired_mask == (1 << 2) &&
         fb.kind == DECIDE_GO && cast(int) fb.x == 3 && cast(int) fb.z == 2 &&
         ref_dup && ref_unsensed && ref_forbidden && ref_late && ref_stale && ref_xfer;
}
```

Output (`coax/bin/coax run openagents/examples/world-guard-decision.coax`, identical in `--release`):

```text
guard ask=(status=0,req=1,live=true) admit=(status=0,kind=3,target=40) open=0
fallback expired_mask=4 kind=2 at=(3,2) digest=3803845797
refuse dup=true unsensed=true forbidden=true late=true stale=true xfer=true
ok=true
```
